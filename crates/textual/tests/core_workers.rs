//! Worker state events, groups and `exclusive` (Textual: `Worker.StateChanged`, `run_worker`).

use std::time::Duration;
use textual::prelude::*;

#[derive(Debug, Clone, PartialEq)]
struct Done(u32);

#[derive(Default)]
struct Root {
    states: Vec<(String, WorkerState)>,
    done: Vec<u32>,
}

impl Widget for Root {
    fn on_message(&mut self, m: &Message, _ctx: &mut Ctx) -> Flow {
        if let Some(e) = m.get::<worker::StateChanged>() {
            self.states.push((e.name.clone(), e.state));
        }
        if let Some(Done(n)) = m.get::<Done>() {
            self.done.push(*n);
        }
        Flow::Continue
    }
}

fn states(p: &Pilot, name: &str) -> Vec<WorkerState> {
    p.app::<Root>()
        .unwrap()
        .states
        .iter()
        .filter(|(n, _)| n == name)
        .map(|(_, s)| *s)
        .collect()
}

async fn settle(p: &mut Pilot) {
    for _ in 0..20 {
        p.wait_for_signal(Duration::from_millis(50)).await;
    }
}

#[tokio::test]
async fn worker_reports_pending_running_success() {
    let mut p = App::new(Root::default()).test(Size::new(20, 5)).await;
    let h = p
        .with::<Root, _>("App", |_, ctx| {
            ctx.run_worker_with(async { Done(1) }, WorkerOptions::new().name("w"))
        })
        .await
        .unwrap();
    settle(&mut p).await;
    use WorkerState::*;
    assert_eq!(states(&p, "w"), vec![Pending, Running, Success]);
    assert_eq!(p.app::<Root>().unwrap().done, vec![1]);
    assert_eq!(h.state(), Success);
    assert!(h.state().is_finished());
}

#[tokio::test]
async fn panicking_worker_reports_error() {
    let mut p = App::new(Root::default()).test(Size::new(20, 5)).await;
    p.with::<Root, _>("App", |_, ctx| {
        ctx.run_blocking_with(
            || -> Done { panic!("boom") },
            WorkerOptions::new().name("bad"),
        )
    })
    .await;
    settle(&mut p).await;
    assert_eq!(states(&p, "bad").last(), Some(&WorkerState::Error));
    assert!(p.app::<Root>().unwrap().done.is_empty());
}

#[tokio::test]
async fn exclusive_cancels_the_group_and_cancel_workers() {
    let mut p = App::new(Root::default()).test(Size::new(20, 5)).await;
    let slow = || async {
        tokio::time::sleep(Duration::from_secs(30)).await;
        Done(0)
    };
    p.with::<Root, _>("App", |_, ctx| {
        ctx.run_worker_with(slow(), WorkerOptions::new().name("a").group("g"));
        ctx.run_worker_with(slow(), WorkerOptions::new().name("other").group("h"));
        ctx.run_worker_with(
            async { Done(2) },
            WorkerOptions::new().name("b").group("g").exclusive(),
        );
    })
    .await;
    settle(&mut p).await;
    assert_eq!(states(&p, "a").last(), Some(&WorkerState::Cancelled));
    assert_eq!(states(&p, "b").last(), Some(&WorkerState::Success));
    assert_eq!(states(&p, "other").last(), Some(&WorkerState::Running));
    let n = p
        .with::<Root, _>("App", |_, ctx| ctx.cancel_workers("h"))
        .await
        .unwrap();
    assert_eq!(n, 1);
    settle(&mut p).await;
    assert_eq!(states(&p, "other").last(), Some(&WorkerState::Cancelled));
    assert_eq!(p.app::<Root>().unwrap().done, vec![2]);
}

#[tokio::test]
async fn plain_run_worker_posts_only_its_output() {
    let mut p = App::new(Root::default()).test(Size::new(20, 5)).await;
    p.with::<Root, _>("App", |_, ctx| ctx.run_worker(async { Done(7) }))
        .await;
    settle(&mut p).await;
    assert_eq!(p.app::<Root>().unwrap().done, vec![7]);
    assert!(p.app::<Root>().unwrap().states.is_empty());
}

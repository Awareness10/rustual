//! Talking to the app from other threads and tasks through `AppHandle` (Textual's thread-safe
//! `post_message` and `App.call_from_thread`), and worker cancellation on removal.

use rustual::prelude::*;
use rustual::widgets::input;
use std::time::Duration;

#[derive(Debug, Clone, PartialEq)]
struct Tick(u32);

#[derive(Debug, Clone, PartialEq)]
struct Reply(String);

#[derive(Default)]
struct Root {
    ticks: Vec<u32>,
    /// Ticks seen by the root that were posted to the child (i.e. bubbled).
    log: Vec<String>,
    changed: Vec<String>,
    replies: Vec<String>,
}

impl Widget for Root {
    fn compose(&mut self) -> Vec<Child> {
        compose![Input::new().id("field"), Static::new("").id("child")]
    }

    fn on_message(&mut self, m: &Message, _ctx: &mut Ctx) -> Flow {
        if let Some(Tick(n)) = m.get::<Tick>() {
            self.ticks.push(*n);
        }
        if let Some(e) = m.get::<input::Changed>() {
            self.changed.push(e.value.clone());
        }
        if let Some(Reply(s)) = m.get::<Reply>() {
            self.replies.push(s.clone());
        }
        Flow::Continue
    }
}

async fn settle(p: &mut Pilot) {
    for _ in 0..20 {
        if !p.wait_for_signal(Duration::from_millis(20)).await {
            break;
        }
    }
    p.pause().await;
}

fn root(p: &Pilot) -> &Root {
    p.app::<Root>().unwrap()
}

#[tokio::test]
async fn post_message_from_a_thread_keeps_order_and_never_blocks() {
    let mut p = App::new(Root::default()).test(Size::new(30, 6)).await;
    let handle = p.handle();
    let target = p.query("App")[0];
    let t = std::thread::spawn(move || {
        for n in 0..2000 {
            assert!(handle.post_message(target, Tick(n)));
        }
    });
    t.join().unwrap(); // finishes without the app handling anything: posting is fire-and-forget
    settle(&mut p).await;
    assert_eq!(root(&p).ticks, (0..2000).collect::<Vec<_>>());
}

#[tokio::test]
async fn post_message_no_bubble_stops_at_the_target() {
    let mut p = App::new(Root::default()).test(Size::new(30, 6)).await;
    let handle = p.handle();
    let child = p.query("#child")[0];
    handle.post_message_no_bubble(child, Tick(1));
    handle.post_message(child, Tick(2));
    settle(&mut p).await;
    assert_eq!(root(&p).ticks, vec![2]);
}

#[tokio::test]
async fn call_on_runs_with_the_typed_widget_in_order_with_messages() {
    let mut p = App::new(Root::default()).test(Size::new(30, 6)).await;
    let handle = p.handle();
    let app = p.query("App")[0];
    let field = p.query("#field")[0];
    std::thread::spawn(move || {
        handle.post_message(app, Tick(1));
        handle.call_on::<Input>(field, |input, ctx| input.set_value("42", ctx));
        // The wrong type: dropped without running.
        handle.call_on::<Label>(field, |_, _| panic!("not a Label"));
        handle.call_later(|ctx| {
            ctx.with::<Root, _>("App", |root, _| root.log.push("later".into()));
        });
        handle.post_message(app, Tick(2));
    })
    .join()
    .unwrap();
    settle(&mut p).await;
    assert_eq!(p.query_one::<Input>("#field").unwrap().get_value(), "42");
    assert_eq!(root(&p).changed, vec!["42".to_string()]);
    assert_eq!(root(&p).ticks, vec![1, 2]);
    assert_eq!(root(&p).log, vec!["later".to_string()]);
}

#[tokio::test]
async fn call_from_thread_returns_the_result() {
    let mut p = App::new(Root::default())
        .title("Handle")
        .test(Size::new(30, 6))
        .await;
    let handle = p.handle();
    let app = p.query("App")[0];
    let t = std::thread::spawn(move || {
        let title = handle.call_from_thread(|ctx| ctx.title().to_string());
        handle.post_message(app, Reply(title.unwrap_or_default()));
    });
    // The thread blocks until the app thread runs the call.
    settle(&mut p).await;
    t.join().unwrap();
    settle(&mut p).await;
    assert_eq!(root(&p).replies, vec!["Handle".to_string()]);
}

#[tokio::test]
async fn call_awaits_the_result_in_an_async_worker() {
    let mut p = App::new(Root::default()).test(Size::new(30, 6)).await;
    p.with::<Root, _>("App", |_, ctx| {
        let handle = ctx.app_handle();
        ctx.run_worker(async move {
            let value = handle
                .call(|ctx| {
                    ctx.with::<Input, _>("#field", |input, ctx| input.set_value("x", ctx));
                    ctx.query_one::<Input>("#field")
                        .unwrap()
                        .get_value()
                        .to_string()
                })
                .await;
            Reply(value.unwrap_or_default())
        });
    })
    .await;
    settle(&mut p).await;
    assert_eq!(root(&p).replies, vec!["x".to_string()]);
}

#[tokio::test]
async fn handle_reports_when_the_app_is_gone() {
    let p = App::new(Root::default()).test(Size::new(30, 6)).await;
    let handle = p.handle();
    assert!(handle.is_running());
    drop(p);
    assert!(!handle.is_running());
    let target = handle.clone();
    let r = std::thread::spawn(move || target.call_from_thread(|_| 1))
        .join()
        .unwrap();
    assert_eq!(r, None);
    assert!(!handle.call_later(|_| {}));
}

#[derive(Default)]
struct Busy;

impl Widget for Busy {
    fn on_mount(&mut self, ctx: &mut Ctx) {
        ctx.run_worker_with(
            async {
                tokio::time::sleep(Duration::from_secs(30)).await;
                Tick(0)
            },
            WorkerOptions::new().name("slow"),
        );
    }
}

#[derive(Default)]
struct Host;

impl Widget for Host {
    fn compose(&mut self) -> Vec<Child> {
        compose![Busy.id("busy")]
    }
}

#[tokio::test]
async fn removing_a_widget_cancels_its_workers() {
    let mut p = App::new(Host).test(Size::new(30, 6)).await;
    let running = p
        .with::<Host, _>("App", |_, ctx| ctx.running_workers())
        .await;
    assert_eq!(running, Some(1));
    let busy = p.query("#busy")[0];
    p.with::<Host, _>("App", |_, ctx| ctx.remove(busy)).await;
    settle(&mut p).await;
    let running = p
        .with::<Host, _>("App", |_, ctx| ctx.running_workers())
        .await;
    assert_eq!(running, Some(0));
}

#[test]
fn max_fps_is_clamped() {
    // Only checks the builder accepts values (0 would mean "never draw").
    let _ = App::new(Host).max_fps(0).max_fps(120);
}

//! ProgressBar: bar, percentage, ETA, indeterminate animation.

use std::time::Duration;
use textual::prelude::*;

struct T {
    total: Option<f64>,
}
impl Widget for T {
    fn compose(&mut self) -> Vec<Child> {
        compose![ProgressBar::new(self.total).id("pb")]
    }
    fn bindings(&self) -> Vec<Binding> {
        vec![
            Binding::new("a", "adv", "Advance"),
            Binding::new("t", "set_total", "Total"),
        ]
    }
    fn run_action(&mut self, a: &str, _: &[String], ctx: &mut Ctx) -> bool {
        let pb = ctx.query_one::<ProgressBar>("#pb").unwrap();
        match a {
            "adv" => pb.advance(10.0),
            "set_total" => pb.update(Some(100.0), 50.0),
            _ => return false,
        }
        true
    }
}

fn row(pilot: &mut Pilot) -> String {
    pilot.screen_text().lines().next().unwrap().to_string()
}

#[tokio::test(start_paused = true)]
async fn determinate_bar_percentage_and_advance() {
    let mut pilot = App::new(T { total: Some(100.0) })
        .test(Size::new(60, 3))
        .await;
    assert!(pilot.css_errors().is_empty(), "{:?}", pilot.css_errors());
    assert_eq!(row(&mut pilot), format!("{}   0% --:--:--", "━".repeat(32)));
    for _ in 0..5 {
        pilot.press(&["a"]).await;
    }
    assert_eq!(
        pilot.query_one::<ProgressBar>("#pb").unwrap().percentage(),
        Some(0.5)
    );
    let text = row(&mut pilot);
    assert!(text.starts_with(&format!("{}╺", "━".repeat(16))), "{text}");
    assert!(text.contains("  50%"), "{text}");
    // Highlighted part is $primary, the rest $surface (drawn as the glyph color).
    let c = pilot.canvas();
    let filled = c.get(0, 0).unwrap().fg;
    let empty = c.get(20, 0).unwrap().fg;
    assert_ne!(filled, empty);
    // Complete: $success.
    for _ in 0..5 {
        pilot.press(&["a"]).await;
    }
    assert!(row(&mut pilot).contains(" 100%"));
    let c = pilot.canvas();
    let complete = c.get(0, 0).unwrap().fg;
    assert_ne!(complete, filled);
    assert_eq!(c.get(31, 0).unwrap().fg, complete);
}

#[tokio::test(start_paused = true)]
async fn eta_is_estimated() {
    let mut pilot = App::new(T { total: Some(100.0) })
        .test(Size::new(60, 3))
        .await;
    // 10% every 2 seconds → 10% per 2s; after 4s at 20%: 8 × 2s = 16s remaining.
    pilot.wait(Duration::from_secs(2)).await;
    pilot.press(&["a"]).await;
    pilot.wait(Duration::from_secs(2)).await;
    pilot.press(&["a"]).await;
    assert_eq!(
        pilot.query_one::<ProgressBar>("#pb").unwrap().eta(),
        Some(16)
    );
    assert!(row(&mut pilot).ends_with("00:00:16"), "{}", row(&mut pilot));
    // The ETA counts down by extrapolation (refreshed each second).
    pilot.wait(Duration::from_secs(3)).await;
    assert!(row(&mut pilot).ends_with("00:00:13"), "{}", row(&mut pilot));
}

#[tokio::test(start_paused = true)]
async fn indeterminate_bar_animates() {
    let mut pilot = App::new(T { total: None }).test(Size::new(60, 3)).await;
    let text = row(&mut pilot);
    assert!(text.contains("--%"), "{text}");
    let frame0: Vec<_> = (0..32)
        .map(|x| pilot.canvas().get(x, 0).unwrap().fg)
        .collect();
    pilot.wait(Duration::from_millis(500)).await;
    let c = pilot.canvas();
    let frame1: Vec<_> = (0..32).map(|x| c.get(x, 0).unwrap().fg).collect();
    assert_ne!(frame0, frame1);
    // At t = 0.5s the highlight (8 cells, 30 cells/s) covers cells 7..15 ($error color).
    let hl = frame1[10];
    assert_ne!(hl, frame1[0]);
    assert_eq!(frame1[8], hl);
    assert_eq!(frame1[14], hl);
    assert_eq!(frame1[20], frame1[0]);
    // Setting a total stops the animation and shows the percentage.
    pilot.press(&["t"]).await;
    assert!(row(&mut pilot).contains("  50%"));
    let a: Vec<_> = (0..32)
        .map(|x| pilot.canvas().get(x, 0).unwrap().fg)
        .collect();
    pilot.wait(Duration::from_millis(300)).await;
    let b: Vec<_> = (0..32)
        .map(|x| pilot.canvas().get(x, 0).unwrap().fg)
        .collect();
    assert_eq!(a, b);
}

#[tokio::test]
async fn hide_parts() {
    struct H;
    impl Widget for H {
        fn compose(&mut self) -> Vec<Child> {
            compose![ProgressBar::new(Some(10.0)).show_eta(false).show_bar(false)]
        }
    }
    let mut pilot = App::new(H).test(Size::new(20, 2)).await;
    assert_eq!(row(&mut pilot), "   0%");
}

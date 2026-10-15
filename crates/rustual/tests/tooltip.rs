use rustual::prelude::*;
use std::time::Duration;

struct T;
impl Widget for T {
    fn compose(&mut self) -> Vec<Child> {
        compose![
            Button::new("Hover me").id("b").tooltip("Helpful tip"),
            Label::new("plain").id("plain"),
            Vertical::new()
                .id("parent")
                .tooltip("From parent")
                .with([Label::new("child").id("child")]),
        ]
    }
}

#[tokio::test(start_paused = true)]
async fn hover_shows_tooltip_after_delay_and_hides_on_move() {
    let mut p = App::new(T).test(Size::new(60, 20)).await;
    assert!(p.hover("#b").await);
    p.wait(Duration::from_millis(300)).await;
    assert!(!p.screen_text().contains("Helpful tip"));
    p.wait(Duration::from_millis(300)).await;
    assert!(
        p.screen_text().contains("Helpful tip"),
        "{}",
        p.screen_text()
    );
    assert!(p.css_errors().is_empty(), "{:?}", p.css_errors());

    // Moving to a widget without a tooltip hides it.
    assert!(p.hover("#plain").await);
    p.wait(Duration::from_millis(600)).await;
    assert!(!p.screen_text().contains("Helpful tip"));
}

#[tokio::test(start_paused = true)]
async fn tooltip_inherited_from_ancestor_and_key_hides() {
    let mut p = App::new(T).test(Size::new(60, 20)).await;
    assert!(p.hover("#child").await);
    p.wait(Duration::from_millis(600)).await;
    assert!(p.screen_text().contains("From parent"));
    p.press(&["x"]).await;
    assert!(!p.screen_text().contains("From parent"));
}

#[tokio::test(start_paused = true)]
async fn tooltip_stays_on_screen_and_set_tooltip_updates() {
    let mut p = App::new(T).test(Size::new(60, 20)).await;
    // Near the bottom-right corner: the tooltip flips above and stays inside.
    let id = p.query("#parent")[0];
    let r = p.dom().get(id).unwrap().layout.region;
    p.click_at(59, r.bottom() - 1).await;
    // Click hides; move to show.
    p.hover("#child").await;
    p.wait(Duration::from_millis(600)).await;
    let tid = p.query("Tooltip")[0];
    let tr = p.dom().get(tid).unwrap().layout.region;
    assert!(
        tr.x >= 0 && tr.right() <= 60 && tr.y >= 0 && tr.bottom() <= 20,
        "{tr:?}"
    );

    p.with::<Label, _>("#child", |_l, ctx| ctx.set_tooltip(Some("Own tip")))
        .await;
    // A displayed tooltip is updated in place.
    assert!(p.screen_text().contains("Own tip"));
    // Moving within the same widget while shown hides it (as in Textual)...
    p.hover("#child").await;
    assert!(!p.screen_text().contains("Own tip"));
    // ...and resting again shows it.
    p.hover("#child").await;
    p.wait(Duration::from_millis(600)).await;
    assert!(p.screen_text().contains("Own tip"));
}

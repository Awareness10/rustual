//! Toasts: notifications shown bottom-right over content, dismissed by click or timeout.

use std::time::Duration;
use textual::prelude::*;

struct T;
impl Widget for T {
    fn compose(&mut self) -> Vec<Child> {
        compose![Static::new("x".repeat(80 * 3)).id("content")]
    }
    fn bindings(&self) -> Vec<Binding> {
        vec![
            Binding::new("n", "note", "Notify"),
            Binding::new("e", "err", "Error"),
            Binding::new("p", "push", "Push"),
        ]
    }
    fn run_action(&mut self, a: &str, _: &[String], ctx: &mut Ctx) -> bool {
        match a {
            "note" => ctx.notify("Saved!"),
            "err" => ctx.notify_with(
                "Disk full",
                "Oops",
                Severity::Error,
                Some(Duration::from_secs(10)),
            ),
            "push" => {
                ctx.push_screen(Label::new("second"));
            }
            _ => return false,
        }
        true
    }
}

fn toast_count(pilot: &Pilot) -> usize {
    pilot
        .query("Toast")
        .into_iter()
        .filter(|id| pilot.dom().contains(*id))
        .count()
}

#[tokio::test(start_paused = true)]
async fn notification_shows_toast_bottom_right() {
    let mut pilot = App::new(T).test(Size::new(80, 20)).await;
    assert!(pilot.css_errors().is_empty(), "{:?}", pilot.css_errors());
    assert_eq!(toast_count(&pilot), 0);
    pilot.press(&["n"]).await;
    assert_eq!(toast_count(&pilot), 1);
    let id = pilot.query("Toast")[0];
    let r = pilot.dom().get(id).unwrap().layout.region;
    // width 60, max-width 50% of the holder (78 = 80 - the rack's scrollbar gutter) → 39; height 3 (padding 1 + 1 line); margin-bottom 1 on the rack.
    assert_eq!((r.width, r.height), (39, 3));
    assert_eq!(r.bottom(), 19);
    assert!(r.right() >= 76, "{r:?}");
    let text = pilot.screen_text();
    let lines: Vec<&str> = text.lines().collect();
    assert!(lines[17].contains("Saved!"), "{text}");
    // Content above is still visible (the rack overlays, does not push content).
    assert!(lines[0].starts_with("xxxx"));
    // The rest of the bottom row (left of the toast) shows what's under it.
    let c = pilot.canvas();
    let screen_bg = c.get(0, 10).unwrap().bg;
    assert_eq!(c.get(5, 19).unwrap().bg, screen_bg);
    // Information toasts have a $success "outer" left border.
    assert_eq!(c.get(r.x, r.y + 1).unwrap().symbol, "▌");
    assert_ne!(c.get(r.x + 2, r.y + 1).unwrap().bg, screen_bg);
}

#[tokio::test(start_paused = true)]
async fn toast_expires() {
    let mut pilot = App::new(T).test(Size::new(80, 20)).await;
    pilot.press(&["n"]).await;
    pilot.wait(Duration::from_secs(4)).await;
    assert_eq!(toast_count(&pilot), 1);
    pilot.wait(Duration::from_millis(1100)).await;
    assert_eq!(toast_count(&pilot), 0);
    assert!(pilot.notifications().is_empty());
    assert!(!pilot.screen_text().contains("Saved!"));
}

#[tokio::test(start_paused = true)]
async fn toasts_stack_and_click_dismisses() {
    let mut pilot = App::new(T).test(Size::new(80, 20)).await;
    pilot.press(&["n", "e"]).await;
    assert_eq!(toast_count(&pilot), 2);
    let text = pilot.screen_text();
    assert!(text.contains("Oops"), "{text}");
    assert!(text.contains("Disk full"));
    let ids = pilot.query("Toast");
    let first = pilot.dom().get(ids[0]).unwrap().layout.region;
    let second = pilot.dom().get(ids[1]).unwrap().layout.region;
    // Newest at the bottom, 1 cell margin between.
    assert_eq!(second.bottom(), 19);
    assert_eq!(first.bottom(), second.y - 1);
    // Title is colored ($text-error) and bold.
    let c = pilot.canvas();
    let title = c.get(second.x + 2, second.y + 1).unwrap();
    assert!(title.attrs.contains(textual::render::Attrs::BOLD));
    // Click dismisses the error toast and its notification.
    pilot.click_at(second.x + 5, second.y + 1).await;
    assert_eq!(toast_count(&pilot), 1);
    assert_eq!(pilot.notifications(), vec!["Saved!".to_string()]);
    assert!(!pilot.screen_text().contains("Oops"));
}

#[tokio::test(start_paused = true)]
async fn toasts_show_on_pushed_screens() {
    let mut pilot = App::new(T).test(Size::new(80, 20)).await;
    pilot.press(&["e", "p"]).await;
    assert_eq!(pilot.screen_count(), 2);
    let text = pilot.screen_text();
    assert!(text.contains("second"), "{text}");
    assert!(text.contains("Disk full"), "{text}");
}

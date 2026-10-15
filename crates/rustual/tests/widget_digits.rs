//! Digits: 3x3 glyphs, bold variant, alignment, update.

use rustual::prelude::*;

struct T;
impl Widget for T {
    fn compose(&mut self) -> Vec<Child> {
        compose![Digits::new("3.14").id("d")]
    }
    fn bindings(&self) -> Vec<Binding> {
        vec![Binding::new("u", "upd", "Update")]
    }
    fn run_action(&mut self, a: &str, _: &[String], ctx: &mut Ctx) -> bool {
        if a == "upd" {
            ctx.query_one::<Digits>("#d").unwrap().update("42");
            return true;
        }
        false
    }
}

#[tokio::test]
async fn digits_render_three_rows() {
    let mut pilot = App::new(T).test(Size::new(30, 5)).await;
    assert!(pilot.css_errors().is_empty(), "{:?}", pilot.css_errors());
    let text = pilot.screen_text();
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines[0], "╶─╮ ╶╮ ╷ ╷");
    assert_eq!(lines[1], " ─┤  │ ╰─┤");
    assert_eq!(lines[2], "╶─╯•╶┴╴  ╵");
    assert_eq!(lines[3], "");
    pilot.press(&["u"]).await;
    let text = pilot.screen_text();
    assert_eq!(text.lines().next().unwrap(), "╷ ╷╶─╮");
}

#[tokio::test]
async fn bold_and_aligned_digits() {
    let mut pilot = App::new(T)
        .css("#d { text-style: bold; text-align: right; width: 20; }")
        .test(Size::new(30, 4))
        .await;
    let text = pilot.screen_text();
    let first = text.lines().next().unwrap();
    // 10 cells of digits right-aligned in 20.
    assert_eq!(first, format!("{}╺━┓ ╺┓ ╻ ╻", " ".repeat(10)), "{text}");
}

#[tokio::test]
async fn auto_width() {
    let mut pilot = App::new(T)
        .css("#d { width: auto; border: round red; }")
        .test(Size::new(30, 6))
        .await;
    let id = pilot.query("#d")[0];
    let r = pilot.dom().get(id).unwrap().layout.region;
    assert_eq!((r.width, r.height), (12, 5));
    assert!(pilot.screen_text().starts_with("╭──────────╮"));
}

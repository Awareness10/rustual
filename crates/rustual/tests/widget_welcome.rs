//! Welcome tests.

use rustual::prelude::*;

struct WApp(Welcome);

impl Widget for WApp {
    fn compose(&mut self) -> Vec<Child> {
        compose![self.0.clone()]
    }
}

#[tokio::test]
async fn renders_text_and_ok_exits() {
    let mut p = App::new(WApp(Welcome::new())).test(Size::new(80, 30)).await;
    assert!(p.css_errors().is_empty(), "{:?}", p.css_errors());
    let text = p.screen_text();
    assert!(text.contains("Welcome!"), "{text}");
    assert!(text.contains("┏"), "h1 is boxed: {text}");
    assert!(text.contains("Text User Interface"), "{text}");
    assert!(text.contains("Dune quote"), "{text}");
    assert!(text.contains("▌ \"I must not fear."), "{text}");
    assert!(text.contains("I will remain."), "{text}");
    // The OK button is docked at the bottom.
    let lines: Vec<&str> = text.lines().collect();
    let ok_row = lines.iter().position(|l| l.trim() == "OK").unwrap();
    assert!(ok_row >= 26, "{text}");
    // Bold paragraph text.
    let c = p.canvas();
    let row = lines.iter().position(|l| l.contains("We hope")).unwrap();
    let x = lines[row].find("We hope").unwrap() as i32;
    assert!(c
        .get(x, row as i32)
        .unwrap()
        .attrs
        .contains(rustual::render::Attrs::BOLD));
    assert!(!p.has_exited());
    p.click("#close").await;
    assert!(p.has_exited());
}

#[tokio::test]
async fn exit_can_be_disabled() {
    let mut p = App::new(WApp(Welcome::new().exit_on_close(false)))
        .test(Size::new(80, 30))
        .await;
    p.click("#close").await;
    assert!(!p.has_exited());
}

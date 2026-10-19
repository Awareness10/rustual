//! `TEXTUAL_THEME` picks the default theme (Textual's `constants.DEFAULT_THEME`).
//!
//! One test in its own binary: it sets a process-wide environment variable.

use rustual::prelude::*;

struct Demo;
impl Widget for Demo {
    fn compose(&mut self) -> Vec<Child> {
        compose![Button::primary("Go").id("go")]
    }
}

async fn background_of_button(app: App) -> (String, String) {
    let mut p = app.test(Size::new(30, 6)).await;
    assert!(p.css_errors().is_empty(), "{:?}", p.css_errors());
    let text = p.screen_text();
    let y = text
        .lines()
        .position(|l| l.contains("Go"))
        .expect("button on screen");
    let x = text.lines().nth(y).unwrap().find("Go").unwrap() as i32;
    let bg = format!("{:?}", p.canvas().get(x, y as i32).unwrap().bg);
    (p.theme().to_string(), bg)
}

#[tokio::test]
async fn textual_theme_env_sets_the_default_theme() {
    std::env::remove_var("TEXTUAL_THEME");
    let (name, dark_bg) = background_of_button(App::new(Demo)).await;
    assert_eq!(name, "textual-dark");

    std::env::set_var("TEXTUAL_THEME", "nord");
    let (name, nord_bg) = background_of_button(App::new(Demo)).await;
    assert_eq!(name, "nord");
    assert_ne!(nord_bg, dark_bg);

    // The builder wins over the variable, as assigning `App.theme` does in Textual.
    let (name, _) = background_of_button(App::new(Demo).theme("dracula")).await;
    assert_eq!(name, "dracula");

    // An unknown name falls back to the default theme instead of leaving the app unstyled.
    std::env::set_var("TEXTUAL_THEME", "no-such-theme");
    let (name, bg) = background_of_button(App::new(Demo)).await;
    assert_eq!(name, "textual-dark");
    assert_eq!(bg, dark_bg);

    std::env::remove_var("TEXTUAL_THEME");
}

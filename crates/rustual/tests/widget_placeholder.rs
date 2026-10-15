//! Placeholder: label / id, variants cycled by click, palette colors.

use rustual::prelude::*;

struct T;
impl Widget for T {
    fn compose(&mut self) -> Vec<Child> {
        compose![Horizontal::new().with(compose![
            Placeholder::new().id("one"),
            Placeholder::with_label("Second").id("two"),
            Placeholder::new(),
        ])]
    }
}

#[tokio::test]
async fn placeholder_shows_label_or_id() {
    let mut pilot = App::new(T)
        .css("Placeholder { width: 1fr; }")
        .test(Size::new(60, 6))
        .await;
    assert!(pilot.css_errors().is_empty(), "{:?}", pilot.css_errors());
    let text = pilot.screen_text();
    assert!(text.contains("#one"), "{text}");
    assert!(text.contains("Second"));
    assert!(text.contains("Placeholder"));
    // Content is centered both ways: row 2 (of 0..6), centered horizontally in 20 columns.
    let row = text.lines().nth(2).unwrap();
    assert_eq!(row.find("#one"), Some(8), "{row:?}");
}

#[tokio::test]
async fn consecutive_placeholders_get_consecutive_colors() {
    let mut pilot = App::new(T)
        .css("Placeholder { width: 1fr; }")
        .test(Size::new(60, 6))
        .await;
    let offsets: Vec<usize> = pilot
        .query("Placeholder")
        .into_iter()
        .map(|id| {
            pilot
                .dom()
                .widget::<Placeholder>(id)
                .unwrap()
                .color_offset()
        })
        .collect();
    assert_eq!(offsets, vec![0, 1, 2]);
    let c = pilot.canvas();
    let screen_bg = c.get(0, 0).unwrap().bg;
    let a = c.get(1, 0).unwrap().bg;
    let b = c.get(21, 0).unwrap().bg;
    assert_ne!(a, b);
    // #881177 at 50% over the screen background.
    let _ = screen_bg;
    // textual-dark $background is #121212.
    let expected = Color::parse("#121212")
        .unwrap()
        .composite(Color::parse("#881177").unwrap().with_alpha(0.5));
    assert_eq!(a, expected);
}

#[tokio::test]
async fn click_cycles_variants() {
    let mut pilot = App::new(T)
        .css("Placeholder { width: 1fr; }")
        .test(Size::new(60, 6))
        .await;
    pilot.click("#one").await;
    assert_eq!(
        pilot
            .query_one::<Placeholder>("#one")
            .unwrap()
            .get_variant(),
        PlaceholderVariant::Size
    );
    assert!(
        pilot.screen_text().contains("20 x 6"),
        "{}",
        pilot.screen_text()
    );
    pilot.click("#one").await;
    let text = pilot.screen_text();
    assert!(text.contains("Lorem ipsum"), "{text}");
    // `-text` adds padding 1: the first row of the widget stays blank.
    assert!(text.lines().next().unwrap().is_empty(), "{text}");
    pilot.click("#one").await;
    assert!(pilot.screen_text().contains("#one"));
}

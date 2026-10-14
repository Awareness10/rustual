use textual::prelude::*;

struct Demo;
impl Widget for Demo {
    fn compose(&mut self) -> Vec<Child> {
        compose![
            Static::new("Hello, [b]World[/]!").id("greeting").border_title("textual-rs").border_subtitle("sub"),
            Horizontal::new().id("row").with(compose![
                Button::new("Default"),
                Button::primary("Primary").id("p"),
                Button::error("Error"),
            ]),
        ]
    }
}

#[tokio::test]
async fn visual_smoke() {
    let mut pilot = App::new(Demo)
        .css("#greeting { border: round $accent; height: 5; content-align: center middle; } #row { height: auto; }")
        .test(Size::new(60, 12))
        .await;
    assert!(pilot.css_errors().is_empty(), "{:?}", pilot.css_errors());
    let text = pilot.screen_text();
    println!("{text}");
    assert!(text.contains("╭─ textual-rs"), "{text}");
    assert!(text.contains("Hello, World!"));
    assert!(text.contains("Primary"));
    let c = pilot.canvas();
    let cell = c.get(0, 0).unwrap();
    println!("corner fg={} bg={}", cell.fg.hex(), cell.bg.hex());
}

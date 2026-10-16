use rustual::prelude::*;

struct Demo;
impl Widget for Demo {
    fn compose(&mut self) -> Vec<Child> {
        compose![
            Static::new("Hi you").id("h"),
            Container::new()
                .id("box")
                .with(compose![Static::new("x").id("child")]),
            Static::new("plain").id("off"),
        ]
    }
}

const CSS: &str = "
Screen { background: black; }
#h { width: 12; height: 4; border: solid white; padding: 0 1; background: black; hatch: right red; }
#box { width: 8; height: 3; background: black; hatch: cross #ffffff 50%; }
#child { width: 3; height: 1; background: blue; }
#off { width: 8; height: 1; hatch: right red; }
#off { hatch: none; }
";

fn row(text: &str, y: usize) -> String {
    text.lines().nth(y).unwrap_or_default().to_string()
}

#[tokio::test]
async fn hatch_fills_blank_cells_inside_the_border() {
    let mut pilot = App::new(Demo).css(CSS).test(Size::new(20, 9)).await;
    assert!(pilot.css_errors().is_empty(), "{:?}", pilot.css_errors());
    let text = pilot.screen_text();

    // Border untouched; padding and the spaces in and after the text are hatched.
    assert!(row(&text, 0).starts_with("┌──────────┐"), "{text}");
    assert!(row(&text, 1).starts_with("│╱Hi╱you╱╱╱│"), "{text}");
    assert!(row(&text, 2).starts_with("│╱╱╱╱╱╱╱╱╱╱│"), "{text}");
    assert!(row(&text, 3).starts_with("└──────────┘"), "{text}");

    // Hatch cells: fg = background + color, bg = the widget's background.
    let c = pilot.canvas();
    let cell = c.get(1, 1).unwrap();
    assert_eq!(cell.symbol, "╱");
    assert_eq!(cell.fg.hex(), "#FF0000");
    assert_eq!(cell.bg.hex(), "#000000");
    // Text keeps its own style.
    assert_eq!(c.get(2, 1).unwrap().symbol, "H");

    // Alpha blends the hatch color over the background; children paint over the hatch.
    // The child (3 cells wide, blue) is not hatched: its blanks stay blank.
    assert!(row(&text, 4).starts_with("x  ╳╳╳╳╳"), "{text}");
    assert_eq!(c.get(1, 4).unwrap().bg.hex(), "#0000FF");
    let hatched = c.get(3, 4).unwrap();
    assert_eq!(hatched.symbol, "╳");
    assert!(
        ["#7F7F7F", "#808080"].contains(&hatched.fg.hex().as_str()),
        "{}",
        hatched.fg.hex()
    );
    assert!(row(&text, 5).starts_with("╳╳╳╳╳╳╳╳"), "{text}");

    // `hatch: none` overrides an earlier rule.
    assert_eq!(row(&text, 7), "plain", "{text}");
    assert_eq!(c.get(6, 7).unwrap().symbol, " ");
}

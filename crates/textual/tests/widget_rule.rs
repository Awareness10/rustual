//! Rule: horizontal / vertical, line styles.

use textual::prelude::*;

struct H(LineStyle);
impl Widget for H {
    fn compose(&mut self) -> Vec<Child> {
        compose![
            Label::new("above"),
            Rule::horizontal().line_style(self.0).id("rule"),
            Label::new("below")
        ]
    }
}

#[tokio::test]
async fn horizontal_rule_with_margins() {
    let mut pilot = App::new(H(LineStyle::Solid)).test(Size::new(20, 6)).await;
    assert!(pilot.css_errors().is_empty(), "{:?}", pilot.css_errors());
    let text = pilot.screen_text();
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines[0], "above");
    assert_eq!(lines[1], "");
    assert_eq!(lines[2], "─".repeat(20));
    assert_eq!(lines[3], "");
    assert_eq!(lines[4], "below");
    // Rule color is $secondary (differs from the default text color).
    let c = pilot.canvas();
    assert_ne!(c.get(0, 2).unwrap().fg, c.get(0, 0).unwrap().fg);
}

#[tokio::test]
async fn line_styles() {
    for (style, ch) in [
        (LineStyle::Heavy, "━"),
        (LineStyle::Double, "═"),
        (LineStyle::Dashed, "╍"),
        (LineStyle::Thick, "█"),
        (LineStyle::Ascii, "-"),
    ] {
        let mut pilot = App::new(H(style)).test(Size::new(10, 6)).await;
        let text = pilot.screen_text();
        assert_eq!(text.lines().nth(2).unwrap(), ch.repeat(10), "{style:?}");
    }
    let mut pilot = App::new(H(LineStyle::Hidden)).test(Size::new(10, 6)).await;
    assert_eq!(pilot.screen_text().lines().nth(2).unwrap(), "");
}

struct V;
impl Widget for V {
    fn compose(&mut self) -> Vec<Child> {
        compose![Horizontal::new().with(compose![
            Label::new("L"),
            Rule::vertical().line_style(LineStyle::Heavy),
            Label::new("R"),
        ])]
    }
}

#[tokio::test]
async fn vertical_rule() {
    let mut pilot = App::new(V).test(Size::new(12, 3)).await;
    let text = pilot.screen_text();
    let lines: Vec<&str> = text.lines().collect();
    // "L", margin 2, rule, margin 2, "R"
    assert_eq!(lines[0], "L  ┃  R", "{text}");
    assert_eq!(lines[1], "   ┃");
    assert_eq!(lines[2], "   ┃");
}

#[test]
fn parse_line_style() {
    assert_eq!(LineStyle::parse("double"), Some(LineStyle::Double));
    assert_eq!(LineStyle::parse("bogus"), None);
    assert_eq!(LineStyle::Solid.vertical_char(), "│");
}

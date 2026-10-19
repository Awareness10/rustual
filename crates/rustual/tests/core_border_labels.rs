//! Border titles and subtitles: default alignment and `auto` colors (Textual's
//! `border-title-align: left`, `border-subtitle-align: right`, `auto_border_*_color`).

use rustual::prelude::*;

struct Panel;
impl Widget for Panel {
    fn compose(&mut self) -> Vec<Child> {
        compose![Vertical::new()
            .id("panel")
            .with(compose![Static::new("body")])
            .border_title("Title")
            .border_subtitle("Sub")]
    }
}

async fn pilot(css: &'static str) -> Pilot {
    let mut p = App::new(Panel).css(css).test(Size::new(30, 5)).await;
    assert!(p.css_errors().is_empty(), "{:?}", p.css_errors());
    p.pause().await;
    p
}

fn find(p: &mut Pilot, needle: &str) -> (i32, i32) {
    let text = p.screen_text();
    let (y, line) = text
        .lines()
        .enumerate()
        .find(|(_, l)| l.contains(needle))
        .unwrap_or_else(|| panic!("{needle:?} not on screen:\n{text}"));
    (
        line[..line.find(needle).unwrap()].chars().count() as i32,
        y as i32,
    )
}

#[tokio::test]
async fn subtitles_default_to_the_right_titles_to_the_left() {
    let mut p = pilot("#panel { border: round red; height: 5; }").await;
    let text = p.screen_text();
    let lines: Vec<&str> = text.lines().collect();
    assert!(lines[0].starts_with("╭─ Title "), "{text}");
    assert!(lines[4].ends_with(" Sub ─╯"), "{text}");

    let mut p =
        pilot("#panel { border: round red; height: 5; border-subtitle-align: left; }").await;
    let text = p.screen_text();
    assert!(
        text.lines().nth(4).unwrap().starts_with("╰─ Sub "),
        "{text}"
    );
}

#[tokio::test]
async fn auto_label_colors_contrast_with_the_background() {
    // `$text-muted` is `auto 60%`: visible text, not the background color.
    let mut p = pilot(
        "Screen { background: #101010; } \
         #panel { border: round red; height: 5; background: #101010; \
                  border-title-color: auto; border-subtitle-color: $text-muted; }",
    )
    .await;
    let bg = rustual::Color::parse("#101010").unwrap();
    for label in ["Title", "Sub"] {
        let (x, y) = find(&mut p, label);
        let cell = p.canvas().get(x, y).unwrap().clone();
        assert_ne!(cell.fg, bg, "{label} is invisible");
        assert_ne!(cell.fg, cell.bg, "{label} is invisible");
    }
    let (x, y) = find(&mut p, "Title");
    assert_eq!(
        p.canvas().get(x, y).unwrap().fg,
        rustual::Color::parse("#ffffff").unwrap()
    );
}

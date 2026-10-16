//! `Syntax` layout compared with real `rich` (goldens from `scripts/rich_golden.py syntax`,
//! rendered with the plain "text" lexer, so only layout is compared, not token colors).

use rustual::renderables::{render_lines, Panel, RenderOptions, Renderable, Syntax};
use rustual::Spacing;

include!("golden/syntax.rs");

const CODE: &str =
    "def hello(name):\n    print(f'hello {name}, this line is fairly long')\n\nhello('world')";

fn check(name: &str, r: &dyn Renderable, width: i32, expected: &[&str]) {
    let lines: Vec<String> = render_lines(r, &RenderOptions::new(width))
        .iter()
        .map(|l| l.plain_text())
        .collect();
    assert_eq!(
        lines,
        expected,
        "{name}:\n--- rustual\n{}\n--- rich\n{}",
        lines.join("\n"),
        expected.join("\n")
    );
}

#[test]
fn syntax_layout_matches_rich() {
    let s = |code: &str| Syntax::new(code, "text");
    check("plain", &s(CODE), 30, SYNTAX_PLAIN_30);
    check(
        "numbers",
        &s(CODE).line_numbers(true),
        40,
        SYNTAX_NUMBERS_40,
    );
    check(
        "wrap",
        &s(CODE).line_numbers(true).word_wrap(true),
        32,
        SYNTAX_WRAP_32,
    );
    check(
        "highlight",
        &s(CODE)
            .line_numbers(true)
            .highlight_lines([10])
            .start_line(9),
        40,
        SYNTAX_HIGHLIGHT_40,
    );
    check(
        "trailing_newline",
        &s("a\nb\n").line_numbers(true),
        10,
        SYNTAX_TRAILING_NEWLINE_10,
    );
    check(
        "trailing_newline_plain",
        &s("a\nb\n"),
        10,
        SYNTAX_TRAILING_NEWLINE_PLAIN_10,
    );
    check(
        "padding",
        &s("x = 1\ty").padding(Spacing::symmetric(1, 2)),
        16,
        SYNTAX_PADDING_16,
    );
    check(
        "dedent",
        &s("    a\n      b\n    c").dedent(true),
        10,
        SYNTAX_DEDENT_10,
    );
    check(
        "in_panel",
        &Panel::fit(s("x = 1").line_numbers(true)),
        30,
        SYNTAX_IN_PANEL_30,
    );
}

#[test]
fn syntax_highlights_known_languages() {
    let lines = render_lines(
        &Syntax::new("fn main() {}", "rust"),
        &RenderOptions::new(20),
    );
    let keyword = lines[0].spans.iter().find(|s| s.text == "fn").unwrap();
    let rest = lines[0]
        .spans
        .iter()
        .find(|s| s.text.contains("main"))
        .unwrap();
    // Keywords get their own color; everything has the theme background (monokai).
    assert_ne!(keyword.style.fg, rest.style.fg);
    let bg = rustual::Color::parse("#272822").ok();
    assert!(lines[0].spans.iter().all(|s| s.style.bg == bg));
}

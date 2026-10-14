//! Pretty tests.

use textual::prelude::*;
use textual::render::Attrs;

#[derive(Debug)]
#[allow(dead_code)]
struct Config {
    name: &'static str,
    retries: u32,
    verbose: bool,
    tags: Vec<&'static str>,
}

fn config() -> Config {
    Config {
        name: "demo",
        retries: 3,
        verbose: true,
        tags: vec!["a", "b"],
    }
}

struct PApp;

impl Widget for PApp {
    fn compose(&mut self) -> Vec<Child> {
        compose![Pretty::new(&config()).id("p")]
    }
}

fn find(text: &str, needle: &str) -> (i32, i32) {
    for (y, line) in text.lines().enumerate() {
        let chars: Vec<char> = line.chars().collect();
        let n: Vec<char> = needle.chars().collect();
        if let Some(x) = (0..chars.len()).find(|i| chars[*i..].starts_with(&n)) {
            return (x as i32, y as i32);
        }
    }
    panic!("{needle:?} not in {text}");
}

#[tokio::test]
async fn compact_when_it_fits_and_highlighted() {
    let mut p = App::new(PApp).test(Size::new(100, 10)).await;
    assert!(p.css_errors().is_empty(), "{:?}", p.css_errors());
    let text = p.screen_text();
    assert!(
        text.lines().next().unwrap().starts_with(
            "Config { name: \"demo\", retries: 3, verbose: true, tags: [\"a\", \"b\"] }"
        ),
        "{text}"
    );
    let c = p.canvas();
    let plain = c.get(find(&text, ", ").0, 0).unwrap().fg;
    let s = find(&text, "\"demo\"");
    let num = find(&text, "3,");
    let field = find(&text, "retries");
    let b = find(&text, "true");
    assert_ne!(c.get(s.0, s.1).unwrap().fg, plain);
    assert_ne!(c.get(num.0, num.1).unwrap().fg, plain);
    assert!(c.get(num.0, num.1).unwrap().attrs.contains(Attrs::BOLD));
    assert_ne!(c.get(field.0, field.1).unwrap().fg, plain);
    assert!(c.get(b.0, b.1).unwrap().attrs.contains(Attrs::ITALIC));
    let brace = find(&text, "{");
    assert!(c.get(brace.0, brace.1).unwrap().attrs.contains(Attrs::BOLD));
}

#[tokio::test]
async fn expands_when_narrow_and_updates() {
    let mut p = App::new(PApp).test(Size::new(30, 20)).await;
    let text = p.screen_text();
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines[0].trim_end(), "Config {", "{text}");
    assert_eq!(lines[1].trim_end(), "    name: \"demo\",", "{text}");
    assert!(text.contains("    tags: ["), "{text}");
    p.with::<Pretty, _>("#p", |w, _ctx| w.update(&vec![1, 2, 3]))
        .await;
    let text = p.screen_text();
    assert!(text.starts_with("[1, 2, 3]"), "{text}");
    // Height is auto: one line now.
    assert!(text.lines().nth(1).unwrap().trim().is_empty());
}

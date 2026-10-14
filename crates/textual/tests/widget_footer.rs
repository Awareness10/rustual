//! Footer: renders active bindings; pressing the mouse on an item runs its binding.

use textual::prelude::*;
use textual::render::Attrs;

#[derive(Default)]
struct Focusable {
    hits: usize,
}
impl Widget for Focusable {
    fn type_names(&self) -> &'static [&'static str] {
        &["Focusable"]
    }
    fn default_css(&self) -> &'static str {
        "Focusable { height: 1; }"
    }
    fn can_focus(&self) -> bool {
        true
    }
    fn bindings(&self) -> Vec<Binding> {
        vec![Binding::new("x", "hit", "Hit me")]
    }
    fn run_action(&mut self, a: &str, _: &[String], _ctx: &mut Ctx) -> bool {
        if a == "hit" {
            self.hits += 1;
            return true;
        }
        false
    }
    fn render(&self, _ctx: &RenderCtx) -> Content {
        Content::plain(&format!("hits={}", self.hits))
    }
}

#[derive(Default)]
struct T {
    log: Vec<String>,
    compact: bool,
}
impl Widget for T {
    fn compose(&mut self) -> Vec<Child> {
        compose![
            Focusable::default().id("f"),
            Footer::new().compact(self.compact).id("footer")
        ]
    }
    fn bindings(&self) -> Vec<Binding> {
        vec![
            Binding::new("q", "quit", "Quit"),
            Binding::new("d", "mark", "Mark"),
            Binding::new("h", "mark", "Hidden").hidden(),
        ]
    }
    fn run_action(&mut self, a: &str, _: &[String], _ctx: &mut Ctx) -> bool {
        if a == "mark" {
            self.log.push("mark".into());
            return true;
        }
        false
    }
}

fn footer_line(pilot: &mut Pilot) -> String {
    pilot.screen_text().lines().last().unwrap().to_string()
}

#[tokio::test]
async fn footer_renders_bindings() {
    let mut pilot = App::new(T::default()).test(Size::new(60, 5)).await;
    assert!(pilot.css_errors().is_empty(), "{:?}", pilot.css_errors());
    let line = footer_line(&mut pilot);
    assert_eq!(line, " q Quit  d Mark  x Hit me", "{line:?}");
    // Key is bold in $footer-key-foreground, the description is not.
    let c = pilot.canvas();
    let key = c.get(1, 4).unwrap();
    let desc = c.get(3, 4).unwrap();
    assert!(key.attrs.contains(Attrs::BOLD));
    assert!(!desc.attrs.contains(Attrs::BOLD));
    assert_ne!(key.fg, desc.fg);
    // Footer background ($footer-background) spans the whole row, differing from the screen.
    assert_eq!(key.bg, c.get(59, 4).unwrap().bg);
    assert_ne!(key.bg, c.get(59, 2).unwrap().bg);
}

#[tokio::test]
async fn compact_footer_has_less_whitespace() {
    let mut pilot = App::new(T {
        compact: true,
        ..Default::default()
    })
    .test(Size::new(60, 5))
    .await;
    assert_eq!(footer_line(&mut pilot), "q Quit d Mark x Hit me");
}

#[tokio::test]
async fn clicking_items_runs_bindings() {
    let mut pilot = App::new(T::default()).test(Size::new(60, 5)).await;
    // " d Mark " spans x = 8..16 → non-namespaced app action.
    pilot.click_at(10, 4).await;
    assert_eq!(pilot.app::<T>().unwrap().log, vec!["mark"]);
    // A binding of the focused widget.
    pilot.click_at(20, 4).await;
    assert_eq!(pilot.query_one::<Focusable>("#f").unwrap().hits, 1);
    // Past the last item: nothing.
    pilot.click_at(50, 4).await;
    assert!(!pilot.has_exited());
    pilot.click_at(2, 4).await;
    assert!(pilot.has_exited());
}

#[tokio::test]
async fn hovered_item_is_highlighted() {
    let mut pilot = App::new(T::default()).test(Size::new(60, 5)).await;
    let before = pilot.canvas().get(10, 4).unwrap().bg;
    pilot.click_at(10, 4).await; // moves the mouse onto "d Mark"
    let c = pilot.canvas();
    assert_ne!(c.get(10, 4).unwrap().bg, before);
    assert_eq!(c.get(2, 4).unwrap().bg, before);
    pilot.hover("#f").await;
    assert_eq!(pilot.canvas().get(10, 4).unwrap().bg, before);
}

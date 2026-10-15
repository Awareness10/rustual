//! Tests for `Collapsible`.

use rustual::prelude::*;
use rustual::widgets::collapsible::{self, CollapsibleTitle};

#[derive(Default)]
struct CApp {
    log: Vec<String>,
}

impl Widget for CApp {
    fn compose(&mut self) -> Vec<Child> {
        compose![
            Collapsible::new("First")
                .id("c1")
                .with(compose![Label::new("one body"), Label::new("more")]),
            Collapsible::new("Second")
                .collapsed(false)
                .id("c2")
                .with(compose![
                    Label::new("two body"),
                    Collapsible::new("Inner")
                        .id("inner")
                        .with(compose![Label::new("inner body")]),
                ]),
        ]
    }
    fn on_message(&mut self, msg: &Message, _ctx: &mut Ctx) -> Flow {
        if let Some(e) = msg.get::<collapsible::Expanded>() {
            self.log.push(format!("expanded {:?}", e.collapsible_id));
        }
        if let Some(c) = msg.get::<collapsible::Collapsed>() {
            self.log.push(format!("collapsed {:?}", c.collapsible_id));
        }
        Flow::Continue
    }
}

async fn pilot() -> Pilot {
    App::new(CApp::default()).test(Size::new(40, 20)).await
}

fn log(p: &Pilot) -> Vec<String> {
    p.app::<CApp>().unwrap().log.clone()
}

fn collapsed(p: &Pilot, sel: &str) -> bool {
    p.query_one::<Collapsible>(sel).unwrap().is_collapsed()
}

#[tokio::test]
async fn renders_titles_and_contents() {
    let mut p = pilot().await;
    assert!(p.css_errors().is_empty(), "{:?}", p.css_errors());
    let text = p.screen_text();
    assert!(text.contains("▶ First"), "{text}");
    assert!(!text.contains("one body"), "{text}");
    assert!(text.contains("▼ Second"), "{text}");
    assert!(text.contains("two body"), "{text}");
    assert!(text.contains("▶ Inner"), "{text}");
    assert!(!text.contains("inner body"), "{text}");
    // Children were moved into the Contents container.
    assert_eq!(p.query("#c1 > Contents > Label").len(), 2);
    assert!(log(&p).is_empty(), "no messages at start");
    let c2 = p.query("#c2")[0];
    assert!(!p.dom().get(c2).unwrap().has_class("-collapsed"));
}

#[tokio::test]
async fn click_title_toggles() {
    let mut p = pilot().await;
    let title = p.query("#c1 > CollapsibleTitle")[0];
    let r = p.dom().get(title).unwrap().layout.region;
    p.click_at(r.x + 1, r.y).await;
    assert!(!collapsed(&p, "#c1"));
    let text = p.screen_text();
    assert!(
        text.contains("▼ First") && text.contains("one body"),
        "{text}"
    );
    p.click_at(r.x + 1, r.y).await;
    assert!(collapsed(&p, "#c1"));
    assert_eq!(
        log(&p),
        vec!["expanded Some(\"c1\")", "collapsed Some(\"c1\")"]
    );
}

#[tokio::test]
async fn enter_on_focused_title_toggles_nested_only() {
    let mut p = pilot().await;
    // Focus order: First title, Second title, Inner title.
    p.press(&["tab", "tab"]).await;
    let focused = p.focused().unwrap();
    assert!(p.dom().widget::<CollapsibleTitle>(focused).is_some());
    p.press(&["enter"]).await;
    assert!(!collapsed(&p, "#inner"));
    assert!(!collapsed(&p, "#c2"), "outer untouched");
    assert!(p.screen_text().contains("inner body"));
    assert_eq!(log(&p), vec!["expanded Some(\"inner\")"]);
}

#[tokio::test]
async fn programmatic_toggle_and_title() {
    let mut p = pilot().await;
    p.with::<Collapsible, _>("#c2", |c, ctx| {
        c.set_collapsed(true, ctx);
        c.set_title("Renamed", ctx);
    })
    .await;
    let text = p.screen_text();
    assert!(text.contains("▶ Renamed"), "{text}");
    assert!(!text.contains("two body"));
    // Setting the same state again posts nothing.
    p.with::<Collapsible, _>("#c2", |c, ctx| c.set_collapsed(true, ctx))
        .await;
    assert_eq!(log(&p), vec!["collapsed Some(\"c2\")"]);
}

#[tokio::test]
async fn custom_symbols() {
    struct S;
    impl Widget for S {
        fn compose(&mut self) -> Vec<Child> {
            compose![Collapsible::new("X")
                .collapsed_symbol("+")
                .expanded_symbol("-")]
        }
    }
    let mut p = App::new(S).test(Size::new(20, 5)).await;
    assert!(p.screen_text().contains("+ X"));
}

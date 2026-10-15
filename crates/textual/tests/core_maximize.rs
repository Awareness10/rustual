//! Maximize / minimize (Textual: `Screen.maximize`, `action_maximize`, escape to minimize).

use textual::prelude::*;
use textual::widgets::command_palette::CommandPalette;

/// A container that may be maximized (like Textual's `ALLOW_MAXIMIZE = True` containers).
struct Panel;

impl Widget for Panel {
    fn type_names(&self) -> &'static [&'static str] {
        &["Panel"]
    }
    fn default_css(&self) -> &'static str {
        "Panel { height: auto; border: round $primary; }"
    }
    fn allow_maximize(&self) -> Option<bool> {
        Some(true)
    }
}

struct Root;

impl Widget for Root {
    fn compose(&mut self) -> Vec<Child> {
        compose![
            Label::new("top label").id("top"),
            Input::new().id("a").style("width: 20;"),
            Child::from(Panel)
                .id("panel")
                .with([Input::new().id("inner")]),
            Input::new().id("b"),
            Footer::new(),
        ]
    }
    fn bindings(&self) -> Vec<Binding> {
        vec![
            Binding::new("f2", "maximize", "Maximize"),
            Binding::new("f3", "minimize", "Minimize"),
        ]
    }
}

async fn pilot() -> Pilot {
    App::new(Root).test(Size::new(40, 16)).await
}

fn visible(p: &Pilot, sel: &str) -> bool {
    let id = p.query(sel)[0];
    p.dom().get(id).unwrap().layout.visible
}

#[tokio::test]
async fn maximize_action_fills_the_screen_with_the_focused_widget() {
    let mut p = pilot().await;
    assert_eq!(p.focused_id().as_deref(), Some("a"));
    p.press(&["f2"]).await;
    let a = p.query("#a")[0];
    assert_eq!(p.maximized(), Some(a));
    assert!(!visible(&p, "#top"));
    assert!(!visible(&p, "#b"));
    assert!(visible(&p, "Footer"));
    let node = p.dom().get(a).unwrap();
    assert!(node.has_class("-maximized"));
    // Centered (align: center middle) in the area above the footer.
    let r = node.layout.region;
    assert_eq!(r.width, 20);
    assert_eq!(r.x, 10);
    assert!(r.y > 3, "{r:?}");
    let screen = p.active_screen().unwrap();
    assert!(p.dom().get(screen).unwrap().has_class("-maximized-view"));
    // The focus chain is limited to the maximized widget.
    p.press(&["tab"]).await;
    assert_eq!(p.focused_id().as_deref(), Some("a"));
    // Escape minimizes (before any binding sees the key).
    p.press(&["escape"]).await;
    assert_eq!(p.maximized(), None);
    assert!(visible(&p, "#top") && visible(&p, "#b"));
    assert!(!p.dom().get(a).unwrap().has_class("-maximized"));
    assert!(!p.dom().get(screen).unwrap().has_class("-maximized-view"));
    assert!(p.css_errors().is_empty(), "{:?}", p.css_errors());
}

#[tokio::test]
async fn maximize_prefers_a_maximizable_container() {
    let mut p = pilot().await;
    let inner = p.query("#inner")[0];
    let panel = p.query("#panel")[0];
    let ok = p
        .with::<Root, _>("App", |_, ctx| ctx.maximize(inner))
        .await
        .unwrap();
    assert!(ok);
    assert_eq!(p.maximized(), Some(panel));
    assert!(visible(&p, "#inner"));
    assert!(!visible(&p, "#a"));
    p.press(&["f3"]).await;
    assert_eq!(p.maximized(), None);
    p.with::<Root, _>("App", |_, ctx| ctx.maximize_exact(inner))
        .await;
    assert_eq!(p.maximized(), Some(inner));
    // A label can't focus, so it can't be maximized.
    let top = p.query("#top")[0];
    let ok = p
        .with::<Root, _>("App", |_, ctx| {
            ctx.minimize();
            ctx.maximize(top)
        })
        .await
        .unwrap();
    assert!(!ok);
    assert_eq!(p.maximized(), None);
}

#[tokio::test]
async fn escape_to_minimize_can_be_disabled() {
    let mut p = App::new(Root)
        .escape_to_minimize(false)
        .test(Size::new(40, 16))
        .await;
    p.press(&["f2"]).await;
    assert!(p.maximized().is_some());
    p.press(&["escape"]).await;
    assert!(p.maximized().is_some());
}

#[tokio::test]
async fn palette_offers_maximize_and_minimize() {
    let mut p = pilot().await;
    p.press(&["ctrl+p"]).await;
    let hits: Vec<String> = p
        .query_one::<CommandPalette>("CommandPalette")
        .map(|c| c.hit_names().into_iter().map(String::from).collect())
        .unwrap();
    assert!(hits.contains(&"Maximize".to_string()), "{hits:?}");
    assert!(hits.contains(&"Screenshot".to_string()), "{hits:?}");
    p.type_text("maximize").await;
    p.press(&["enter"]).await;
    assert_eq!(p.screen_count(), 1);
    assert_eq!(p.maximized(), Some(p.query("#a")[0]));
    p.press(&["ctrl+p"]).await;
    let hits: Vec<String> = p
        .query_one::<CommandPalette>("CommandPalette")
        .map(|c| c.hit_names().into_iter().map(String::from).collect())
        .unwrap();
    assert!(hits.contains(&"Minimize".to_string()), "{hits:?}");
    assert!(!hits.contains(&"Maximize".to_string()), "{hits:?}");
}

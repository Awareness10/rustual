//! Tests for `TabbedContent`, `TabPane` and `ContentSwitcher`.

use rustual::prelude::*;
use rustual::widgets::{tabbed_content, tabs};

#[derive(Default)]
struct TcApp {
    log: Vec<String>,
    initial: String,
    titles: bool,
}

impl Widget for TcApp {
    fn compose(&mut self) -> Vec<Child> {
        if self.titles {
            return compose![TabbedContent::new()
                .titles(["Left", "Right"])
                .id("tc")
                .with(compose![Label::new("left body"), Label::new("right body")])];
        }
        compose![TabbedContent::new()
            .initial(self.initial.clone())
            .id("tc")
            .with(compose![
                TabPane::new("Alpha")
                    .id("a")
                    .with(compose![Label::new("alpha body").id("la")]),
                TabPane::new("Beta")
                    .id("b")
                    .with(compose![Button::new("Beta button").id("bb")]),
                TabPane::new("Gamma").with(compose![Label::new("gamma body")]),
            ])]
    }
    fn on_message(&mut self, msg: &Message, _ctx: &mut Ctx) -> Flow {
        if let Some(a) = msg.get::<tabbed_content::TabActivated>() {
            self.log.push(format!("pane {} {}", a.pane_id, a.index));
        }
        if msg.is::<tabs::TabActivated>() {
            self.log.push("leaked tabs message".into());
        }
        if msg.is::<tabbed_content::Cleared>() {
            self.log.push("cleared".into());
        }
        Flow::Continue
    }
}

async fn pilot(a: TcApp) -> Pilot {
    App::new(a).test(Size::new(40, 10)).await
}

fn log(p: &Pilot) -> Vec<String> {
    p.app::<TcApp>().unwrap().log.clone()
}

fn active(p: &Pilot) -> String {
    p.query_one::<TabbedContent>("#tc")
        .unwrap()
        .active()
        .to_string()
}

#[tokio::test]
async fn composes_tabs_and_shows_first_pane() {
    let mut p = pilot(TcApp::default()).await;
    assert!(p.css_errors().is_empty(), "{:?}", p.css_errors());
    let text = p.screen_text();
    assert!(text.starts_with(" Alpha  Beta  Gamma"), "{text}");
    assert!(text.contains("alpha body"), "{text}");
    assert!(!text.contains("Beta button"), "{text}");
    assert!(!text.contains("gamma body"), "{text}");
    assert_eq!(active(&p), "a");
    assert_eq!(log(&p), vec!["pane a 0"]);
    // Panes were moved into the switcher; tabs have prefixed ids.
    assert_eq!(p.query("ContentSwitcher > TabPane").len(), 3);
    assert_eq!(p.query("ContentTabs Tab").len(), 3);
    assert_eq!(p.query("#tab-1").len(), 1, "auto pane id");
}

#[tokio::test]
async fn clicking_and_keys_switch_panes() {
    let mut p = pilot(TcApp::default()).await;
    let beta_tab = p.query("ContentTabs Tab")[1];
    let r = p.dom().get(beta_tab).unwrap().layout.region;
    p.click_at(r.x + 1, r.y).await;
    assert_eq!(active(&p), "b");
    let text = p.screen_text();
    assert!(text.contains("Beta button"), "{text}");
    assert!(!text.contains("alpha body"), "{text}");
    p.press(&["right"]).await;
    assert_eq!(active(&p), "tab-1");
    assert!(p.screen_text().contains("gamma body"));
    assert_eq!(log(&p), vec!["pane a 0", "pane b 1", "pane tab-1 2"]);
}

#[tokio::test]
async fn initial_and_set_active() {
    let mut p = pilot(TcApp {
        initial: "b".into(),
        ..Default::default()
    })
    .await;
    assert_eq!(active(&p), "b");
    assert_eq!(log(&p), vec!["pane b 1"]);
    p.with::<TabbedContent, _>("#tc", |t, ctx| assert!(t.set_active("a", ctx)))
        .await;
    assert_eq!(active(&p), "a");
    assert!(p.screen_text().contains("alpha body"));
    // Setting the same pane again posts nothing.
    p.with::<TabbedContent, _>("#tc", |t, ctx| t.set_active("a", ctx))
        .await;
    assert_eq!(log(&p), vec!["pane b 1", "pane a 0"]);
    // Tab key focus never reaches widgets in hidden panes.
    p.press(&["tab"]).await;
    assert_ne!(p.focused_id().as_deref(), Some("bb"));
}

#[tokio::test]
async fn add_and_remove_panes() {
    let mut p = pilot(TcApp::default()).await;
    p.with::<TabbedContent, _>("#tc", |t, ctx| {
        t.add_pane(
            TabPane::new("Delta")
                .id("d")
                .with(compose![Label::new("delta body")]),
            ctx,
        );
        assert_eq!(t.tab_count(ctx), 4);
    })
    .await;
    assert!(p.screen_text().contains("Delta"));
    p.with::<TabbedContent, _>("#tc", |t, ctx| t.set_active("d", ctx))
        .await;
    assert!(p.screen_text().contains("delta body"));
    p.with::<TabbedContent, _>("#tc", |t, ctx| t.remove_pane("d", ctx))
        .await;
    assert_eq!(active(&p), "tab-1", "previous pane activated");
    assert!(!p.screen_text().contains("Delta"));
    p.with::<TabbedContent, _>("#tc", |t, ctx| t.clear_panes(ctx))
        .await;
    assert_eq!(active(&p), "");
    assert_eq!(log(&p).last().unwrap(), "cleared");
    assert!(!log(&p).contains(&"leaked tabs message".to_string()));
}

#[tokio::test]
async fn disable_and_hide_tabs() {
    let mut p = pilot(TcApp::default()).await;
    p.with::<TabbedContent, _>("#tc", |t, ctx| {
        t.disable_tab("b", ctx);
    })
    .await;
    let pane_b = p.query("#b")[0];
    assert!(p.dom().get(pane_b).unwrap().disabled, "pane disabled too");
    p.press(&["right"]).await;
    assert_eq!(active(&p), "tab-1");
    p.with::<TabbedContent, _>("#tc", |t, ctx| {
        t.enable_tab("b", ctx);
        t.hide_tab("tab-1", ctx);
    })
    .await;
    assert!(!p.screen_text().contains("Gamma"));
    assert_eq!(active(&p), "b");
}

#[tokio::test]
async fn plain_children_are_wrapped_with_titles() {
    let mut p = pilot(TcApp {
        titles: true,
        ..Default::default()
    })
    .await;
    let text = p.screen_text();
    assert!(text.starts_with(" Left  Right"), "{text}");
    assert!(text.contains("left body"));
    assert!(!text.contains("right body"));
    p.press(&["right"]).await;
    assert!(p.screen_text().contains("right body"));
    assert_eq!(p.query("TabPane").len(), 2);
}

// ---------------------------------------------------------------- ContentSwitcher

struct SwApp;

impl Widget for SwApp {
    fn compose(&mut self) -> Vec<Child> {
        compose![ContentSwitcher::new()
            .initial("one")
            .id("sw")
            .with(compose![
                Label::new("first").id("one"),
                Label::new("second").id("two"),
                Label::new("anonymous"),
            ])]
    }
}

#[tokio::test]
async fn content_switcher_shows_only_current() {
    let mut p = App::new(SwApp).test(Size::new(30, 5)).await;
    assert!(p.css_errors().is_empty(), "{:?}", p.css_errors());
    let text = p.screen_text();
    assert!(text.contains("first") && !text.contains("second") && !text.contains("anonymous"));
    p.with::<ContentSwitcher, _>("#sw", |s, ctx| s.set_current(Some("two"), ctx))
        .await;
    let text = p.screen_text();
    assert!(!text.contains("first") && text.contains("second"), "{text}");
    assert_eq!(
        p.query_one::<ContentSwitcher>("#sw").unwrap().current(),
        Some("two")
    );
    p.with::<ContentSwitcher, _>("#sw", |s, ctx| s.set_current(None, ctx))
        .await;
    assert!(p.screen_text().trim().is_empty());
    p.with::<ContentSwitcher, _>("#sw", |s, ctx| {
        s.add_content(Label::new("third").id("three"), true, ctx)
    })
    .await;
    assert_eq!(p.screen_text().trim(), "third");
}

// ---------------------------------------------------------------- nested widgets (navigator-like)

struct Nav;

impl Widget for Nav {
    fn compose(&mut self) -> Vec<Child> {
        let mut t: Tree<()> = Tree::new("project");
        let root = t.root();
        t.set_expanded(root, true);
        t.add_leaf(root, "file.rs", None);
        compose![TabbedContent::new().id("tc").with(compose![
            TabPane::new("Files").id("f").with(compose![t.id("tree")]),
            TabPane::new("Pick")
                .id("p")
                .with(compose![Select::new([("One", 1)]).id("sel")]),
        ])]
    }
}

const NAV_CSS: &str = "
TabbedContent { height: 1fr; }
TabbedContent ContentSwitcher { height: 1fr; }
TabPane { height: 1fr; }
";

#[tokio::test]
async fn tree_and_select_inside_panes() {
    let mut p = App::new(Nav).css(NAV_CSS).test(Size::new(40, 12)).await;
    assert!(p.css_errors().is_empty(), "{:?}", p.css_errors());
    let text = p.screen_text();
    assert!(
        text.contains("▼ project") && text.contains("└── file.rs"),
        "{text}"
    );
    let tree = p.query("#tree")[0];
    assert!(p.dom().get(tree).unwrap().layout.region.height >= 9);
    p.press(&["right"]).await;
    let text = p.screen_text();
    assert!(
        text.contains("Select") && !text.contains("project"),
        "{text}"
    );
    p.press(&["tab"]).await;
    assert_eq!(p.focused_id().as_deref(), Some("sel"));
    p.press(&["enter", "down", "enter"]).await;
    assert!(p.screen_text().contains("One"));
}

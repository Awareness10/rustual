//! Tests for `Tabs` / `Tab` / `Underline`.

use rustual::prelude::*;
use rustual::widgets::tabs;

#[derive(Default)]
struct TabsApp {
    tabs: Vec<Tab>,
    log: Vec<String>,
    initial: Option<String>,
}

impl Widget for TabsApp {
    fn compose(&mut self) -> Vec<Child> {
        let mut t = Tabs::new(std::mem::take(&mut self.tabs));
        if let Some(a) = self.initial.take() {
            t = t.active(a);
        }
        compose![t.id("tabs"), Label::new("below").id("below")]
    }
    fn on_message(&mut self, msg: &Message, _ctx: &mut Ctx) -> Flow {
        if let Some(a) = msg.get::<tabs::TabActivated>() {
            self.log
                .push(format!("activated {} {} {}", a.tab_id, a.index, a.label));
        }
        if msg.is::<tabs::Cleared>() {
            self.log.push("cleared".into());
        }
        Flow::Continue
    }
}

fn app(labels: &[&str]) -> TabsApp {
    TabsApp {
        tabs: labels.iter().map(|l| Tab::new(*l)).collect(),
        ..Default::default()
    }
}

async fn pilot(a: TabsApp) -> Pilot {
    App::new(a).test(Size::new(40, 6)).await
}

fn log(p: &Pilot) -> Vec<String> {
    p.app::<TabsApp>().unwrap().log.clone()
}

fn active(p: &Pilot) -> String {
    p.query_one::<Tabs>("#tabs")
        .unwrap()
        .active_id()
        .to_string()
}

#[tokio::test]
async fn renders_tabs_and_underline() {
    let mut p = pilot(app(&["Alpha", "Beta", "Gamma"])).await;
    assert!(p.css_errors().is_empty(), "{:?}", p.css_errors());
    let text = p.screen_text();
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines[0], " Alpha  Beta  Gamma", "{text}");
    // The first tab is active: its content (cells 1..6) is highlighted.
    assert_eq!(
        lines[1], "╸━━━━━╺━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━",
        "{text}"
    );
    assert_eq!(lines[2], "below");
    assert_eq!(log(&p), vec!["activated tab-1 0 Alpha"]);
    assert_eq!(active(&p), "tab-1");
}

#[tokio::test(start_paused = true)]
async fn left_right_keys_cycle_tabs() {
    let mut p = pilot(app(&["A", "B", "C"])).await;
    assert_eq!(p.focused_id().as_deref(), Some("tabs"));
    p.press(&["right"]).await;
    assert_eq!(active(&p), "tab-2");
    p.press(&["right", "right"]).await;
    assert_eq!(active(&p), "tab-1", "wraps around");
    p.press(&["left"]).await;
    assert_eq!(active(&p), "tab-3");
    assert_eq!(
        log(&p),
        vec![
            "activated tab-1 0 A",
            "activated tab-2 1 B",
            "activated tab-3 2 C",
            "activated tab-1 0 A",
            "activated tab-3 2 C",
        ]
    );
    // Underline slid (0.3s) under "C" (content cells 7..8).
    p.wait(std::time::Duration::from_millis(300)).await;
    assert_eq!(p.animations_running(), 0);
    let text = p.screen_text();
    let bar: Vec<char> = text.lines().nth(1).unwrap().chars().collect();
    assert_eq!(bar[6], '╸', "{text}");
    assert_eq!(bar[7], '━');
    assert_eq!(bar[8], '╺');
}

#[tokio::test]
async fn click_activates_tab() {
    let mut p = pilot(app(&["One", "Two"])).await;
    assert!(p.click("#tab-2").await);
    assert_eq!(active(&p), "tab-2");
    assert!(p
        .dom()
        .get(p.query("#tab-2")[0])
        .unwrap()
        .has_class("-active"));
    assert!(!p
        .dom()
        .get(p.query("#tab-1")[0])
        .unwrap()
        .has_class("-active"));
    // Clicking the underline below a tab activates it too.
    p.click_at(2, 1).await;
    assert_eq!(active(&p), "tab-1");
    // Clicking the active tab again posts nothing new.
    let n = log(&p).len();
    p.click("#tab-1").await;
    assert_eq!(log(&p).len(), n);
}

#[tokio::test]
async fn active_tab_is_styled_when_focused() {
    let mut p = pilot(app(&["One", "Two"])).await;
    let c = p.canvas();
    let active_bg = c.get(2, 0).unwrap().bg;
    let other_bg = c.get(7, 0).unwrap().bg;
    assert_ne!(active_bg, other_bg);
}

#[tokio::test]
async fn initial_active_and_explicit_ids() {
    let a = TabsApp {
        tabs: vec![Tab::new("X").tab_id("x"), Tab::new("Y").tab_id("y")],
        initial: Some("y".into()),
        ..Default::default()
    };
    let p = pilot(a).await;
    assert_eq!(active(&p), "y");
    assert_eq!(log(&p), vec!["activated y 1 Y"]);
}

#[tokio::test]
async fn add_remove_and_set_active() {
    let mut p = pilot(app(&[])).await;
    assert_eq!(active(&p), "");
    p.with::<Tabs, _>("#tabs", |t, ctx| {
        t.add_tab("First", ctx);
        t.add_tab(Tab::new("Second").tab_id("second"), ctx);
    })
    .await;
    assert_eq!(active(&p), "tab-1", "first added tab becomes active");
    assert!(p.screen_text().contains("First  Second"));
    p.with::<Tabs, _>("#tabs", |t, ctx| assert!(t.set_active("second", ctx)))
        .await;
    assert_eq!(active(&p), "second");
    p.with::<Tabs, _>("#tabs", |t, ctx| assert!(!t.set_active("nope", ctx)))
        .await;
    p.with::<Tabs, _>("#tabs", |t, ctx| t.remove_tab("second", ctx))
        .await;
    assert_eq!(active(&p), "tab-1", "previous tab becomes active");
    assert!(!p.screen_text().contains("Second"));
    p.with::<Tabs, _>("#tabs", |t, ctx| t.remove_tab("tab-1", ctx))
        .await;
    assert_eq!(active(&p), "");
    assert_eq!(log(&p).last().unwrap(), "cleared");
    assert_eq!(
        p.with::<Tabs, _>("#tabs", |t, ctx| t.tab_count(ctx)).await,
        Some(0)
    );
}

#[tokio::test]
async fn disabled_and_hidden_tabs_are_skipped() {
    let mut p = pilot(app(&["A", "B", "C"])).await;
    p.with::<Tabs, _>("#tabs", |t, ctx| {
        t.disable("tab-2", ctx);
    })
    .await;
    p.press(&["right"]).await;
    assert_eq!(active(&p), "tab-3");
    p.with::<Tabs, _>("#tabs", |t, ctx| {
        t.hide("tab-3", ctx);
    })
    .await;
    assert_eq!(
        active(&p),
        "tab-1",
        "hiding the active tab moves to another"
    );
    assert!(!p.screen_text().contains('C'));
    p.with::<Tabs, _>("#tabs", |t, ctx| {
        t.show("tab-3", ctx);
        t.enable("tab-2", ctx);
    })
    .await;
    p.press(&["right"]).await;
    assert_eq!(active(&p), "tab-2");
}

#[tokio::test]
async fn tabs_given_with_children() {
    struct W;
    impl Widget for W {
        fn compose(&mut self) -> Vec<Child> {
            compose![Tabs::default()
                .id("tabs")
                .with(compose![Tab::new("P").id("p"), Tab::new("Q")])]
        }
    }
    let mut p = App::new(W).test(Size::new(30, 4)).await;
    let t = p.query_one::<Tabs>("#tabs").unwrap();
    assert_eq!(t.active_id(), "p");
    assert!(p.screen_text().starts_with(" P  Q"), "{}", p.screen_text());
    assert_eq!(p.query("#tab-1").len(), 1, "auto id for Q");
}

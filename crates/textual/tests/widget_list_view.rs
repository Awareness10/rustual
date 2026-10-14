//! ListView / ListItem tests.

use textual::prelude::*;
use textual::widgets::list_view::{Highlighted, Selected};

#[derive(Default)]
struct LvApp {
    n: usize,
    events: Vec<String>,
}

impl Widget for LvApp {
    fn compose(&mut self) -> Vec<Child> {
        let items: Vec<Child> = (0..self.n)
            .map(|i| {
                ListItem::new()
                    .with(compose![Label::new(format!("item {i}"))])
                    .id(&format!("i{i}"))
                    .disabled(i == 2)
            })
            .collect();
        compose![ListView::new().with(items).id("lv")]
    }
    fn on_message(&mut self, m: &Message, _ctx: &mut Ctx) -> Flow {
        if let Some(h) = m.get::<Highlighted>() {
            self.events
                .push(format!("hl {:?} {:?}", h.index, h.item_id));
        }
        if let Some(s) = m.get::<Selected>() {
            self.events.push(format!("sel {} {:?}", s.index, s.item_id));
        }
        Flow::Continue
    }
}

async fn pilot(n: usize, size: Size) -> Pilot {
    App::new(LvApp { n, events: vec![] }).test(size).await
}

fn events(p: &Pilot) -> Vec<String> {
    p.app::<LvApp>().unwrap().events.clone()
}

fn index(p: &Pilot) -> Option<usize> {
    p.query_one::<ListView>("#lv").unwrap().index()
}

#[tokio::test]
async fn renders_and_highlights_first() {
    let mut p = pilot(5, Size::new(30, 10)).await;
    assert!(p.css_errors().is_empty(), "{:?}", p.css_errors());
    let text = p.screen_text();
    assert!(text.contains("item 0") && text.contains("item 4"), "{text}");
    assert_eq!(index(&p), Some(0));
    assert_eq!(events(&p), vec!["hl Some(0) Some(\"i0\")"]);
    assert_eq!(p.query(".-highlight").len(), 1);
    // Highlighted row uses the focused cursor colors.
    let c = p.canvas();
    assert_ne!(c.get(0, 0).unwrap().bg, c.get(0, 1).unwrap().bg);
}

#[tokio::test]
async fn keyboard_skips_disabled_and_selects() {
    let mut p = pilot(5, Size::new(30, 10)).await;
    p.press(&["down", "down"]).await;
    assert_eq!(index(&p), Some(3), "disabled item 2 is skipped");
    p.press(&["enter"]).await;
    assert!(events(&p).contains(&"sel 3 Some(\"i3\")".to_string()));
    p.press(&["up"]).await;
    assert_eq!(index(&p), Some(1));
    p.press(&["end"]).await;
    assert_eq!(index(&p), Some(4));
    p.press(&["home"]).await;
    assert_eq!(index(&p), Some(0));
    p.press(&["up"]).await;
    assert_eq!(index(&p), Some(0), "no wrapping");
}

#[tokio::test]
async fn click_selects() {
    let mut p = pilot(5, Size::new(30, 10)).await;
    p.click_at(3, 4).await;
    assert_eq!(index(&p), Some(4));
    assert_eq!(p.focused_id().as_deref(), Some("lv"));
    assert!(events(&p).contains(&"sel 4 Some(\"i4\")".to_string()));
}

#[tokio::test]
async fn scrolls_highlight_into_view_and_wheel() {
    let mut p = pilot(50, Size::new(30, 10)).await;
    for _ in 0..15 {
        p.press(&["down"]).await;
    }
    assert_eq!(index(&p), Some(16));
    let text = p.screen_text();
    assert!(text.contains("item 16"), "{text}");
    assert!(!text.contains("item 0\n"), "{text}");
    p.press(&["pagedown"]).await;
    let i = index(&p).unwrap();
    assert!(i >= 24, "{i}");
    assert!(p.screen_text().contains(&format!("item {i}")));
    p.press(&["home"]).await;
    assert!(p.screen_text().contains("item 0"));
    p.scroll_down_at(5, 5).await;
    assert!(!p.screen_text().contains("item 0\n"));
}

#[tokio::test]
async fn append_pop_clear() {
    let mut p = pilot(3, Size::new(30, 10)).await;
    p.with::<ListView, _>("#lv", |lv, ctx| {
        lv.append(ListItem::label("new item").id("new"), ctx);
    })
    .await;
    assert!(p.screen_text().contains("new item"));
    p.with::<ListView, _>("#lv", |lv, ctx| assert_eq!(lv.len(ctx), 4))
        .await;
    // Pop the highlighted first item: the next item becomes highlighted.
    p.with::<ListView, _>("#lv", |lv, ctx| {
        lv.pop(Some(0), ctx);
    })
    .await;
    assert!(!p.screen_text().contains("item 0"));
    assert_eq!(index(&p), Some(0));
    assert_eq!(p.query("#i1.-highlight").len(), 1);
    p.with::<ListView, _>("#lv", |lv, ctx| {
        lv.pop(None, ctx);
    })
    .await;
    assert!(!p.screen_text().contains("new item"));
    p.with::<ListView, _>("#lv", |lv, ctx| lv.clear(ctx)).await;
    assert_eq!(p.query("ListItem").len(), 0);
    assert_eq!(index(&p), None);
    assert!(events(&p).contains(&"hl None None".to_string()));
}

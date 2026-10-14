//! OptionList tests.

use textual::prelude::*;
use textual::widgets::option_list::{OptionHighlighted, OptionSelected};

#[derive(Default)]
struct OlApp {
    n: usize,
    events: Vec<String>,
}

impl Widget for OlApp {
    fn compose(&mut self) -> Vec<Child> {
        let mut ol = OptionList::new()
            .option(OptionItem::new("[b]Bold[/] first").id("first"))
            .option(OptionItem::new("Disabled").id("dis").disabled(true))
            .separator()
            .option("Third");
        for i in 0..self.n {
            ol = ol.option(OptionItem::new(format!("opt {i}")).id(format!("o{i}")));
        }
        compose![ol.id("ol")]
    }
    fn on_message(&mut self, m: &Message, _ctx: &mut Ctx) -> Flow {
        if let Some(h) = m.get::<OptionHighlighted>() {
            self.events
                .push(format!("hl {} {:?}", h.option_index, h.option_id));
        }
        if let Some(s) = m.get::<OptionSelected>() {
            self.events
                .push(format!("sel {} {:?}", s.option_index, s.option_id));
        }
        Flow::Continue
    }
}

async fn pilot(n: usize, size: Size) -> Pilot {
    App::new(OlApp { n, events: vec![] }).test(size).await
}

fn events(p: &Pilot) -> Vec<String> {
    p.app::<OlApp>().unwrap().events.clone()
}

fn highlighted(p: &Pilot) -> Option<usize> {
    p.query_one::<OptionList>("#ol").unwrap().highlighted()
}

#[tokio::test]
async fn renders_options_separator_and_border() {
    let mut p = pilot(0, Size::new(30, 10)).await;
    assert!(p.css_errors().is_empty(), "{:?}", p.css_errors());
    let text = p.screen_text();
    let lines: Vec<&str> = text.lines().collect();
    // tall border at top, padding 0 1.
    assert!(lines[1].contains("Bold first"), "{text}");
    assert!(lines[2].contains("Disabled"), "{text}");
    assert!(lines[3].contains("────"), "{text}");
    assert!(lines[4].contains("Third"), "{text}");
    // Auto height: 4 lines + border.
    assert_eq!(lines.iter().filter(|l| !l.is_empty()).count(), 6, "{text}");
    let c = p.canvas();
    assert!(c
        .get(2, 1)
        .unwrap()
        .attrs
        .contains(textual::render::Attrs::BOLD));
    // Highlighted option has a distinct background; disabled option dimmer fg.
    assert_ne!(c.get(10, 1).unwrap().bg, c.get(10, 4).unwrap().bg);
    assert_ne!(c.get(3, 2).unwrap().fg, c.get(3, 4).unwrap().fg);
}

#[tokio::test]
async fn keyboard_navigation_wraps_and_skips_disabled() {
    let mut p = pilot(0, Size::new(30, 10)).await;
    assert_eq!(highlighted(&p), Some(0));
    p.press(&["down"]).await;
    assert_eq!(highlighted(&p), Some(2));
    p.press(&["down"]).await;
    assert_eq!(highlighted(&p), Some(0), "wraps around");
    p.press(&["up", "enter"]).await;
    assert_eq!(
        events(&p),
        vec![
            "hl 2 None",
            "hl 0 Some(\"first\")",
            "hl 2 None",
            "sel 2 None"
        ]
    );
    p.press(&["home"]).await;
    assert_eq!(highlighted(&p), Some(0));
    p.press(&["end"]).await;
    assert_eq!(highlighted(&p), Some(2));
}

#[tokio::test]
async fn click_selects_and_disabled_ignored() {
    let mut p = pilot(0, Size::new(30, 10)).await;
    p.click_at(5, 4).await;
    assert_eq!(highlighted(&p), Some(2));
    assert!(events(&p).contains(&"sel 2 None".to_string()));
    p.click_at(5, 2).await; // disabled
    assert_eq!(highlighted(&p), Some(2));
    p.click_at(5, 3).await; // separator
    assert_eq!(highlighted(&p), Some(2));
}

#[tokio::test]
async fn hover_highlight() {
    // 6 content lines + border = 8 lines; the center (y 4) is "Third".
    let mut p = pilot(2, Size::new(30, 10)).await;
    let before = p.canvas().get(10, 4).unwrap().bg;
    p.hover("#ol").await;
    let after = p.canvas().get(10, 4).unwrap().bg;
    assert_ne!(before, after);
    assert_eq!(p.canvas().get(10, 5).unwrap().bg, before);
}

#[tokio::test]
async fn scrolling_keeps_highlight_visible_and_wheel() {
    let mut p = pilot(50, Size::new(30, 12)).await;
    // 10 content lines (border takes 2).
    for _ in 0..15 {
        p.press(&["down"]).await;
    }
    let h = highlighted(&p).unwrap();
    let text = p.screen_text();
    assert!(text.contains(&format!("opt {}", h - 3)), "{text}");
    assert!(!text.contains("Bold first"), "{text}");
    p.press(&["pagedown"]).await;
    let h2 = highlighted(&p).unwrap();
    assert!(h2 >= h + 8, "{h} -> {h2}");
    assert!(p
        .screen_text()
        .contains(&format!("opt {} ", h2 - 3).trim_end().to_string()));
    p.press(&["home"]).await;
    assert!(p.screen_text().contains("Bold first"));
    p.scroll_down_at(5, 5).await;
    assert!(!p.screen_text().contains("Bold first"));
}

#[tokio::test]
async fn api_add_get_clear() {
    let mut p = pilot(0, Size::new(30, 10)).await;
    p.with::<OptionList, _>("#ol", |ol, ctx| {
        ol.add_option(OptionItem::new("Added").id("added")).unwrap();
        assert!(ol.add_option(OptionItem::new("dup").id("added")).is_err());
        assert_eq!(ol.get_option("added").unwrap().plain_prompt(), "Added");
        assert_eq!(ol.get_option_index("added").unwrap(), 3);
        assert!(ol.get_option("nope").is_err());
        ol.set_highlighted(Some(3), ctx);
    })
    .await;
    assert!(p.screen_text().contains("Added"));
    assert!(events(&p).contains(&"hl 3 Some(\"added\")".to_string()));
    p.with::<OptionList, _>("#ol", |ol, ctx| ol.clear_options(ctx))
        .await;
    assert_eq!(p.query_one::<OptionList>("#ol").unwrap().option_count(), 0);
    assert!(!p.screen_text().contains("Third"));
}

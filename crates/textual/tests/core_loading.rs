//! The `loading` overlay (Textual: `Widget.loading` / `set_loading`).

use textual::prelude::*;

#[derive(Default)]
struct Root {
    pressed: usize,
}

impl Widget for Root {
    fn compose(&mut self) -> Vec<Child> {
        compose![
            Button::new("OK").id("ok"),
            Input::new().id("inp"),
            Vertical::new()
                .id("box")
                .style("height: 5; border: solid red;")
                .with([Label::new("inside").id("inside")]),
        ]
    }
    fn on_message(&mut self, m: &Message, _ctx: &mut Ctx) -> Flow {
        if m.get::<button::Pressed>().is_some() {
            self.pressed += 1;
        }
        Flow::Continue
    }
}

async fn set_loading(p: &mut Pilot, sel: &str, on: bool) {
    let id = p.query(sel)[0];
    p.with::<Root, _>("App", |_, ctx| ctx.set_loading(id, on))
        .await;
}

#[tokio::test]
async fn loading_covers_a_leaf_widget_and_blocks_input() {
    let mut p = App::new(Root::default()).test(Size::new(30, 12)).await;
    assert_eq!(p.focused_id().as_deref(), Some("ok"));
    let nodes_before = p.dom().len();
    assert!(p.screen_text().contains("OK"));
    set_loading(&mut p, "#ok", true).await;
    let ok = p.query("#ok")[0];
    assert!(p.dom().get(ok).unwrap().loading);
    let ind = p.query("LoadingIndicator");
    assert_eq!(ind.len(), 1);
    let cover = p.dom().get(ind[0]).unwrap();
    assert!(cover.has_class("-textual-loading-indicator"));
    assert_eq!(cover.layout.region, p.dom().get(ok).unwrap().layout.region);
    assert!(cover.layout.visible);
    let text = p.screen_text();
    assert!(!text.contains("OK"), "{text}");
    assert!(text.contains('●'), "{text}");
    // Focus left the loading widget; it is skipped by the focus chain.
    assert_ne!(p.focused_id().as_deref(), Some("ok"));
    p.press(&["tab", "tab", "tab"]).await;
    assert_ne!(p.focused_id().as_deref(), Some("ok"));
    // Clicks hit the indicator, not the button.
    p.click("#ok").await;
    assert_eq!(p.app::<Root>().unwrap().pressed, 0);

    set_loading(&mut p, "#ok", false).await;
    assert!(p.query("LoadingIndicator").is_empty());
    assert_eq!(p.dom().len(), nodes_before);
    assert!(p.screen_text().contains("OK"));
    p.click("#ok").await;
    assert_eq!(p.app::<Root>().unwrap().pressed, 1);
    assert!(p.css_errors().is_empty(), "{:?}", p.css_errors());
}

#[tokio::test]
async fn loading_container_hides_children_and_keeps_its_border() {
    let mut p = App::new(Root::default()).test(Size::new(30, 12)).await;
    set_loading(&mut p, "#box", true).await;
    let boxed = p.query("#box")[0];
    let ind = p.query("LoadingIndicator")[0];
    let box_layout = p.dom().get(boxed).unwrap().layout.clone();
    let cover_region = p.dom().get(ind).unwrap().layout.region;
    assert_eq!(cover_region, box_layout.region.shrink(Spacing::all(1)));
    assert!(!p.dom().get(p.query("#inside")[0]).unwrap().layout.visible);
    let text = p.screen_text();
    assert!(text.contains('┌'), "{text}");
    assert!(!text.contains("inside"), "{text}");
    // The indicator animates (16 fps interval) and is removed with its widget.
    p.with::<Root, _>("App", |_, ctx| ctx.remove(boxed)).await;
    assert!(!p.dom().contains(ind));
    assert!(p.query("LoadingIndicator").is_empty());
}

#[tokio::test]
async fn is_loading_and_idempotent_set() {
    let mut p = App::new(Root::default()).test(Size::new(30, 12)).await;
    set_loading(&mut p, "#inp", true).await;
    set_loading(&mut p, "#inp", true).await;
    assert_eq!(p.query("LoadingIndicator").len(), 1);
    let inp = p.query("#inp")[0];
    let loading = p
        .with::<Root, _>("App", |_, ctx| ctx.is_loading(inp))
        .await
        .unwrap();
    assert!(loading);
}

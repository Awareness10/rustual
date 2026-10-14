use textual::prelude::*;

struct K;
impl Widget for K {
    fn compose(&mut self) -> Vec<Child> {
        compose![Input::new().id("in"), Footer::new()]
    }
    fn bindings(&self) -> Vec<Binding> {
        vec![
            Binding::new("f1", "toggle_keys", "Toggle keys"),
            Binding::new("ctrl+s", "save", "Save file").hidden(),
        ]
    }
    fn on_mount(&mut self, ctx: &mut Ctx) {
        ctx.toggle_key_panel();
    }
}

#[tokio::test]
async fn key_panel_lists_bindings_and_toggles() {
    let mut p = App::new(K).test(Size::new(100, 30)).await;
    assert!(p.css_errors().is_empty(), "{:?}", p.css_errors());
    assert_eq!(p.query("KeyPanel").len(), 1);
    let text = p.screen_text();
    // App bindings (including hidden ones) and the focused Input's bindings.
    assert!(text.contains("Save file"), "{text}");
    assert!(text.contains("f1 Toggle"), "{text}");
    assert!(text.contains("palette"), "{text}");
    assert!(text.contains("shift+left Move"), "{text}");
    assert!(text.contains("tab Focus next"), "{text}");
    // Docked right: the panel is on the right part of the screen.
    let id = p.query("KeyPanel")[0];
    let r = p.dom().get(id).unwrap().layout.region;
    assert_eq!(r.right(), 100);
    assert!(r.width >= 30);

    p.press(&["f1"]).await;
    assert!(p.query("KeyPanel").is_empty());
    assert!(!p.screen_text().contains("Save file"));
    p.press(&["f1"]).await;
    assert_eq!(p.query("KeyPanel").len(), 1);
}

#[tokio::test]
async fn show_and_hide_actions() {
    struct A;
    impl Widget for A {
        fn compose(&mut self) -> Vec<Child> {
            compose![Button::new("x").id("x")]
        }
        fn bindings(&self) -> Vec<Binding> {
            vec![
                Binding::new("s", "app.show_keys", "Show"),
                Binding::new("h", "app.hide_keys", "Hide"),
            ]
        }
    }
    let mut p = App::new(A).test(Size::new(80, 20)).await;
    p.press(&["s"]).await;
    p.press(&["s"]).await;
    assert_eq!(p.query("KeyPanel").len(), 1);
    p.press(&["h"]).await;
    assert!(p.query("KeyPanel").is_empty());
}

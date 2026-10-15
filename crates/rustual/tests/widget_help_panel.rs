use rustual::prelude::*;

/// A focusable widget with help text.
struct Helpful;
impl Widget for Helpful {
    fn type_names(&self) -> &'static [&'static str] {
        &["Helpful"]
    }
    fn default_css(&self) -> &'static str {
        "Helpful { height: 1; }"
    }
    fn can_focus(&self) -> bool {
        true
    }
    fn help(&self) -> Option<&str> {
        Some(
            "
            # Helpful widget

            Press **x** to do the *thing*.
            ",
        )
    }
    fn bindings(&self) -> Vec<Binding> {
        vec![Binding::new("x", "thing", "Do the thing")]
    }
    fn render(&self, _ctx: &RenderCtx) -> Content {
        Content::plain("helpful")
    }
}

/// A container with help; its focusable child has none (help is inherited).
struct Section;
impl Widget for Section {
    fn type_names(&self) -> &'static [&'static str] {
        &["Section"]
    }
    fn default_css(&self) -> &'static str {
        "Section { height: 3; }"
    }
    fn help(&self) -> Option<&str> {
        Some("Section help: **inherited**.")
    }
    fn compose(&mut self) -> Vec<Child> {
        compose![Button::new("Inner").id("inner")]
    }
}

struct App1;
impl Widget for App1 {
    fn compose(&mut self) -> Vec<Child> {
        compose![
            Helpful.id("helpful"),
            Button::new("Plain").id("plain"),
            Section.id("section"),
        ]
    }
    fn bindings(&self) -> Vec<Binding> {
        vec![
            Binding::new("f1", "app.show_help_panel", "Help"),
            Binding::new("f2", "app.hide_help_panel", "Hide help"),
            Binding::new("f3", "app.show_keys", "Keys"),
        ]
    }
}

fn panel_region(p: &rustual::Pilot) -> Region {
    let id = p.query("HelpPanel")[0];
    p.dom().get(id).unwrap().layout.region
}

#[tokio::test]
async fn help_panel_shows_focused_widget_help_and_keys() {
    let mut p = App::new(App1).test(Size::new(100, 30)).await;
    assert_eq!(p.focused_id().as_deref(), Some("helpful"));
    p.press(&["f1"]).await;
    assert!(p.css_errors().is_empty(), "{:?}", p.css_errors());
    assert_eq!(p.query("HelpPanel").len(), 1);
    // The embedded KeyPanel is not docked: it fills the panel below the help.
    assert_eq!(p.query("HelpPanel KeyPanel#keys-help").len(), 1);
    let text = p.screen_text();
    assert!(text.contains("Helpful widget"), "{text}");
    assert!(text.contains("Press x to do the thing"), "{text}");
    assert!(text.contains("x Do the thing"), "{text}");
    assert!(text.contains("f1 Help"), "{text}");

    // Docked right.
    let r = panel_region(&p);
    assert_eq!(r.right(), 100);
    assert_eq!(r.width, 33);
    let keys = p.query("#keys-help")[0];
    let kr = p.dom().get(keys).unwrap().layout.region;
    let help = p.query("#widget-help")[0];
    let hr = p.dom().get(help).unwrap().layout.region;
    assert!(kr.x >= r.x && kr.right() <= r.right(), "{kr:?} in {r:?}");
    assert!(kr.y >= hr.bottom(), "keys below help: {kr:?} {hr:?}");

    // Focus moves to a widget without help: the help block is hidden.
    p.press(&["tab"]).await;
    p.pause().await;
    assert_eq!(p.focused_id().as_deref(), Some("plain"));
    let panel = p.query("HelpPanel")[0];
    assert!(!p.dom().get(panel).unwrap().has_class("-show-help"));
    let text = p.screen_text();
    assert!(!text.contains("Helpful widget"), "{text}");
    assert!(!text.contains("Do the thing"), "{text}");

    // Help is inherited from the nearest ancestor with help.
    p.press(&["tab"]).await;
    p.pause().await;
    assert_eq!(p.focused_id().as_deref(), Some("inner"));
    assert!(p.dom().get(panel).unwrap().has_class("-show-help"));
    let text = p.screen_text();
    assert!(text.contains("Section help: inherited."), "{text}");

    // Back to the first widget.
    p.press(&["shift+tab", "shift+tab"]).await;
    p.pause().await;
    assert!(p.screen_text().contains("Helpful widget"));

    p.press(&["f2"]).await;
    assert!(p.query("HelpPanel").is_empty());
    assert!(p.query("KeyPanel").is_empty());
}

#[tokio::test]
async fn show_keys_still_shows_a_bare_key_panel() {
    let mut p = App::new(App1).test(Size::new(100, 30)).await;
    p.press(&["f3"]).await;
    assert_eq!(p.query("KeyPanel").len(), 1);
    assert!(p.query("HelpPanel").is_empty());
    let id = p.query("KeyPanel")[0];
    assert!(p.dom().get(id).unwrap().has_class("-docked"));
    // The help panel can be shown alongside; show_help_panel is idempotent.
    p.press(&["f1", "f1"]).await;
    assert_eq!(p.query("HelpPanel").len(), 1);
    let embedded = p.query("#keys-help")[0];
    assert!(!p.dom().get(embedded).unwrap().has_class("-docked"));
}

#[tokio::test]
async fn ctx_help_panel_api_and_command_palette() {
    struct T;
    impl Widget for T {
        fn compose(&mut self) -> Vec<Child> {
            compose![Input::new().id("in")]
        }
        fn bindings(&self) -> Vec<Binding> {
            vec![Binding::new("f1", "toggle", "Toggle help")]
        }
        fn run_action(&mut self, action: &str, _args: &[String], ctx: &mut Ctx) -> bool {
            if action == "toggle" {
                if ctx.help_panel_visible() {
                    ctx.hide_help_panel();
                } else {
                    ctx.show_help_panel();
                }
                return true;
            }
            false
        }
    }
    let mut p = App::new(T).test(Size::new(100, 30)).await;
    p.press(&["f1"]).await;
    assert_eq!(p.query("HelpPanel").len(), 1);
    p.press(&["f1"]).await;
    assert!(p.query("HelpPanel").is_empty());

    // The palette's "Keys" command shows (then hides) the help panel.
    p.press(&["ctrl+p"]).await;
    p.type_text("keys").await;
    p.press(&["enter"]).await;
    assert_eq!(p.query("HelpPanel").len(), 1);
    p.press(&["ctrl+p"]).await;
    p.type_text("keys").await;
    p.press(&["enter"]).await;
    assert!(p.query("HelpPanel").is_empty());
}

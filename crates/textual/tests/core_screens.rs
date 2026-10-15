//! `switch_screen`, named (installed) screens and modes.

use textual::prelude::*;

/// A screen-ish page: a label plus a focusable button, counting how often it was mounted.
struct Page {
    name: String,
}

impl Page {
    fn new(name: &str) -> Self {
        Page { name: name.into() }
    }
}

impl Widget for Page {
    fn type_names(&self) -> &'static [&'static str] {
        &["Page"]
    }
    fn compose(&mut self) -> Vec<Child> {
        compose![
            Label::new(format!("page {}", self.name)),
            Button::new(format!("btn-{}", self.name)).id(&format!("btn-{}", self.name)),
        ]
    }
}

#[derive(Default)]
struct Root {
    dismissed: usize,
}

impl Widget for Root {
    fn compose(&mut self) -> Vec<Child> {
        compose![Label::new("home"), Button::new("home-btn").id("home-btn")]
    }
    fn bindings(&self) -> Vec<Binding> {
        vec![
            Binding::new("h", "push_screen('help')", "Help"),
            Binding::new("w", "switch_screen('help')", "Switch help"),
            Binding::new("1", "switch_mode('one')", "One"),
            Binding::new("2", "switch_mode('two')", "Two"),
            Binding::new("0", "switch_mode('_default')", "Default"),
            Binding::new("p", "push", "Push"),
        ]
    }
    fn run_action(&mut self, action: &str, _args: &[String], ctx: &mut Ctx) -> bool {
        if action == "push" {
            ctx.push_screen(Page::new("pushed"));
            return true;
        }
        false
    }
    fn on_message(&mut self, m: &Message, _ctx: &mut Ctx) -> Flow {
        if m.get::<ScreenDismissed>().is_some() {
            self.dismissed += 1;
        }
        Flow::Continue
    }
}

fn app() -> App {
    App::new(Root::default())
        .screen("help", || Page::new("help"))
        .mode("one", || Page::new("one"))
        .mode("two", || Page::new("two"))
}

#[tokio::test]
async fn switch_screen_replaces_the_top_screen() {
    let mut pilot = app().test(Size::new(40, 10)).await;
    pilot.press(&["p"]).await;
    assert_eq!(pilot.screen_count(), 2);
    assert!(pilot.screen_text().contains("page pushed"));
    let replaced = pilot
        .with::<Root, _>("App", |_, ctx| ctx.switch_screen(Page::new("next")))
        .await
        .unwrap();
    assert_eq!(pilot.screen_count(), 2);
    assert_eq!(pilot.active_screen(), Some(replaced));
    let text = pilot.screen_text();
    assert!(text.contains("page next"), "{text}");
    // The replaced screen is gone and its pusher got no result.
    assert!(pilot.query("#btn-pushed").is_empty());
    assert_eq!(pilot.focused_id().as_deref(), Some("btn-next"));
    pilot.press(&["escape"]).await; // no binding: nothing happens
    pilot
        .with::<Root, _>("App", |_, ctx| ctx.pop_screen())
        .await;
    assert_eq!(pilot.screen_count(), 1);
    assert!(pilot.screen_text().contains("home"));
    // Popping "next" (pushed by nobody) posts nothing; "pushed" never got a result either.
    assert_eq!(pilot.app::<Root>().unwrap().dismissed, 0);
}

#[tokio::test]
async fn switch_screen_can_replace_the_base_screen() {
    let mut pilot = app().test(Size::new(40, 10)).await;
    pilot
        .with::<Root, _>("App", |_, ctx| ctx.switch_screen(Page::new("base")))
        .await;
    assert_eq!(pilot.screen_count(), 1);
    let text = pilot.screen_text();
    assert!(
        text.contains("page base") && !text.contains("home"),
        "{text}"
    );
    assert!(pilot.query("#home-btn").is_empty());
}

#[tokio::test]
async fn named_screens_are_installed_and_reused() {
    let mut pilot = app().test(Size::new(40, 10)).await;
    pilot.press(&["h"]).await;
    assert_eq!(pilot.screen_count(), 2);
    let help = pilot.active_screen().unwrap();
    assert!(pilot.screen_text().contains("page help"));
    assert_eq!(pilot.focused_id().as_deref(), Some("btn-help"));
    pilot
        .with::<Root, _>("App", |_, ctx| ctx.pop_screen())
        .await;
    assert_eq!(pilot.screen_count(), 1);
    assert_eq!(pilot.app::<Root>().unwrap().dismissed, 1);
    // Popped but kept in the DOM.
    assert!(pilot.dom().contains(help));
    assert!(pilot.screen_text().contains("home"));
    assert_eq!(pilot.focused_id().as_deref(), Some("home-btn"));
    pilot.press(&["h"]).await;
    assert_eq!(pilot.active_screen(), Some(help));
    assert_eq!(pilot.focused_id().as_deref(), Some("btn-help"));
    // Pushing it again while on the stack is a no-op.
    pilot.press(&["h"]).await;
    assert_eq!(pilot.screen_count(), 2);
}

#[tokio::test]
async fn switch_to_named_screen_and_unknown_names() {
    let mut pilot = app().test(Size::new(40, 10)).await;
    pilot.press(&["p"]).await;
    pilot.press(&["w"]).await;
    assert_eq!(pilot.screen_count(), 2);
    assert!(pilot.screen_text().contains("page help"));
    assert!(pilot.query("#btn-pushed").is_empty());
    let (unknown, installed) = pilot
        .with::<Root, _>("App", |_, ctx| {
            (
                ctx.push_screen_named("nope").is_none()
                    && ctx.switch_screen_named("nope").is_none(),
                ctx.is_screen_installed("help"),
            )
        })
        .await
        .unwrap();
    assert!(unknown);
    assert!(installed);
}

#[tokio::test]
async fn install_screen_by_instance() {
    let mut pilot = app().test(Size::new(40, 10)).await;
    let id = pilot
        .with::<Root, _>("App", |_, ctx| {
            ctx.install_screen(Page::new("inst"), "inst")
        })
        .await
        .unwrap();
    // Installed screens are not shown until pushed.
    assert_eq!(pilot.screen_count(), 1);
    assert!(!pilot.screen_text().contains("page inst"));
    let pushed = pilot
        .with::<Root, _>("App", |_, ctx| ctx.push_screen_named("inst"))
        .await
        .unwrap();
    assert_eq!(pushed, Some(id));
    assert!(pilot.screen_text().contains("page inst"));
    pilot
        .with::<Root, _>("App", |_, ctx| ctx.pop_screen())
        .await;
    assert!(pilot
        .with::<Root, _>("App", |_, ctx| ctx.uninstall_screen("inst"))
        .await
        .unwrap());
    assert!(!pilot.dom().contains(id));
}

#[tokio::test]
async fn modes_keep_their_own_screen_stacks() {
    let mut pilot = app().test(Size::new(40, 10)).await;
    assert_eq!(pilot.current_mode(), "_default");
    pilot.press(&["1"]).await;
    assert_eq!(pilot.current_mode(), "one");
    assert!(pilot.screen_text().contains("page one"));
    assert_eq!(pilot.screen_count(), 1);
    pilot.press(&["p"]).await;
    assert_eq!(pilot.screen_count(), 2);
    pilot.press(&["2"]).await;
    assert_eq!(pilot.current_mode(), "two");
    let text = pilot.screen_text();
    assert!(
        text.contains("page two") && !text.contains("pushed"),
        "{text}"
    );
    assert_eq!(pilot.screen_count(), 1);
    assert_eq!(pilot.focused_id().as_deref(), Some("btn-two"));
    // Back to "one": its stack (with the pushed screen) is intact.
    pilot.press(&["1"]).await;
    assert_eq!(pilot.screen_count(), 2);
    assert!(pilot.screen_text().contains("page pushed"));
    assert_eq!(pilot.focused_id().as_deref(), Some("btn-pushed"));
    // The default mode holds the root's compose() screen.
    pilot.press(&["0"]).await;
    assert!(pilot.screen_text().contains("home"));
    assert_eq!(pilot.focused_id().as_deref(), Some("home-btn"));
    let ok = pilot
        .with::<Root, _>("App", |_, ctx| ctx.switch_mode("missing"))
        .await
        .unwrap();
    assert!(!ok);
    assert_eq!(pilot.current_mode(), "_default");
}

#[tokio::test]
async fn default_mode_is_entered_at_startup() {
    let mut pilot = app().default_mode("two").test(Size::new(40, 10)).await;
    assert_eq!(pilot.current_mode(), "two");
    assert!(pilot.screen_text().contains("page two"));
    assert_eq!(pilot.focused_id().as_deref(), Some("btn-two"));
    // Root bindings still apply in every mode.
    pilot.press(&["1"]).await;
    assert_eq!(pilot.current_mode(), "one");
    assert!(pilot.css_errors().is_empty());
}

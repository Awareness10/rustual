//! Keymaps: overriding binding keys by binding id (Textual: `Binding(id=...)`, `set_keymap`).

use std::collections::HashMap;
use textual::prelude::*;

#[derive(Default)]
struct Root {
    log: Vec<&'static str>,
}

impl Widget for Root {
    fn compose(&mut self) -> Vec<Child> {
        compose![Footer::new()]
    }
    fn bindings(&self) -> Vec<Binding> {
        vec![
            Binding::new("s", "save", "Save").id("app.save"),
            Binding::new("o", "open", "Open").id("app.open"),
            Binding::new("x", "other", "Other"),
        ]
    }
    fn run_action(&mut self, action: &str, _args: &[String], _ctx: &mut Ctx) -> bool {
        match action {
            "save" => self.log.push("save"),
            "open" => self.log.push("open"),
            "other" => self.log.push("other"),
            _ => return false,
        }
        true
    }
}

#[tokio::test]
async fn app_keymap_overrides_keys_by_id() {
    let keymap = HashMap::from([("app.save", "ctrl+s,f2")]);
    let mut p = App::new(Root::default())
        .keymap(keymap)
        .test(Size::new(60, 5))
        .await;
    p.press(&["s", "ctrl+s", "f2", "o"]).await;
    assert_eq!(p.app::<Root>().unwrap().log, vec!["save", "save", "open"]);
    let text = p.screen_text();
    assert!(text.contains("^s Save"), "{text}");
}

#[tokio::test]
async fn set_keymap_at_runtime_and_clashes() {
    let mut p = App::new(Root::default()).test(Size::new(60, 5)).await;
    // Remapping "open" to "x" takes "x" away from the id-less binding.
    p.with::<Root, _>("App", |_, ctx| ctx.set_keymap([("app.open", "x")]))
        .await;
    p.press(&["x", "o"]).await;
    assert_eq!(p.app::<Root>().unwrap().log, vec!["open"]);
    p.with::<Root, _>("App", |_, ctx| ctx.update_keymap([("app.save", "w")]))
        .await;
    p.press(&["w", "x"]).await;
    assert_eq!(p.app::<Root>().unwrap().log, vec!["open", "save", "open"]);
    let b = Binding::new("q", "quit", "Quit").id("quit");
    assert_eq!(b.id.as_deref(), Some("quit"));
}

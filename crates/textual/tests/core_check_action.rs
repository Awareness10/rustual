//! `Widget::check_action` (Textual's `check_action`): hidden / disabled bindings.

use textual::prelude::*;

#[derive(Default)]
struct Root {
    page: i32,
}

impl Widget for Root {
    fn compose(&mut self) -> Vec<Child> {
        compose![Footer::new()]
    }
    fn bindings(&self) -> Vec<Binding> {
        vec![
            Binding::new("n", "next", "Next"),
            Binding::new("p", "prev", "Previous"),
            Binding::new("x", "secret", "Secret"),
        ]
    }
    fn check_action(&self, action: &str, _args: &[String]) -> Option<bool> {
        match action {
            "prev" if self.page == 0 => None,
            "secret" => Some(false),
            _ => Some(true),
        }
    }
    fn run_action(&mut self, action: &str, _args: &[String], _ctx: &mut Ctx) -> bool {
        match action {
            "next" => self.page += 1,
            "prev" => self.page -= 1,
            "secret" => panic!("hidden action ran"),
            _ => return false,
        }
        true
    }
}

#[tokio::test]
async fn check_action_hides_and_disables() {
    let mut p = App::new(Root::default()).test(Size::new(60, 3)).await;
    let footer = p.screen_text();
    assert!(footer.contains("Next") && footer.contains("Previous"));
    assert!(!footer.contains("Secret"));
    p.press(&["p", "x"]).await;
    assert_eq!(p.app::<Root>().unwrap().page, 0);
    p.press(&["n", "n", "p"]).await;
    assert_eq!(p.app::<Root>().unwrap().page, 1);
}

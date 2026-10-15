//! The examples shown in the README (kept compiling and passing).

use rustual::prelude::*;

struct Hello;

impl Widget for Hello {
    fn compose(&mut self) -> Vec<Child> {
        compose![
            Label::new("Hello, [b $accent]World[/]!").id("greeting"),
            Button::primary("Quit").id("quit"),
        ]
    }

    fn on_message(&mut self, msg: &Message, ctx: &mut Ctx) -> Flow {
        if msg.get::<button::Pressed>().is_some() && msg.sender_id() == Some("quit") {
            ctx.exit();
        }
        Flow::Continue
    }
}

#[tokio::test]
async fn quit_button_exits() {
    let mut pilot = App::new(Hello).test(Size::new(80, 24)).await;
    pilot.click("#quit").await;
    assert!(pilot.has_exited());
}

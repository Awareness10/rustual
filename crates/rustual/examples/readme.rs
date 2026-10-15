//! The example from the README.

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

#[tokio::main]
async fn main() -> std::io::Result<()> {
    App::new(Hello)
        .css("Screen { align: center middle; } #greeting { margin: 1; }")
        .run()
        .await
}

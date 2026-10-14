//! The classic: a centered greeting that changes on key presses.
//! Run with `cargo run -p textual --example hello`. Press any letter, `t` to switch theme, ctrl+q to quit.

use textual::prelude::*;

const CSS: &str = "
Screen {
    align: center middle;
}
#greeting {
    width: 40;
    height: 5;
    content-align: center middle;
    border: round $accent;
    background: $panel;
    color: $text;
}
";

struct Hello {
    count: usize,
}

impl Widget for Hello {
    fn compose(&mut self) -> Vec<Child> {
        compose![Static::new("Hello, [b $accent]World[/]!")
            .id("greeting")
            .border_title("textual-rs")
            .border_subtitle("ctrl+q to quit")]
    }

    fn bindings(&self) -> Vec<Binding> {
        vec![Binding::new("t", "cycle_theme", "Theme")]
    }

    fn run_action(&mut self, action: &str, _args: &[String], ctx: &mut Ctx) -> bool {
        if action == "cycle_theme" {
            let names = ctx.theme_names();
            let i = names.iter().position(|n| n == ctx.theme()).unwrap_or(0);
            let next = names[(i + 1) % names.len()].clone();
            ctx.set_theme(&next);
            if let Some(s) = ctx.query_one::<Static>("#greeting") {
                s.update(format!("Theme: [b]{next}[/]"));
            }
            return true;
        }
        false
    }

    fn on_event(&mut self, event: &Event, ctx: &mut Ctx) -> Flow {
        if let Event::Key(k) = event {
            if let Some(c) = k.character.filter(|c| c.is_alphabetic()) {
                self.count += 1;
                if let Some(s) = ctx.query_one::<Static>("#greeting") {
                    s.update(format!(
                        "You pressed [b $accent]{c}[/] ({} keys)",
                        self.count
                    ));
                }
            }
        }
        Flow::Continue
    }
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    App::new(Hello { count: 0 }).css(CSS).run().await
}

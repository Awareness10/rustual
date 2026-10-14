//! Command palette, tooltips, `@click` links, `Link` and the keys panel.
//!
//! `ctrl+p` opens the palette (or click the header icon), `f1` toggles the keys panel.

use textual::prelude::*;

struct PaletteDemo {
    count: u32,
}

impl Widget for PaletteDemo {
    fn compose(&mut self) -> Vec<Child> {
        compose![
            Header::new(),
            Vertical::new().id("body").with([
                Label::new("Press [b]ctrl+p[/] for the command palette, [b]f1[/] for keys.")
                    .id("intro"),
                Button::new("Hover me")
                    .id("hover")
                    .tooltip("Buttons can have [b]tooltips[/] too."),
                Static::new("Markup links: [@click=app.bell]ring the bell[/] or [@click=app.increment]count[/].")
                    .id("links"),
                Label::new("Count: 0").id("count"),
                Link::new("Textual documentation")
                    .url("https://textual.textualize.io")
                    .tooltip("Opens in your browser"),
            ]),
            Footer::new(),
        ]
    }

    fn bindings(&self) -> Vec<Binding> {
        vec![
            Binding::new("f1", "toggle_keys", "Keys"),
            Binding::new("ctrl+p", "command_palette", "Palette"),
        ]
    }

    fn commands(&self) -> Vec<Command> {
        vec![
            Command::new("Increment", "Add one to the counter", "app.increment"),
            Command::callback("Greet", "Show a greeting notification", |ctx| {
                ctx.notify("Hello from the command palette!")
            }),
        ]
    }

    fn run_action(&mut self, action: &str, _args: &[String], ctx: &mut Ctx) -> bool {
        if action == "increment" {
            self.count += 1;
            let text = format!("Count: {}", self.count);
            if let Some(l) = ctx.query_one::<Label>("#count") {
                l.update(text);
            }
            return true;
        }
        false
    }
}

fn main() -> std::io::Result<()> {
    App::new(PaletteDemo { count: 0 })
        .title("Palette demo")
        .command_provider(|ctx: &mut Ctx| {
            ctx.theme_names()
                .into_iter()
                .filter(|t| t.contains("dark"))
                .map(|t| {
                    Command::new(
                        &format!("Dark theme: {t}"),
                        "Switch theme",
                        &format!("app.set_theme('{t}')"),
                    )
                })
                .collect()
        })
        .css("#body { padding: 1 2; } #body > * { margin-bottom: 1; }")
        .run_blocking()
}

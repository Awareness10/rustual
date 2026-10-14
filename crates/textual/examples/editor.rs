//! A small code editor: a `TextArea` with line numbers, a status bar showing the cursor
//! position, and bindings to toggle soft wrap and cycle editor themes.
//! Run with `cargo run -p textual --example editor [file]`. ctrl+q quits.

use textual::prelude::*;
use textual::widgets::text_area::{self, TextAreaTheme};

const CSS: &str = "
#status {
    height: 1;
    width: 1fr;
    padding: 0 1;
    background: $panel;
    color: $text-muted;
}
";

const SAMPLE: &str = r#"# A sample Python file — edit away!

def fibonacci(n: int) -> list[int]:
    """Return the first n Fibonacci numbers."""
    result = [0, 1]
    while len(result) < n:
        result.append(result[-1] + result[-2])
    return result[:n]


if __name__ == "__main__":
    for i, value in enumerate(fibonacci(20)):
        print(f"{i:>3}: {value}")  # 日本語 and emoji 🎉 work too
"#;

struct Editor {
    text: String,
    language: &'static str,
}

impl Editor {
    fn update_status(&self, ctx: &mut Ctx) {
        let Some(ta) = ctx.query_one::<TextArea>("#editor") else {
            return;
        };
        let (row, col) = ta.cursor_location();
        let selected = ta.selected_text().chars().count();
        let status = format!(
            "Ln {}, Col {}{}  ·  {} lines  ·  {}  ·  theme: {}  ·  wrap: {}",
            row + 1,
            col + 1,
            if selected > 0 {
                format!(" ({selected} selected)")
            } else {
                String::new()
            },
            ta.document().line_count(),
            ta.get_language().unwrap_or("plain"),
            ta.theme_name(),
            if ta.is_soft_wrap() { "on" } else { "off" },
        );
        if let Some(label) = ctx.query_one::<Label>("#status") {
            label.update_plain(status);
        }
    }
}

impl Widget for Editor {
    fn compose(&mut self) -> Vec<Child> {
        compose![
            Header::new(),
            TextArea::code_editor(&self.text, Some(self.language)).id("editor"),
            Label::new("").id("status"),
            Footer::new(),
        ]
    }

    fn on_mount(&mut self, ctx: &mut Ctx) {
        ctx.set_title("textual-rs editor");
        self.update_status(ctx);
    }

    fn bindings(&self) -> Vec<Binding> {
        vec![
            Binding::new("ctrl+t", "cycle_theme", "Theme"),
            Binding::new("ctrl+r", "toggle_wrap", "Wrap"),
            Binding::new("f7", "select_all", "Select all").hidden(),
        ]
    }

    fn run_action(&mut self, action: &str, _args: &[String], ctx: &mut Ctx) -> bool {
        match action {
            "cycle_theme" => {
                ctx.with::<TextArea, _>("#editor", |ta, ctx| {
                    let names = TextAreaTheme::builtin_names();
                    let i = names
                        .iter()
                        .position(|n| *n == ta.theme_name())
                        .unwrap_or(0);
                    ta.set_theme(names[(i + 1) % names.len()], ctx);
                });
            }
            "toggle_wrap" => {
                ctx.with::<TextArea, _>("#editor", |ta, ctx| {
                    let wrap = !ta.is_soft_wrap();
                    ta.set_soft_wrap(wrap, ctx);
                });
            }
            _ => return false,
        }
        self.update_status(ctx);
        true
    }

    fn on_message(&mut self, m: &Message, ctx: &mut Ctx) -> Flow {
        if m.get::<text_area::SelectionChanged>().is_some()
            || m.get::<text_area::Changed>().is_some()
        {
            self.update_status(ctx);
        }
        Flow::Continue
    }
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let (text, language) = match std::env::args().nth(1) {
        Some(path) => {
            let language = match path.rsplit('.').next() {
                Some("rs") => "rust",
                Some("js") => "javascript",
                Some("json") => "json",
                Some("toml") => "toml",
                _ => "python",
            };
            (std::fs::read_to_string(&path)?, language)
        }
        None => (SAMPLE.to_string(), "python"),
    };
    App::new(Editor { text, language }).css(CSS).run().await
}

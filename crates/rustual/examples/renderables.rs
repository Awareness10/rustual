//! Rich-style renderables in widgets: a table, syntax-highlighted code, panels and columns
//! shown by `Static`, and renderables written to a `RichLog`.
//! Run with `cargo run -p rustual --example renderables`. Press `w` to log a table, ctrl+q to quit.

use rustual::prelude::*;
use rustual::render::Justify;
use rustual::renderables::{Columns, Panel, Syntax, Table};

const CSS: &str = "
#top { height: auto; }
#table { width: auto; margin: 0 1; }
#code { width: 1fr; }
#log { height: 1fr; border: round $accent; }
";

const CODE: &str = r#"fn main() {
    // Rich-style renderables, laid out by width.
    let table = Table::new().title("Planets");
    println!("{}", "hello");
}
"#;

fn planets(count: usize) -> Table {
    let mut table = Table::new()
        .title("Planets")
        .caption(&format!("log entry {count}"))
        .row_styles(vec![Style::default(), Style::new().dim()]);
    table.add_column_with("Planet", |c| {
        c.style(Style::parse("cyan").unwrap_or_default())
    });
    table.add_column_with("Moons", |c| c.justify(Justify::Right));
    table.add_column_with("Notes", |c| c.ratio(1));
    table.add_row(["Earth", "1", "[b]home[/]"]);
    table.add_row(["Mars", "2", "red, dusty"]);
    table.add_row(["Jupiter", "95", "largest planet in the solar system"]);
    table
}

struct Demo {
    count: usize,
}

impl Widget for Demo {
    fn compose(&mut self) -> Vec<Child> {
        let code = Syntax::new(CODE, "rust")
            .line_numbers(true)
            .highlight_lines([3]);
        let words = Columns::new(
            "alpha bravo charlie delta echo foxtrot golf hotel india juliett kilo lima".split(' '),
        );
        compose![
            Horizontal::new().id("top").with(compose![
                Static::renderable(planets(0)).id("table"),
                Static::renderable(Panel::new(code).title("main.rs")).id("code"),
            ]),
            Static::renderable(
                Panel::new(words)
                    .title("Columns")
                    .subtitle("fills the width")
            ),
            RichLog::new().id("log"),
            Footer::new(),
        ]
    }

    fn bindings(&self) -> Vec<Binding> {
        vec![Binding::new("w", "write", "Log a table")]
    }

    fn run_action(&mut self, action: &str, _args: &[String], ctx: &mut Ctx) -> bool {
        if action == "write" {
            self.count += 1;
            let table = planets(self.count);
            ctx.with::<RichLog, _>("#log", |log, ctx| log.write_renderable(table, ctx));
            return true;
        }
        false
    }
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    App::new(Demo { count: 0 }).css(CSS).run().await
}

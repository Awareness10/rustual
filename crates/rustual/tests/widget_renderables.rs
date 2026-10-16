//! Rich-style renderables shown by `Static` / `Label` and written to `RichLog`.

use rustual::prelude::*;
use rustual::renderables::{Columns, Panel, Table};

fn planets() -> Table {
    let mut t = Table::new().title("Planets");
    t.add_column("Name");
    t.add_column_with("Moons", |c| c.justify(rustual::render::Justify::Right));
    t.add_row(["Earth", "1"]);
    t.add_row(["[b]Mars[/]", "2"]);
    t
}

struct Statics;
impl Widget for Statics {
    fn compose(&mut self) -> Vec<Child> {
        compose![
            Static::renderable(planets()).id("table"),
            Static::renderable(Panel::fit("boxed")).id("auto"),
            Static::renderable(Panel::new("wide")).id("full"),
        ]
    }
}

#[tokio::test]
async fn static_shows_renderables() {
    let mut pilot = App::new(Statics)
        .css("#auto { width: auto; } Screen { layout: vertical; }")
        .test(Size::new(40, 16))
        .await;
    let text = pilot.screen_text();
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines[0].trim(), "Planets", "{text}");
    assert!(lines[1].starts_with("┏━━━━━━━┳━━━━━━━┓"), "{text}");
    assert!(lines[3].starts_with("┡━━━━━━━╇━━━━━━━┩"), "{text}");
    assert!(lines[5].starts_with("│ Mars  │     2 │"), "{text}");
    // Height is auto: the next widget starts right after the table.
    assert!(lines[7].starts_with("╭───────╮"), "{text}");
    // `width: auto` takes the measured width (Panel.fit: 9 cells).
    let auto = pilot.query("#auto")[0];
    assert_eq!(pilot.dom().get(auto).unwrap().layout.region.width, 9);
    // An expanding panel fills the width.
    assert_eq!(lines[10].chars().count(), 40, "{text}");
    assert!(
        lines[10].starts_with('╭') && lines[10].ends_with('╮'),
        "{text}"
    );

    // Markup inside cells keeps its style.
    let c = pilot.canvas();
    assert!(c
        .get(2, 5)
        .unwrap()
        .attrs
        .contains(rustual::render::Attrs::BOLD));

    // Updating swaps the renderable.
    pilot
        .with::<Static, _>("#table", |s, _| s.update_renderable(Panel::fit("new")))
        .await;
    let text = pilot.screen_text();
    assert!(text.starts_with("╭─────╮\n│ new │"), "{text}");
    assert_eq!(
        pilot.query_one::<Static>("#auto").unwrap().plain_text(),
        "╭───────╮\n│ boxed │\n╰───────╯"
    );
}

struct Log;
impl Widget for Log {
    fn compose(&mut self) -> Vec<Child> {
        compose![RichLog::new().wrap(true).min_width(10).id("log")]
    }
}

#[tokio::test]
async fn rich_log_writes_renderables() {
    let mut pilot = App::new(Log)
        .css("RichLog { scrollbar-size-vertical: 0; }")
        .test(Size::new(30, 12))
        .await;
    pilot
        .with::<RichLog, _>("#log", |log, ctx| {
            log.write("a long line of text that surely wraps", ctx);
            log.write_renderable(planets(), ctx);
            log.write_renderable(
                Columns::new(["one", "two", "three", "four", "five", "six"]),
                ctx,
            );
        })
        .await;
    let text = pilot.screen_text();
    let lines: Vec<&str> = text.lines().collect();
    // Text is wrapped to the width; the table keeps its own layout (not re-wrapped).
    assert_eq!(lines[0].trim_end(), "a long line of text that", "{text}");
    assert_eq!(lines[1].trim_end(), "surely wraps", "{text}");
    assert_eq!(lines[2].trim(), "Planets", "{text}");
    assert_eq!(lines[3].trim_end(), "┏━━━━━━━┳━━━━━━━┓", "{text}");
    assert_eq!(lines[8].trim_end(), "└───────┴───────┘", "{text}");
    // Columns has no natural width: it is laid out for the region (30 cells).
    assert_eq!(lines[9].trim_end(), "one two three four five six", "{text}");
}

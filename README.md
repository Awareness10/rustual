# textual-rs

A Rust re-creation of Python's [Textual](https://textual.textualize.io) TUI framework: a DOM of
widgets, styled with **Textual CSS**, arranged by Textual's layout engine (vertical, horizontal,
grid, dock, layers, scrolling), and driven by an async **tokio + crossterm** event loop.

```rust
use textual::prelude::*;

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
```

Run the examples: `cargo run -p textual --example <name>` with `hello`, `form`, `dashboard`,
`table`, `navigator` or `readme`. Quit with `ctrl+q`.

## Widgets

Static, Label, Button, Input, Checkbox, RadioButton/RadioSet, Switch, Header, Footer, Placeholder,
Rule, ProgressBar, LoadingIndicator, Sparkline, Digits, Toasts (`ctx.notify`), ListView, OptionList,
DataTable, Log, RichLog, Tabs, TabbedContent/TabPane, ContentSwitcher, Tree, Collapsible, Select,
and containers (Container, Vertical, Horizontal, Grid, Center, Middle, VerticalScroll, ...).

Not yet: TextArea, Markdown, command palette, animations, text selection, tooltips, inline mode.

## Concepts (Textual → textual-rs)

| Textual | textual-rs |
|---|---|
| `class MyApp(App)` with `compose`, `BINDINGS`, `action_*`, `on_*` | `impl Widget for MyApp` passed to `App::new` |
| `yield Button("OK", id="ok")` | `compose![Button::new("OK").id("ok")]` |
| `with Horizontal(): ...` | `Horizontal::new().with(compose![...])` |
| `DEFAULT_CSS`, `CSS`, `CSS_PATH` | `Widget::default_css`, `App::css`, `App::css_path` |
| `on_button_pressed(self, event)` | `on_message`: `msg.get::<button::Pressed>()` |
| `self.query_one("#x", Label)` | `ctx.query_one::<Label>("#x")` |
| `@work async def` | `ctx.run_worker(async { Msg })` |
| `set_interval` | `ctx.set_interval(dur, \|w: &mut Self, ctx\| ..)` |
| `push_screen` / `dismiss` | `ctx.push_screen` / `ctx.push_modal` / `ctx.dismiss(result)` |
| `async with app.run_test() as pilot` | `let mut pilot = App::new(..).test(size).await` |

See `docs/widget-guide.md` for writing widgets and `docs/plans/` for the architecture.

## Workspace

| Crate | Role |
|---|---|
| `textual-render` | geometry, colors, styles, cell widths, styled text + markup, strips, border glyphs, cell canvas with diffing (the "rich" layer) |
| `textual-css` | TCSS tokenizer/parser (variables, nesting, `!important`), selectors & specificity, typed `Styles`, cascade, themes/design system |
| `textual` | DOM, widgets, events/messages/bindings/actions, layout, compositor, crossterm driver, app runtime, Pilot test harness |

## Testing

`cargo test --workspace`. Apps are tested headlessly with `Pilot`
(`press`, `type_text`, `click`, `hover`, `wait`, `screen_text`, `canvas`, `query_one`, ...).

The `reference/` directory (git-ignored) holds the upstream Textual and Rich sources used as the
behavioural reference.

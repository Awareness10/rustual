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

Run the examples with `cargo run -p textual --example <name>`: `hello`, `form`, `dashboard`, `table`,
`navigator`, `editor`, `markdown`, `palette`, `widgets_extra`, `readme`. Quit with `ctrl+q`;
`ctrl+p` opens the command palette.

## What's included

- **Widgets:** Static, Label, Button, Input, MaskedInput, TextArea, Checkbox, RadioButton/RadioSet, Switch,
  Select, SelectionList, OptionList, ListView, DataTable, Tree, DirectoryTree, Tabs, TabbedContent,
  ContentSwitcher, Collapsible, Header, Footer, Markdown, MarkdownViewer, Log, RichLog, ProgressBar,
  LoadingIndicator, Sparkline, Digits, Placeholder, Rule, Pretty, Link, Tooltip, Toasts, KeyPanel, Welcome,
  plus containers (Container, Vertical, Horizontal, Grid, Center, Middle, VerticalScroll, ...).
- **Core:** Textual CSS with themes, vertical/horizontal/grid/dock layouts, layers, scrolling, screens and
  modals, command palette, tooltips, animations with easing, CSS transitions, timers, async workers,
  notifications, SVG screenshots, and a headless `Pilot` for tests.

See [`features.md`](features.md) for per-feature parity with Textual and [`ToDo.md`](ToDo.md) for what's left.

## Python

[`crates/textual-py`](crates/textual-py/README.md) is a PyO3/maturin package, `textual_rs`, with a
Textual-compatible Python API on this engine. Port a Textual app by changing its imports from
`textual` to `textual_rs`. It runs the same app code 13–56× faster than Python Textual in the included
benchmark.

```sh
cd crates/textual-py && uv sync && uv run python examples/counter.py
```

## Screenshots

Render any app to SVG without a terminal (handy for docs and visual checks):

```sh
TEXTUAL_SCREENSHOT=shot.svg TEXTUAL_SIZE=100x30 TEXTUAL_PRESS=tab,down cargo run -p textual --example table
```

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
| `textual-py` | Python bindings (PyO3, built with maturin/uv): the `textual_rs` package (see [`docs/python-bindings.md`](docs/python-bindings.md)) |

## Testing

`cargo test --workspace` (the Python extension has no Rust tests; test it with
`cd crates/textual-py && uv run pytest`). Apps are tested headlessly with `Pilot`
(`press`, `type_text`, `click`, `hover`, `wait`, `screen_text`, `canvas`, `query_one`, ...).

The `reference/` directory (git-ignored) holds the upstream Textual and Rich sources used as the
behavioural reference.

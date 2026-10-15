<div align="center">

<img src="https://raw.githubusercontent.com/Awareness10/rustual/master/docs/assets/logo.svg" alt="rustual" width="360">

**Textual's TUI framework, rebuilt in Rust.**

[Repository](https://github.com/Awareness10/rustual) · [Features](https://github.com/Awareness10/rustual/blob/master/features.md) · [Python bindings](https://github.com/Awareness10/rustual/tree/master/crates/rustual-py)

</div>

A DOM of widgets, styled with **Textual CSS** (variables, nesting, themes, transitions), laid out with
vertical / horizontal / grid / dock layouts, and driven by an async tokio + crossterm event loop. 40+
widgets (DataTable, Tree, TextArea, Markdown, TabbedContent, Select, Input, ...) and a headless
`Pilot` for tests.

```rust,no_run
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
```

See the [repository](https://github.com/Awareness10/rustual) for examples, screenshots and the Textual → rustual concept map.

## License

MIT. rustual is a re-creation of [Textual](https://github.com/Textualize/textual) and
[Rich](https://github.com/Textualize/rich) by Will McGugan and the Textualize team; see `LICENSE`.

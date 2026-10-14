//! A `MarkdownViewer` with a table of contents. Run with `cargo run -p textual --example markdown`.
//! `t` toggles the table of contents, `ctrl+q` quits. Click a heading in the sidebar to jump to it.

use textual::prelude::*;
use textual::widgets::markdown;

const DOCUMENT: &str = r#"# textual-rs Markdown

A **Rust** re-creation of *Textual's* `Markdown` widget. It supports **bold**, *italic*,
~~strikethrough~~, `inline code` and [links](https://textual.textualize.io).
Hard breaks work too:\
like this.

## Lists

- Block widgets for every element
- Nested lists
  - with different bullets
    - at each level
- Task lists:
  - [x] parse with pulldown-cmark
  - [x] render tables
  - [ ] syntax highlighting

1. Ordered lists
2. are numbered
3. and aligned

## Quotes

> Markdown is intended to be as easy-to-read and easy-to-write as is feasible.
>
> — John Gruber

## Code

```rust
fn main() -> std::io::Result<()> {
    App::new(MarkdownApp).run_blocking()
}
```

## Tables

| Widget | Status | Coverage |
|:-------|:------:|---------:|
| Markdown | done | 85% |
| MarkdownViewer | done | 80% |
| TextArea | planned | 0% |

---

## Links

Jump back to the [top](#textual-rs-markdown) or visit [Textual](https://github.com/Textualize/textual).

### Heading three

#### Heading four

##### Heading five

###### Heading six

![screenshot](screenshot.png)
"#;

struct MarkdownApp;

impl Widget for MarkdownApp {
    fn compose(&mut self) -> Vec<Child> {
        compose![
            Header::new(),
            MarkdownViewer::new(DOCUMENT).id("viewer"),
            Footer::new()
        ]
    }

    fn bindings(&self) -> Vec<Binding> {
        vec![
            Binding::new("t", "toggle_toc", "Table of contents"),
            Binding::new("ctrl+q", "app.quit", "Quit"),
        ]
    }

    fn run_action(&mut self, action: &str, _args: &[String], ctx: &mut Ctx) -> bool {
        if action == "toggle_toc" {
            ctx.with::<MarkdownViewer, _>("#viewer", |v, ctx| {
                let show = !v.is_showing_table_of_contents();
                v.set_show_table_of_contents(show, ctx);
            });
            return true;
        }
        false
    }

    fn on_message(&mut self, msg: &Message, ctx: &mut Ctx) -> Flow {
        if let Some(link) = msg.get::<markdown::LinkClicked>() {
            ctx.notify(&format!("Link clicked: {}", link.href));
        }
        Flow::Continue
    }
}

fn main() -> std::io::Result<()> {
    App::new(MarkdownApp).title("Markdown").run_blocking()
}

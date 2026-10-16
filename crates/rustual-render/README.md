# rustual-render

Rendering primitives for [rustual](https://github.com/Awareness10/rustual), playing the role that [Rich](https://github.com/Textualize/rich)
plays for Python Textual: colors and styles, Textual / Rich-style markup, styled text with
cell-width-aware wrapping, line strips, border glyphs, Rich-style renderables (`Table`, `Panel`, `Columns`,
`Padding`, `Group`; see `renderables`), and a 2D cell canvas with diffing and SVG / ANSI export.

```rust
use rustual_render::{Color, Text};

let text = Text::from_markup("[b red]Hello[/] 世界");
assert_eq!(text.plain_text(), "Hello 世界");
assert_eq!(text.cell_len(), 10); // CJK characters are two cells wide
assert_eq!(Color::parse("#ff8800").unwrap(), Color::rgb(255, 136, 0));
```

Most users want the [`rustual`](https://crates.io/crates/rustual) crate, which builds on this one.
License: MIT (see `LICENSE`).

# rustual-css

Textual CSS (TCSS) for [rustual](https://github.com/Awareness10/rustual): tokenizer, parser (variables, nesting, `!important`),
selectors and specificity, typed `Styles`, the cascade, CSS transitions, and the theme / design system
that generates Textual's `$primary`, `$surface`, ... variables (19 built-in themes).

```rust
use rustual_css::{Display, SelectorList, Styles, Theme};

let styles = Styles::parse_inline("display: none; width: 50%; color: red").unwrap();
assert_eq!(styles.get_display(), Display::None);
assert!(SelectorList::parse("Screen > Button.primary, #sidebar Label").is_ok());
assert!(Theme::builtin().iter().any(|t| t.name == "textual-dark"));
```

Most users want the [`rustual`](https://crates.io/crates/rustual) crate, which builds on this one.
License: MIT (see `LICENSE`).

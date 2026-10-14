//! Textual content markup: `"[bold red]Hello[/] [on $accent]world[/on $accent]"`.
//!
//! Tags: style strings accepted by [`Style::parse_with`] (`b`, `bold`, `i`, `u`, `s`, `d`, `r`,
//! colors, `on <color>`, `$var`), `[@click=action]` (stored in `Style::link`).
//! `[/]` closes the most recent tag, `[/tag]` closes the matching tag. `\[` escapes a bracket.
//! A `[` not followed by a valid tag (e.g. `[1, 2]`) is treated as literal text, like Textual.

use crate::color::Color;
use crate::text::Text;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkupError(pub String);

impl fmt::Display for MarkupError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "markup error: {}", self.0)
    }
}

impl std::error::Error for MarkupError {}

pub fn parse_markup(s: &str, vars: &dyn Fn(&str) -> Option<Color>) -> Result<Text, MarkupError> {
    todo!("{s}{:?}", vars(""))
}

/// Escape `[` so text is rendered literally when interpreted as markup.
pub fn escape(s: &str) -> String {
    s.replace('[', "\\[")
}

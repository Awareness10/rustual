//! Span styles (like `rich.style.Style`): optional colors and attributes that combine.

use crate::color::Color;
use std::fmt;
use std::sync::Arc;

/// Resolved text attribute bit flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Attrs(pub u8);

impl Attrs {
    pub const NONE: Attrs = Attrs(0);
    pub const BOLD: Attrs = Attrs(1);
    pub const DIM: Attrs = Attrs(2);
    pub const ITALIC: Attrs = Attrs(4);
    pub const UNDERLINE: Attrs = Attrs(8);
    pub const STRIKE: Attrs = Attrs(16);
    pub const REVERSE: Attrs = Attrs(32);
    pub const BLINK: Attrs = Attrs(64);

    pub fn contains(&self, o: Attrs) -> bool {
        self.0 & o.0 == o.0
    }
    pub fn insert(&mut self, o: Attrs) {
        self.0 |= o.0
    }
    pub fn remove(&mut self, o: Attrs) {
        self.0 &= !o.0
    }
    pub fn union(self, o: Attrs) -> Attrs {
        Attrs(self.0 | o.0)
    }
    pub fn is_empty(&self) -> bool {
        self.0 == 0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StyleParseError(pub String);

impl fmt::Display for StyleParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid style: {}", self.0)
    }
}

impl std::error::Error for StyleParseError {}

/// A style where every field is optional; `None` means "inherit from whatever is below".
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Style {
    pub fg: Option<Color>,
    pub bg: Option<Color>,
    pub bold: Option<bool>,
    pub dim: Option<bool>,
    pub italic: Option<bool>,
    pub underline: Option<bool>,
    pub strike: Option<bool>,
    pub reverse: Option<bool>,
    pub blink: Option<bool>,
    /// Link / action metadata (e.g. from markup `[@click=app.bell]`).
    pub link: Option<Arc<str>>,
}

impl Style {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn fg(mut self, c: Color) -> Self {
        self.fg = Some(c);
        self
    }
    pub fn bg(mut self, c: Color) -> Self {
        self.bg = Some(c);
        self
    }
    pub fn bold(mut self) -> Self {
        self.bold = Some(true);
        self
    }
    pub fn dim(mut self) -> Self {
        self.dim = Some(true);
        self
    }
    pub fn italic(mut self) -> Self {
        self.italic = Some(true);
        self
    }
    pub fn underline(mut self) -> Self {
        self.underline = Some(true);
        self
    }
    pub fn strike(mut self) -> Self {
        self.strike = Some(true);
        self
    }
    pub fn reverse(mut self) -> Self {
        self.reverse = Some(true);
        self
    }

    /// Parse a rich-like style string: `"bold red on #000033 not italic underline"`.
    /// Words: attribute names (b/bold, d/dim, i/italic, u/underline, s/strike, r/reverse, blink),
    /// `not <attr>`, a color, `on <color>`, `none`. Colors use [`Color::parse`] and `$name`
    /// is looked up with `vars`.
    pub fn parse_with(
        s: &str,
        vars: &dyn Fn(&str) -> Option<Color>,
    ) -> Result<Style, StyleParseError> {
        todo!("{s}{:?}", vars("x"))
    }

    pub fn parse(s: &str) -> Result<Style, StyleParseError> {
        Self::parse_with(s, &|_| None)
    }

    /// Combine with `over` taking precedence. Colors in `over` with alpha < 1 are
    /// composited over the corresponding color in `self` (when present).
    pub fn combine(&self, over: &Style) -> Style {
        todo!("{over:?}")
    }

    /// Resolve attribute flags (`None` → off).
    pub fn attrs(&self) -> Attrs {
        todo!()
    }

    pub fn is_empty(&self) -> bool {
        *self == Style::default()
    }
}

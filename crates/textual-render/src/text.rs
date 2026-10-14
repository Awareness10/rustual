//! Styled text (like `rich.text.Text`) with wrapping and justification.

use crate::strip::Strip;
use crate::style::Style;
use crate::color::Color;

/// A run of text with a single style. May contain `\n` inside a [`Text`]; never inside a [`Strip`].
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Span {
    pub text: String,
    pub style: Style,
}

impl Span {
    pub fn new(text: impl Into<String>, style: Style) -> Self {
        Span { text: text.into(), style }
    }
    pub fn raw(text: impl Into<String>) -> Self {
        Span { text: text.into(), style: Style::default() }
    }
    pub fn cell_len(&self) -> i32 {
        crate::cells::cell_len(&self.text)
    }
}

/// Horizontal justification of wrapped lines.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Justify {
    #[default]
    Left,
    Center,
    Right,
    /// Distribute spaces so lines (except the last) fill the width.
    Full,
}

/// What to do with a word/line longer than the width.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextOverflow {
    /// Break words that don't fit across lines.
    #[default]
    Fold,
    /// Cut off at the width.
    Clip,
    /// Cut off and end with `…`.
    Ellipsis,
}

/// Multi-line styled text.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Text {
    pub spans: Vec<Span>,
}

impl Text {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn plain(s: impl Into<String>) -> Self {
        Text { spans: vec![Span::raw(s)] }
    }

    pub fn styled(s: impl Into<String>, style: Style) -> Self {
        Text { spans: vec![Span::new(s, style)] }
    }

    /// Parse Textual content markup leniently: on a markup error the input is returned as plain text.
    pub fn from_markup(s: &str) -> Self {
        crate::markup::parse_markup(s, &|_| None).unwrap_or_else(|_| Text::plain(s))
    }

    /// Like [`Text::from_markup`] with `$variable` color lookup.
    pub fn from_markup_with(s: &str, vars: &dyn Fn(&str) -> Option<Color>) -> Self {
        crate::markup::parse_markup(s, vars).unwrap_or_else(|_| Text::plain(s))
    }

    pub fn push(&mut self, text: impl Into<String>, style: Style) -> &mut Self {
        self.spans.push(Span::new(text, style));
        self
    }

    pub fn append(&mut self, other: Text) -> &mut Self {
        self.spans.extend(other.spans);
        self
    }

    /// Concatenated text without styles.
    pub fn plain_text(&self) -> String {
        self.spans.iter().map(|s| s.text.as_str()).collect()
    }

    pub fn is_empty(&self) -> bool {
        self.spans.iter().all(|s| s.text.is_empty())
    }

    /// Width of the widest line (lines split on `\n`, no wrapping).
    pub fn cell_len(&self) -> i32 {
        todo!()
    }

    /// Apply `style` underneath every span (span styles take precedence).
    pub fn stylize_base(&mut self, style: &Style) {
        todo!("{style:?}")
    }

    /// Split on `\n` without wrapping. An empty text yields zero lines; a trailing
    /// newline does not produce an extra empty line (`"a\n"` → 1 line, `"a\n\nb"` → 3).
    pub fn lines(&self) -> Vec<Strip> {
        todo!()
    }

    /// Word-wrap to `width` cells. Splits on `\n` first, then wraps each paragraph at
    /// word boundaries (whitespace collapsed at wrap points; leading indentation kept).
    /// `no_wrap` disables wrapping (lines are then cropped per `overflow`).
    /// Justify Center/Right pads on the left with unstyled spaces; no right padding is added
    /// except for `Full`. Every returned strip has `cell_len() <= width`.
    pub fn wrap(&self, width: i32, justify: Justify, overflow: TextOverflow, no_wrap: bool) -> Vec<Strip> {
        todo!("{width}{justify:?}{overflow:?}{no_wrap}")
    }

    /// Number of lines after wrapping to `width` (Left justify, Fold overflow).
    pub fn height_for_width(&self, width: i32) -> i32 {
        self.wrap(width, Justify::Left, TextOverflow::Fold, false).len() as i32
    }
}

impl From<&str> for Text {
    fn from(s: &str) -> Self {
        Text::plain(s)
    }
}

impl From<String> for Text {
    fn from(s: String) -> Self {
        Text::plain(s)
    }
}

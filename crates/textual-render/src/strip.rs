//! A single line of styled spans.

use crate::style::Style;
use crate::text::{Justify, Span};

/// One line of styled spans (no newlines). Cropping is by cell position, and a wide
/// character split by a crop boundary is replaced by a space.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Strip {
    pub spans: Vec<Span>,
}

impl Strip {
    pub fn new(spans: Vec<Span>) -> Self {
        Strip { spans }
    }

    pub fn from_str(s: &str, style: Style) -> Self {
        Strip { spans: vec![Span::new(s, style)] }
    }

    /// `width` spaces in `style`.
    pub fn blank(width: i32, style: Style) -> Self {
        Strip { spans: vec![Span::new(" ".repeat(width.max(0) as usize), style)] }
    }

    pub fn push(&mut self, span: Span) {
        self.spans.push(span);
    }

    pub fn cell_len(&self) -> i32 {
        self.spans.iter().map(|s| s.cell_len()).sum()
    }

    pub fn plain_text(&self) -> String {
        self.spans.iter().map(|s| s.text.as_str()).collect()
    }

    /// Cells `start..end`.
    pub fn crop(&self, start: i32, end: i32) -> Strip {
        todo!("{start}{end}")
    }

    /// Pad (with spaces in `style`) or crop to exactly `width` cells.
    pub fn set_cell_len(&self, width: i32, style: &Style) -> Strip {
        todo!("{width}{style:?}")
    }

    /// Pad to `width` with the content positioned per `justify` (`Full` acts like `Left`).
    pub fn align(&self, width: i32, justify: Justify, style: &Style) -> Strip {
        todo!("{width}{justify:?}{style:?}")
    }

    /// Apply `base` underneath every span's style.
    pub fn apply_base_style(&self, base: &Style) -> Strip {
        todo!("{base:?}")
    }

    /// Apply `over` on top of every span's style.
    pub fn apply_style(&self, over: &Style) -> Strip {
        todo!("{over:?}")
    }

    /// Merge adjacent spans with identical styles.
    pub fn simplify(&self) -> Strip {
        todo!()
    }
}

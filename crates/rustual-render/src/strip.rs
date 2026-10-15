//! A single line of styled spans.

use crate::cells::{grapheme_width, graphemes};
use crate::style::Style;
use crate::text::{Justify, Span};

/// One line of styled spans (no newlines). Cropping is by cell position, and a wide
/// character split by a crop boundary is replaced by a space.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Strip {
    pub spans: Vec<Span>,
}

/// Appends `text` in `style` to `spans`, merging with the last span if the style is equal.
pub(crate) fn push_merged(spans: &mut Vec<Span>, text: &str, style: &Style) {
    if text.is_empty() {
        return;
    }
    if let Some(last) = spans.last_mut() {
        if last.style == *style {
            last.text.push_str(text);
            return;
        }
    }
    spans.push(Span::new(text, style.clone()));
}

impl Strip {
    pub fn new(spans: Vec<Span>) -> Self {
        Strip { spans }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str, style: Style) -> Self {
        Strip {
            spans: vec![Span::new(s, style)],
        }
    }

    /// `width` spaces in `style`.
    pub fn blank(width: i32, style: Style) -> Self {
        Strip {
            spans: vec![Span::new(" ".repeat(width.max(0) as usize), style)],
        }
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
        let start = start.max(0);
        let mut out: Vec<Span> = Vec::new();
        if end <= start {
            return Strip::default();
        }
        let mut pos = 0;
        'outer: for span in &self.spans {
            for g in graphemes(&span.text) {
                if pos >= end {
                    break 'outer;
                }
                let w = grapheme_width(g);
                let g_end = pos + w;
                if w == 0 {
                    if pos >= start {
                        push_merged(&mut out, g, &span.style);
                    }
                } else if pos >= start && g_end <= end {
                    push_merged(&mut out, g, &span.style);
                } else if g_end > start {
                    // Partially visible wide grapheme: replace visible cells with spaces.
                    let visible = g_end.min(end) - pos.max(start);
                    if visible > 0 {
                        push_merged(&mut out, &" ".repeat(visible as usize), &span.style);
                    }
                }
                pos = g_end;
            }
        }
        Strip { spans: out }
    }

    /// Pad (with spaces in `style`) or crop to exactly `width` cells.
    pub fn set_cell_len(&self, width: i32, style: &Style) -> Strip {
        let len = self.cell_len();
        if len > width {
            self.crop(0, width)
        } else {
            let mut out = self.clone();
            if len < width {
                out.spans
                    .push(Span::new(" ".repeat((width - len) as usize), style.clone()));
            }
            out
        }
    }

    /// Pad to `width` with the content positioned per `justify` (`Full` acts like `Left`).
    /// Content wider than `width` is cropped.
    pub fn align(&self, width: i32, justify: Justify, style: &Style) -> Strip {
        let len = self.cell_len();
        if len >= width {
            return self.crop(0, width);
        }
        let excess = width - len;
        let left = match justify {
            Justify::Left | Justify::Full => 0,
            Justify::Center => excess / 2,
            Justify::Right => excess,
        };
        let right = excess - left;
        let mut spans = Vec::with_capacity(self.spans.len() + 2);
        if left > 0 {
            spans.push(Span::new(" ".repeat(left as usize), style.clone()));
        }
        spans.extend(self.spans.iter().cloned());
        if right > 0 {
            spans.push(Span::new(" ".repeat(right as usize), style.clone()));
        }
        Strip { spans }
    }

    /// Apply `base` underneath every span's style.
    pub fn apply_base_style(&self, base: &Style) -> Strip {
        Strip {
            spans: self
                .spans
                .iter()
                .map(|s| Span::new(s.text.clone(), base.combine(&s.style)))
                .collect(),
        }
    }

    /// Apply `over` on top of every span's style.
    pub fn apply_style(&self, over: &Style) -> Strip {
        Strip {
            spans: self
                .spans
                .iter()
                .map(|s| Span::new(s.text.clone(), s.style.combine(over)))
                .collect(),
        }
    }

    /// Merge adjacent spans with identical styles (empty spans are dropped).
    pub fn simplify(&self) -> Strip {
        let mut out = Vec::with_capacity(self.spans.len());
        for s in &self.spans {
            push_merged(&mut out, &s.text, &s.style);
        }
        Strip { spans: out }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::Color;

    fn red() -> Style {
        Style::new().fg(Color::rgb(255, 0, 0))
    }

    fn two_spans() -> Strip {
        Strip::new(vec![Span::raw("hello "), Span::new("world", red())])
    }

    #[test]
    fn crop_basic() {
        let s = two_spans();
        assert_eq!(s.crop(0, 5).plain_text(), "hello");
        let c = s.crop(4, 8);
        assert_eq!(c.plain_text(), "o wo");
        assert_eq!(c.spans.len(), 2);
        assert_eq!(c.spans[1].style, red());
        assert_eq!(s.crop(6, 100).plain_text(), "world");
        assert_eq!(s.crop(20, 30).plain_text(), "");
        assert_eq!(s.crop(5, 3).plain_text(), "");
        assert_eq!(s.crop(-3, 2).plain_text(), "he");
    }

    #[test]
    fn crop_wide() {
        let s = Strip::from_str("a中b", Style::default());
        assert_eq!(s.cell_len(), 4);
        assert_eq!(s.crop(0, 2).plain_text(), "a ");
        assert_eq!(s.crop(2, 4).plain_text(), " b");
        assert_eq!(s.crop(1, 3).plain_text(), "中");
        assert_eq!(s.crop(2, 3).plain_text(), " ");
        for a in 0..5 {
            for b in a..5 {
                assert_eq!(s.crop(a, b).cell_len(), b - a, "crop({a},{b})");
            }
        }
    }

    #[test]
    fn set_cell_len_pad_and_crop() {
        let s = Strip::from_str("ab", Style::default());
        let p = s.set_cell_len(5, &red());
        assert_eq!(p.plain_text(), "ab   ");
        assert_eq!(p.spans.last().unwrap().style, red());
        assert_eq!(s.set_cell_len(1, &red()).plain_text(), "a");
        assert_eq!(s.set_cell_len(2, &red()), s);
        let w = Strip::from_str("中中", Style::default());
        assert_eq!(w.set_cell_len(3, &red()).plain_text(), "中 ");
    }

    #[test]
    fn align_modes() {
        let s = Strip::from_str("ab", Style::default());
        assert_eq!(s.align(6, Justify::Left, &red()).plain_text(), "ab    ");
        assert_eq!(s.align(6, Justify::Full, &red()).plain_text(), "ab    ");
        assert_eq!(s.align(6, Justify::Right, &red()).plain_text(), "    ab");
        assert_eq!(s.align(7, Justify::Center, &red()).plain_text(), "  ab   ");
        assert_eq!(s.align(1, Justify::Center, &red()).plain_text(), "a");
    }

    #[test]
    fn styles_apply() {
        let s = two_spans();
        let base = Style::new().fg(Color::rgb(0, 0, 255)).bold();
        let b = s.apply_base_style(&base);
        assert_eq!(b.spans[0].style.fg, Some(Color::rgb(0, 0, 255)));
        assert_eq!(b.spans[1].style.fg, Some(Color::rgb(255, 0, 0)));
        assert_eq!(b.spans[1].style.bold, Some(true));
        let o = s.apply_style(&base);
        assert_eq!(o.spans[1].style.fg, Some(Color::rgb(0, 0, 255)));
    }

    #[test]
    fn simplify_merges() {
        let s = Strip::new(vec![
            Span::raw("a"),
            Span::raw("b"),
            Span::raw(""),
            Span::new("c", red()),
            Span::new("d", red()),
            Span::raw("e"),
        ]);
        let m = s.simplify();
        assert_eq!(m.spans.len(), 3);
        assert_eq!(m.spans[0].text, "ab");
        assert_eq!(m.spans[1].text, "cd");
        assert_eq!(m.plain_text(), "abcde");
    }

    #[test]
    fn blank_strip() {
        assert_eq!(Strip::blank(3, Style::default()).cell_len(), 3);
        assert_eq!(Strip::blank(-1, Style::default()).cell_len(), 0);
    }
}

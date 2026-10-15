//! Styled text (like `rich.text.Text`) with wrapping and justification.

use crate::cells::{grapheme_width, graphemes};
use crate::color::Color;
use crate::strip::{push_merged, Strip};
use crate::style::Style;

/// A run of text with a single style. May contain `\n` inside a [`Text`]; never inside a [`Strip`].
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Span {
    pub text: String,
    pub style: Style,
}

impl Span {
    pub fn new(text: impl Into<String>, style: Style) -> Self {
        Span {
            text: text.into(),
            style,
        }
    }
    pub fn raw(text: impl Into<String>) -> Self {
        Span {
            text: text.into(),
            style: Style::default(),
        }
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
        Text {
            spans: vec![Span::raw(s)],
        }
    }

    pub fn styled(s: impl Into<String>, style: Style) -> Self {
        Text {
            spans: vec![Span::new(s, style)],
        }
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
        self.paragraphs()
            .iter()
            .map(|p| p.iter().map(|g| g.w).sum::<i32>())
            .max()
            .unwrap_or(0)
    }

    /// Apply `style` underneath every span (span styles take precedence).
    pub fn stylize_base(&mut self, style: &Style) {
        for span in &mut self.spans {
            span.style = style.combine(&span.style);
        }
    }

    /// Split on `\n` without wrapping. An empty text yields zero lines; a trailing
    /// newline does not produce an extra empty line (`"a\n"` → 1 line, `"a\n\nb"` → 3).
    pub fn lines(&self) -> Vec<Strip> {
        let mut paras = self.paragraphs();
        if paras.last().is_some_and(|p| p.is_empty()) {
            paras.pop();
        }
        paras.iter().map(|p| build_strip(p)).collect()
    }

    /// Word-wrap to `width` cells. Splits on `\n` first, then wraps each paragraph at
    /// word boundaries (whitespace collapsed at wrap points; leading indentation kept).
    /// `no_wrap` disables wrapping (lines are then cropped per `overflow`).
    /// Justify Center/Right pads on the left with unstyled spaces; no right padding is added
    /// except for `Full`. Every returned strip has `cell_len() <= width`.
    ///
    /// Details (following Textual's `Content._wrap_and_format`):
    /// - every `\n` starts a new paragraph, so `""` gives one empty strip and `"a\n"` two;
    /// - with `no_wrap` and [`TextOverflow::Fold`] long lines are folded into width-sized
    ///   chunks (as Textual does); `Clip`/`Ellipsis` crop to a single line;
    /// - tabs are expanded to 8-cell tab stops and other control characters are dropped;
    /// - `width <= 0` yields one empty strip per paragraph.
    pub fn wrap(
        &self,
        width: i32,
        justify: Justify,
        overflow: TextOverflow,
        no_wrap: bool,
    ) -> Vec<Strip> {
        let ellipsis = overflow == TextOverflow::Ellipsis;
        let mut out = Vec::new();
        for para in self.paragraphs() {
            if width <= 0 {
                out.push(Strip::default());
                continue;
            }
            let lines: Vec<&[G]> = if no_wrap {
                if overflow == TextOverflow::Fold {
                    chop_cells(&para, width)
                } else {
                    vec![&para[..]]
                }
            } else {
                let breaks = divide_line(&para, width, overflow == TextOverflow::Fold);
                let mut lines = Vec::with_capacity(breaks.len() + 1);
                let mut prev = 0;
                for b in breaks {
                    lines.push(&para[prev..b]);
                    prev = b;
                }
                lines.push(&para[prev..]);
                let last = lines.len() - 1;
                for line in &mut lines[..last] {
                    *line = rstrip(line);
                }
                lines
            };
            let last = lines.len() - 1;
            for (i, line) in lines.into_iter().enumerate() {
                let line = truncate(line, width, ellipsis);
                out.push(justify_line(&line, width, justify, i == last));
            }
        }
        out
    }

    /// Split into paragraphs of graphemes on `\n` (every newline starts a new paragraph).
    fn paragraphs(&self) -> Vec<Vec<G<'_>>> {
        let mut paras: Vec<Vec<G>> = vec![Vec::new()];
        let mut col = 0;
        for span in &self.spans {
            for g in graphemes(&span.text) {
                if g == "\n" || g == "\r\n" {
                    paras.push(Vec::new());
                    col = 0;
                    continue;
                }
                let current = paras.last_mut().expect("at least one paragraph");
                if g == "\t" {
                    let n = TAB_SIZE - col % TAB_SIZE;
                    for _ in 0..n {
                        current.push(G::new(" ", 1, &span.style));
                    }
                    col += n;
                    continue;
                }
                if g.chars().next().is_some_and(|c| c.is_control()) {
                    continue;
                }
                let w = grapheme_width(g);
                current.push(G::new(g, w, &span.style));
                col += w;
            }
        }
        paras
    }

    /// Number of lines after wrapping to `width` (Left justify, Fold overflow).
    pub fn height_for_width(&self, width: i32) -> i32 {
        self.wrap(width, Justify::Left, TextOverflow::Fold, false)
            .len() as i32
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

const TAB_SIZE: i32 = 8;

/// A grapheme with its cell width and style, used internally for wrapping.
#[derive(Debug, Clone, Copy)]
struct G<'a> {
    s: &'a str,
    w: i32,
    style: &'a Style,
}

impl<'a> G<'a> {
    fn new(s: &'a str, w: i32, style: &'a Style) -> Self {
        G { s, w, style }
    }

    /// Breakable whitespace (non-breaking spaces are treated as word characters).
    fn is_ws(&self) -> bool {
        self.s
            .chars()
            .all(|c| c.is_whitespace() && !matches!(c, '\u{a0}' | '\u{2007}' | '\u{202f}'))
    }
}

fn cells(gs: &[G]) -> i32 {
    gs.iter().map(|g| g.w).sum()
}

fn build_strip(gs: &[G]) -> Strip {
    let mut spans = Vec::new();
    for g in gs {
        push_merged(&mut spans, g.s, g.style);
    }
    Strip::new(spans)
}

fn rstrip<'s, 'a>(gs: &'s [G<'a>]) -> &'s [G<'a>] {
    let end = gs.iter().rposition(|g| !g.is_ws()).map_or(0, |i| i + 1);
    &gs[..end]
}

/// Words as `(start, end, trimmed_end)` grapheme indices, like Rich's `\s*\S+\s*`.
fn words(gs: &[G]) -> Vec<(usize, usize, usize)> {
    let mut out = Vec::new();
    let n = gs.len();
    let mut i = 0;
    while i < n {
        let start = i;
        while i < n && gs[i].is_ws() {
            i += 1;
        }
        if i == n {
            break;
        }
        while i < n && !gs[i].is_ws() {
            i += 1;
        }
        let trimmed_end = i;
        while i < n && gs[i].is_ws() {
            i += 1;
        }
        out.push((start, i, trimmed_end));
    }
    out
}

/// Split into pieces of at most `width` cells (a single grapheme wider than `width`
/// gets a piece of its own). Never returns an empty list.
fn chop_cells<'s, 'a>(gs: &'s [G<'a>], width: i32) -> Vec<&'s [G<'a>]> {
    let mut pieces = Vec::new();
    let mut start = 0;
    let mut total = 0;
    for (i, g) in gs.iter().enumerate() {
        if total + g.w > width && i > start {
            pieces.push(&gs[start..i]);
            start = i;
            total = 0;
        }
        total += g.w;
    }
    pieces.push(&gs[start..]);
    pieces
}

/// Grapheme indices to break the paragraph at (port of Rich's `_wrap.divide_line`).
fn divide_line(gs: &[G], width: i32, fold: bool) -> Vec<usize> {
    let mut breaks = Vec::new();
    let mut offset = 0;
    for (start, end, trimmed_end) in words(gs) {
        let word_len = cells(&gs[start..trimmed_end]);
        let full_len = cells(&gs[start..end]);
        if width - offset >= word_len {
            offset += full_len;
        } else if word_len > width {
            if fold {
                let pieces = chop_cells(&gs[start..trimmed_end], width);
                let last = pieces.len() - 1;
                let mut s = start;
                for (i, piece) in pieces.iter().enumerate() {
                    if s > 0 {
                        breaks.push(s);
                    }
                    if i == last {
                        offset = cells(piece) + cells(&gs[trimmed_end..end]);
                    } else {
                        s += piece.len();
                    }
                }
            } else {
                if start > 0 {
                    breaks.push(start);
                }
                offset = full_len;
            }
        } else if offset > 0 && start > 0 {
            breaks.push(start);
            offset = full_len;
        }
    }
    breaks
}

/// Crop to `width` cells, optionally ending with `…`. Trailing whitespace that overflows
/// is dropped first.
fn truncate<'a>(gs: &[G<'a>], width: i32, ellipsis: bool) -> Vec<G<'a>> {
    if cells(gs) <= width {
        return gs.to_vec();
    }
    let stripped = rstrip(gs);
    if cells(stripped) <= width {
        return stripped.to_vec();
    }
    let limit = if ellipsis { width - 1 } else { width };
    let mut out = Vec::new();
    let mut total = 0;
    for g in gs {
        if total + g.w > limit {
            break;
        }
        out.push(*g);
        total += g.w;
    }
    if ellipsis && width >= 1 {
        let style = out.last().unwrap_or(&gs[out.len()]).style;
        out.push(G::new("…", 1, style));
    }
    out
}

fn justify_line(gs: &[G], width: i32, justify: Justify, last: bool) -> Strip {
    match justify {
        Justify::Left => build_strip(gs),
        Justify::Center | Justify::Right => {
            let gs = rstrip(gs);
            let excess = width - cells(gs);
            let pad = if justify == Justify::Center {
                excess / 2
            } else {
                excess
            };
            let mut strip = build_strip(gs);
            if pad > 0 {
                strip.spans.insert(0, Span::raw(" ".repeat(pad as usize)));
            }
            strip
        }
        Justify::Full => {
            if last || cells(gs) >= width {
                return build_strip(gs);
            }
            let mut words: Vec<&[G]> = Vec::new();
            let mut seps: Vec<&Style> = Vec::new();
            let mut start = 0;
            for (i, g) in gs.iter().enumerate() {
                if g.s == " " {
                    words.push(&gs[start..i]);
                    seps.push(g.style);
                    start = i + 1;
                }
            }
            words.push(&gs[start..]);
            let n = seps.len();
            if n == 0 {
                return build_strip(gs);
            }
            // Distribute extra spaces from the right, like Textual's `_FormattedLine`.
            let mut spaces = vec![1; n];
            let mut total = words.iter().map(|w| cells(w)).sum::<i32>() + n as i32;
            let mut index = 0;
            while total < width {
                spaces[n - index - 1] += 1;
                total += 1;
                index = (index + 1) % n;
            }
            let mut spans = Vec::new();
            for (i, word) in words.iter().enumerate() {
                for g in word.iter() {
                    push_merged(&mut spans, g.s, g.style);
                }
                if i < n {
                    push_merged(&mut spans, &" ".repeat(spaces[i]), seps[i]);
                }
            }
            Strip::new(spans)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(strips: &[Strip]) -> Vec<String> {
        strips.iter().map(|s| s.plain_text()).collect()
    }

    fn wrap(s: &str, width: i32) -> Vec<String> {
        texts(&Text::plain(s).wrap(width, Justify::Left, TextOverflow::Fold, false))
    }

    fn red() -> Style {
        Style::new().fg(Color::rgb(255, 0, 0))
    }

    #[test]
    fn cell_len_widest_line() {
        assert_eq!(Text::plain("ab\nabcd\n").cell_len(), 4);
        assert_eq!(Text::plain("中文").cell_len(), 4);
        assert_eq!(Text::new().cell_len(), 0);
    }

    #[test]
    fn lines_split() {
        assert!(Text::plain("").lines().is_empty());
        assert!(Text::new().lines().is_empty());
        assert_eq!(texts(&Text::plain("a\n").lines()), vec!["a"]);
        assert_eq!(texts(&Text::plain("a\n\nb").lines()), vec!["a", "", "b"]);
        assert_eq!(texts(&Text::plain("\n").lines()), vec![""]);
        let mut t = Text::new();
        t.push("ab\nc", red()).push("d", Style::default());
        let lines = t.lines();
        assert_eq!(texts(&lines), vec!["ab", "cd"]);
        assert_eq!(lines[1].spans[0].style, red());
        assert_eq!(lines[1].spans[1].style, Style::default());
    }

    #[test]
    fn stylize_base_under() {
        let mut t = Text::styled("x", red());
        t.push("y", Style::default());
        t.stylize_base(&Style::new().fg(Color::rgb(0, 0, 255)).bold());
        assert_eq!(t.spans[0].style.fg, Some(Color::rgb(255, 0, 0)));
        assert_eq!(t.spans[0].style.bold, Some(true));
        assert_eq!(t.spans[1].style.fg, Some(Color::rgb(0, 0, 255)));
    }

    #[test]
    fn wrap_simple() {
        assert_eq!(wrap("hello world", 20), vec!["hello world"]);
        assert_eq!(wrap("hello world", 5), vec!["hello", "world"]);
        assert_eq!(wrap("hello world", 8), vec!["hello", "world"]);
        assert_eq!(wrap("hello world", 11), vec!["hello world"]);
        assert_eq!(
            wrap("the quick brown fox", 10),
            vec!["the quick", "brown fox"]
        );
    }

    #[test]
    fn wrap_collapses_whitespace_at_breaks() {
        assert_eq!(wrap("ab     cd", 4), vec!["ab", "cd"]);
        assert_eq!(wrap("  indented text", 10), vec!["  indented", "text"]);
    }

    #[test]
    fn wrap_paragraphs() {
        assert_eq!(wrap("a\n\nb", 10), vec!["a", "", "b"]);
        assert_eq!(wrap("", 10), vec![""]);
        assert_eq!(wrap("a\n", 10), vec!["a", ""]);
        assert_eq!(Text::plain("one two\nthree").height_for_width(5), 3);
    }

    #[test]
    fn wrap_fold_long_word() {
        assert_eq!(wrap("abcdefghij", 4), vec!["abcd", "efgh", "ij"]);
        assert_eq!(wrap("x abcdefghij y", 4), vec!["x", "abcd", "efgh", "ij y"]);
    }

    #[test]
    fn wrap_clip_and_ellipsis() {
        let t = Text::plain("x abcdefghij y");
        let clip = texts(&t.wrap(4, Justify::Left, TextOverflow::Clip, false));
        assert_eq!(clip, vec!["x", "abcd", "y"]);
        let ell = texts(&t.wrap(4, Justify::Left, TextOverflow::Ellipsis, false));
        assert_eq!(ell, vec!["x", "abc…", "y"]);
    }

    #[test]
    fn no_wrap_modes() {
        let t = Text::plain("hello world");
        let clip = texts(&t.wrap(7, Justify::Left, TextOverflow::Clip, true));
        assert_eq!(clip, vec!["hello w"]);
        let ell = texts(&t.wrap(7, Justify::Left, TextOverflow::Ellipsis, true));
        assert_eq!(ell, vec!["hello …"]);
        let fold = texts(&t.wrap(4, Justify::Left, TextOverflow::Fold, true));
        assert_eq!(fold, vec!["hell", "o wo", "rld"]);
        let short = texts(&t.wrap(20, Justify::Left, TextOverflow::Clip, true));
        assert_eq!(short, vec!["hello world"]);
    }

    #[test]
    fn ellipsis_ignores_overflowing_trailing_space() {
        let t = Text::plain("abc   ");
        let ell = texts(&t.wrap(4, Justify::Left, TextOverflow::Ellipsis, true));
        assert_eq!(ell, vec!["abc"]);
    }

    #[test]
    fn wrap_keeps_styles() {
        let mut t = Text::new();
        t.push("hello ", red()).push("big world", Style::default());
        let lines = t.wrap(9, Justify::Left, TextOverflow::Fold, false);
        assert_eq!(texts(&lines), vec!["hello big", "world"]);
        assert_eq!(lines[0].spans.len(), 2);
        assert_eq!(lines[0].spans[0].text, "hello ");
        assert_eq!(lines[0].spans[0].style, red());
        assert_eq!(lines[1].spans[0].style, Style::default());
    }

    #[test]
    fn justify_center_right() {
        let t = Text::plain("ab cd");
        let c = t.wrap(9, Justify::Center, TextOverflow::Fold, false);
        assert_eq!(texts(&c), vec!["  ab cd"]);
        assert!(c[0].spans[0].style.is_empty());
        let r = t.wrap(9, Justify::Right, TextOverflow::Fold, false);
        assert_eq!(texts(&r), vec!["    ab cd"]);
        let r2 = Text::plain("aaa bbbb").wrap(5, Justify::Right, TextOverflow::Fold, false);
        assert_eq!(texts(&r2), vec!["  aaa", " bbbb"]);
    }

    #[test]
    fn justify_full() {
        let t = Text::plain("a b c dddddd e");
        let f = t.wrap(7, Justify::Full, TextOverflow::Fold, false);
        assert_eq!(texts(&f), vec!["a  b  c", "dddddd", "e"]);
        let f2 = Text::plain("aa b cc dd").wrap(8, Justify::Full, TextOverflow::Fold, false);
        assert_eq!(texts(&f2), vec!["aa b  cc", "dd"]);
        assert_eq!(f2[0].cell_len(), 8);
    }

    #[test]
    fn wrap_wide_chars() {
        assert_eq!(wrap("中文字符测试", 5), vec!["中文", "字符", "测试"]);
        assert_eq!(wrap("ab 中文字", 4), vec!["ab", "中文", "字"]);
        // width 1 can't hold a wide char at all: lines are empty but valid
        for s in Text::plain("中").wrap(1, Justify::Left, TextOverflow::Fold, false) {
            assert!(s.cell_len() <= 1);
        }
        let e = Text::plain("中文字").wrap(4, Justify::Left, TextOverflow::Ellipsis, true);
        assert_eq!(texts(&e), vec!["中…"]);
    }

    #[test]
    fn wrap_never_splits_graphemes() {
        let family = "👨\u{200d}👩\u{200d}👧";
        let s = format!("{family}{family}{family}");
        assert_eq!(wrap(&s, 3), vec![family, family, family]);
        let accented = "e\u{301}e\u{301}e\u{301}";
        assert_eq!(wrap(accented, 2), vec!["e\u{301}e\u{301}", "e\u{301}"]);
    }

    #[test]
    fn wrap_width_property() {
        let sample = "Lorem ipsum dolor sit amet, 中文 consectetur  adipiscing\n\
                      elit 😀 sed do eiusmod tempor incididunt";
        let t = Text::plain(sample);
        for width in 1..30 {
            for justify in [
                Justify::Left,
                Justify::Center,
                Justify::Right,
                Justify::Full,
            ] {
                for overflow in [
                    TextOverflow::Fold,
                    TextOverflow::Clip,
                    TextOverflow::Ellipsis,
                ] {
                    for no_wrap in [false, true] {
                        for s in t.wrap(width, justify, overflow, no_wrap) {
                            assert!(
                                s.cell_len() <= width,
                                "{width} {justify:?} {overflow:?} {no_wrap}: {:?}",
                                s.plain_text()
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn wrap_zero_width() {
        assert_eq!(wrap("abc\ndef", 0), vec!["", ""]);
    }

    #[test]
    fn tabs_expand() {
        assert_eq!(texts(&Text::plain("a\tb").lines()), vec!["a       b"]);
        assert_eq!(Text::plain("a\tb").cell_len(), 9);
    }
}

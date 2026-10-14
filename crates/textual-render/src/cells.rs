//! Unicode cell-width helpers.

use unicode_width::UnicodeWidthStr;

/// Width in terminal cells of a string (wide CJK = 2, combining = 0).
pub fn cell_len(s: &str) -> i32 {
    if s.is_ascii() {
        // Fast path: printable ASCII is one cell each; control codes take no space.
        return s.bytes().filter(|b| !b.is_ascii_control()).count() as i32;
    }
    graphemes(s).map(grapheme_width).sum()
}

/// Width of a single grapheme cluster.
///
/// Control characters are zero width. Everything else is measured with `unicode-width`
/// (which understands emoji presentation sequences and ZWJ sequences) and capped at 2,
/// since a terminal never renders a single cluster wider than two cells.
pub fn grapheme_width(g: &str) -> i32 {
    let mut chars = g.chars();
    match chars.next() {
        None => 0,
        Some(c) if c.is_control() => 0,
        Some(_) if g.len() == 1 => 1,
        Some(_) => (UnicodeWidthStr::width(g) as i32).clamp(0, 2),
    }
}

/// Iterate extended grapheme clusters.
pub fn graphemes(s: &str) -> impl Iterator<Item = &str> {
    unicode_segmentation::UnicodeSegmentation::graphemes(s, true)
}

/// Truncate to at most `max` cells. Returns the truncated string and its cell width.
/// Graphemes are never split; a wide character that would straddle `max` is dropped.
pub fn truncate(s: &str, max: i32) -> (String, i32) {
    let mut out = String::new();
    let mut width = 0;
    for g in graphemes(s) {
        let w = grapheme_width(g);
        if width + w > max {
            break;
        }
        out.push_str(g);
        width += w;
    }
    (out, width)
}

/// Pad with spaces or truncate so the result is exactly `width` cells.
/// When a wide character would straddle the edge it is dropped and replaced by a space.
pub fn set_cell_len(s: &str, width: i32) -> String {
    let (mut out, w) = truncate(s, width.max(0));
    for _ in w..width {
        out.push(' ');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_width() {
        assert_eq!(cell_len(""), 0);
        assert_eq!(cell_len("hello"), 5);
        assert_eq!(cell_len("a\nb"), 2);
    }

    #[test]
    fn wide_width() {
        assert_eq!(cell_len("中文"), 4);
        assert_eq!(cell_len("a中b"), 4);
        assert_eq!(grapheme_width("中"), 2);
        assert_eq!(grapheme_width("😀"), 2);
    }

    #[test]
    fn combining_and_zwj() {
        // e + combining acute accent is a single 1-cell grapheme
        assert_eq!(cell_len("e\u{301}"), 1);
        assert_eq!(graphemes("e\u{301}x").count(), 2);
        // family emoji (ZWJ sequence) is one grapheme, 2 cells
        let family = "👨\u{200d}👩\u{200d}👧";
        assert_eq!(graphemes(family).count(), 1);
        assert_eq!(cell_len(family), 2);
        // emoji presentation selector
        assert_eq!(cell_len("❤\u{fe0f}"), 2);
    }

    #[test]
    fn truncate_basic() {
        assert_eq!(truncate("hello", 3), ("hel".to_string(), 3));
        assert_eq!(truncate("hi", 5), ("hi".to_string(), 2));
        assert_eq!(truncate("hi", 0), (String::new(), 0));
    }

    #[test]
    fn truncate_wide() {
        assert_eq!(truncate("中文字", 3), ("中".to_string(), 2));
        assert_eq!(truncate("中文字", 4), ("中文".to_string(), 4));
    }

    #[test]
    fn set_len() {
        assert_eq!(set_cell_len("ab", 4), "ab  ");
        assert_eq!(set_cell_len("abcdef", 3), "abc");
        assert_eq!(set_cell_len("中文", 3), "中 ");
        assert_eq!(set_cell_len("x", 0), "");
        assert_eq!(set_cell_len("x", -1), "");
    }
}

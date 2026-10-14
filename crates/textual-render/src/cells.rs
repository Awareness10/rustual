//! Unicode cell-width helpers.

/// Width in terminal cells of a string (wide CJK = 2, combining = 0).
pub fn cell_len(s: &str) -> i32 {
    todo!("{s}")
}

/// Width of a single grapheme cluster.
pub fn grapheme_width(g: &str) -> i32 {
    todo!("{g}")
}

/// Iterate extended grapheme clusters.
pub fn graphemes(s: &str) -> impl Iterator<Item = &str> {
    unicode_segmentation::UnicodeSegmentation::graphemes(s, true)
}

/// Truncate to at most `max` cells. Returns the truncated string and its cell width.
pub fn truncate(s: &str, max: i32) -> (String, i32) {
    todo!("{s}{max}")
}

/// Pad with spaces or truncate so the result is exactly `width` cells.
pub fn set_cell_len(s: &str, width: i32) -> String {
    todo!("{s}{width}")
}

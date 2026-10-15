//! Wrapping-aware cursor navigation (port of `textual/document/_document_navigator.py`).
//!
//! Cursor locations live in document space, but vertical movement, home and end follow the
//! *wrapped* (visual) lines. Horizontal movement steps over whole grapheme clusters.

use super::document::{char_len, is_word_char, line_cells, Document, Location};
use super::wrap::WrappedDocument;
use rustual_render::cells::graphemes;

/// Navigation queries over a document and its wrapped view.
pub struct Navigator<'a> {
    pub document: &'a Document,
    pub wrapped: &'a WrappedDocument,
    /// The remembered x offset (cells) used for vertical movement.
    pub last_x_offset: i32,
}

/// Character offsets of grapheme cluster starts in `line` (plus the line length).
fn grapheme_boundaries(line: &str) -> Vec<usize> {
    let mut out = Vec::new();
    let mut col = 0;
    for g in graphemes(line) {
        out.push(col);
        col += char_len(g);
    }
    out.push(col);
    out
}

impl<'a> Navigator<'a> {
    pub fn new(document: &'a Document, wrapped: &'a WrappedDocument, last_x_offset: i32) -> Self {
        Navigator {
            document,
            wrapped,
            last_x_offset,
        }
    }

    pub fn is_end_of_document_line(&self, (row, col): Location) -> bool {
        col >= self.document.line_len(row)
    }

    pub fn is_last_document_line(&self, (row, _): Location) -> bool {
        row + 1 >= self.document.line_count()
    }

    pub fn is_end_of_document(&self, location: Location) -> bool {
        self.is_last_document_line(location) && self.is_end_of_document_line(location)
    }

    /// One grapheme to the left, or the end of the previous line.
    pub fn get_location_left(&self, (row, col): Location) -> Location {
        if (row, col) == (0, 0) {
            return (0, 0);
        }
        if col == 0 {
            return (row - 1, self.document.line_len(row - 1));
        }
        let line = self.document.get_line(row);
        if line.is_ascii() {
            return (row, col - 1);
        }
        let b = grapheme_boundaries(line);
        let target = b.iter().rev().find(|&&c| c < col).copied().unwrap_or(0);
        (row, target)
    }

    /// One grapheme to the right, or the start of the next line.
    pub fn get_location_right(&self, location: Location) -> Location {
        if self.is_end_of_document(location) {
            return location;
        }
        let (row, col) = location;
        if self.is_end_of_document_line(location) {
            return (row + 1, 0);
        }
        let line = self.document.get_line(row);
        if line.is_ascii() {
            return (row, col + 1);
        }
        let b = grapheme_boundaries(line);
        let target = b
            .iter()
            .find(|&&c| c > col)
            .copied()
            .unwrap_or(char_len(line));
        (row, target)
    }

    /// The visual x offset of `location` within its wrapped section.
    fn section_x(&self, location: Location) -> i32 {
        self.wrapped.location_to_offset(self.document, location).0
    }

    /// The location visually above (`max(current x, last_x_offset)` is kept).
    pub fn get_location_above(&self, location: Location) -> Location {
        let (row, col) = location;
        let section = self.wrapped.section_of(row, col);
        let target_x = self.section_x(location).max(self.last_x_offset);
        if section == 0 {
            if row == 0 {
                return (0, 0);
            }
            let target_row = row - 1;
            let last_section = self.wrapped.section_count(target_row) - 1;
            let c = self
                .wrapped
                .target_column(self.document, target_row, target_x, last_section);
            (target_row, c)
        } else {
            let c = self
                .wrapped
                .target_column(self.document, row, target_x, section - 1);
            (row, c)
        }
    }

    /// The location visually below.
    pub fn get_location_below(&self, location: Location) -> Location {
        let (row, col) = location;
        let section = self.wrapped.section_of(row, col);
        let target_x = self.section_x(location).max(self.last_x_offset);
        if section + 1 >= self.wrapped.section_count(row) {
            if self.is_last_document_line(location) {
                return (row, self.document.line_len(row));
            }
            let c = self
                .wrapped
                .target_column(self.document, row + 1, target_x, 0);
            (row + 1, c)
        } else {
            let c = self
                .wrapped
                .target_column(self.document, row, target_x, section + 1);
            (row, c)
        }
    }

    /// End of the current wrapped section (or of the line).
    pub fn get_location_end(&self, (row, col): Location) -> Location {
        let offsets = self.wrapped.get_offsets(row);
        let next = offsets.partition_point(|&o| o <= col);
        if next < offsets.len() {
            (row, offsets[next] - 1)
        } else {
            (row, self.document.line_len(row))
        }
    }

    /// Start of the current wrapped section. With `smart_home` (unwrapped lines only), the
    /// first non-whitespace character, unless the cursor is already there.
    pub fn get_location_home(&self, (row, col): Location, smart_home: bool) -> Location {
        let offsets = self.wrapped.get_offsets(row);
        if !offsets.is_empty() {
            let prev = offsets.partition_point(|&o| o <= col);
            return if prev == 0 {
                (row, 0)
            } else {
                (row, offsets[prev - 1])
            };
        }
        if smart_home {
            let line = self.document.get_line(row);
            let target = line.chars().position(|c| !c.is_whitespace()).unwrap_or(0);
            if col == 0 || col > target {
                return (row, target);
            }
        }
        (row, 0)
    }

    /// The location `dy` visual lines away, at the same x offset.
    pub fn get_location_at_y_offset(&self, location: Location, dy: i32) -> Location {
        let (x, y) = self.wrapped.location_to_offset(self.document, location);
        self.wrapped
            .offset_to_location(self.document, x, y as i32 + dy)
    }

    /// Start of the previous word (Textual's `get_cursor_word_left_location`).
    pub fn get_location_word_left(&self, (row, col): Location) -> Location {
        if row > 0 && col == 0 {
            return (row - 1, self.document.line_len(row - 1));
        }
        let chars: Vec<char> = self.document.get_line(row).chars().take(col).collect();
        let mut end = chars.len();
        while end > 0 && chars[end - 1].is_whitespace() {
            end -= 1;
        }
        let target = (1..end)
            .rev()
            .find(|&i| is_word_char(chars[i - 1]) != is_word_char(chars[i]))
            .unwrap_or(0);
        (row, target)
    }

    /// Next word boundary to the right, skipping leading whitespace.
    pub fn get_location_word_right(&self, (row, col): Location) -> Location {
        let line_len = self.document.line_len(row);
        if row + 1 < self.document.line_count() && col >= line_len {
            return (row + 1, 0);
        }
        let chars: Vec<char> = self.document.get_line(row).chars().skip(col).collect();
        let strip = chars.iter().take_while(|c| c.is_whitespace()).count();
        let rest = &chars[strip..];
        match (1..rest.len()).find(|&i| is_word_char(rest[i - 1]) != is_word_char(rest[i])) {
            Some(i) => (row, col + strip + i),
            None => (row, line_len),
        }
    }

    /// Target of "delete word right": the end of the first word after the cursor (Textual's
    /// `\s*\w+` search), else the next line start / line end.
    pub fn get_delete_word_right_target(&self, (row, col): Location) -> Location {
        let chars: Vec<char> = self.document.get_line(row).chars().skip(col).collect();
        if let Some(first) = chars.iter().position(|&c| is_word_char(c)) {
            let end = chars[first..]
                .iter()
                .position(|&c| !is_word_char(c))
                .map(|p| first + p)
                .unwrap_or(chars.len());
            return (row, col + end);
        }
        let line_len = self.document.line_len(row);
        if row + 1 < self.document.line_count() && col == line_len {
            (row + 1, 0)
        } else {
            (row, line_len)
        }
    }

    /// The word (run of word characters) around `location`, as a column range.
    pub fn word_range_at(&self, (row, col): Location) -> (usize, usize) {
        let chars: Vec<char> = self.document.get_line(row).chars().collect();
        if chars.is_empty() {
            return (0, 0);
        }
        let col = col.min(chars.len() - 1);
        let kind = is_word_char(chars[col]);
        let mut start = col;
        while start > 0 && is_word_char(chars[start - 1]) == kind {
            start -= 1;
        }
        let mut end = col;
        while end < chars.len() && is_word_char(chars[end]) == kind {
            end += 1;
        }
        (start, end)
    }

    /// Cell width of the line up to `col` (tabs expanded).
    pub fn column_width(&self, (row, col): Location) -> i32 {
        let cells = line_cells(self.document.get_line(row), self.wrapped.tab_width());
        cells[..col.min(cells.len())].iter().sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nav_test(text: &str, width: i32, f: impl FnOnce(&Navigator)) {
        let doc = Document::new(text);
        let wrapped = WrappedDocument::new(&doc, width, 4);
        let nav = Navigator::new(&doc, &wrapped, 0);
        f(&nav);
    }

    #[test]
    fn left_right() {
        nav_test("ab\ncd", 0, |n| {
            assert_eq!(n.get_location_left((1, 0)), (0, 2));
            assert_eq!(n.get_location_right((0, 2)), (1, 0));
            assert_eq!(n.get_location_right((1, 2)), (1, 2));
            assert_eq!(n.get_location_left((0, 0)), (0, 0));
        });
        // ZWJ family emoji is one grapheme of several chars.
        nav_test("a👨‍👩‍👧b", 0, |n| {
            assert_eq!(n.get_location_right((0, 1)), (0, 6));
            assert_eq!(n.get_location_left((0, 6)), (0, 1));
        });
    }

    #[test]
    fn up_down_wrapped() {
        // "hello " | "world" at width 6.
        nav_test("hello world\nxy", 6, |n| {
            assert_eq!(n.get_location_below((0, 2)), (0, 8));
            assert_eq!(n.get_location_below((0, 8)), (1, 2));
            assert_eq!(n.get_location_above((1, 1)), (0, 7));
            assert_eq!(n.get_location_above((0, 7)), (0, 1));
            assert_eq!(n.get_location_above((0, 1)), (0, 0));
            assert_eq!(n.get_location_below((1, 1)), (1, 2));
            assert_eq!(n.get_location_end((0, 1)), (0, 5));
            assert_eq!(n.get_location_home((0, 8), true), (0, 6));
        });
    }

    #[test]
    fn up_down_cjk_alignment() {
        nav_test("日本語\nabcdef", 0, |n| {
            // Column 2 of line 0 is at x=4.
            assert_eq!(n.get_location_below((0, 2)), (1, 4));
            assert_eq!(n.get_location_above((1, 3)), (0, 1));
        });
    }

    #[test]
    fn smart_home_and_words() {
        nav_test("    foo bar_baz(x)", 0, |n| {
            assert_eq!(n.get_location_home((0, 10), true), (0, 4));
            assert_eq!(n.get_location_home((0, 4), true), (0, 0));
            assert_eq!(n.get_location_home((0, 0), true), (0, 4));
            assert_eq!(n.get_location_word_right((0, 0)), (0, 7));
            assert_eq!(n.get_location_word_right((0, 7)), (0, 15));
            assert_eq!(n.get_location_word_left((0, 15)), (0, 8));
            assert_eq!(n.get_location_word_left((0, 8)), (0, 4));
            assert_eq!(n.get_delete_word_right_target((0, 7)), (0, 15));
            assert_eq!(n.word_range_at((0, 10)), (8, 15));
        });
    }
}

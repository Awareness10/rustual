//! The document model behind [`super::TextArea`] (port of `textual/document/_document.py`).
//!
//! A [`Document`] is a list of lines (without line separators) plus the newline style used to
//! join them. Positions are [`Location`]s: `(row, column)` where `column` is a *character*
//! (Unicode scalar value) index, like Textual's codepoint offsets.

use rustual_render::cells::{grapheme_width, graphemes};

/// A `(row, column)` position in a document. Columns are character indices.
pub type Location = (usize, usize);

/// A range of the document from `start` (the anchor) to `end` (the cursor).
///
/// The range is inclusive of the minimum location and exclusive of the maximum one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Selection {
    pub start: Location,
    pub end: Location,
}

impl Selection {
    pub fn new(start: Location, end: Location) -> Self {
        Selection { start, end }
    }

    /// A zero-width selection (just a cursor) at `location`.
    pub fn cursor(location: Location) -> Self {
        Selection {
            start: location,
            end: location,
        }
    }

    /// True if nothing is selected.
    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }

    /// `(top, bottom)` in document order.
    pub fn sorted(&self) -> (Location, Location) {
        if self.start <= self.end {
            (self.start, self.end)
        } else {
            (self.end, self.start)
        }
    }

    /// True if row `y` intersects the selection.
    pub fn contains_line(&self, y: usize) -> bool {
        let (top, bottom) = self.sorted();
        y >= top.0 && y <= bottom.0
    }
}

/// Information about a completed edit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditResult {
    /// The location at the end of the inserted text.
    pub end_location: Location,
    /// The text that was replaced (removed).
    pub replaced_text: String,
}

/// A line separator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Newline {
    #[default]
    Lf,
    CrLf,
    Cr,
}

impl Newline {
    pub fn as_str(&self) -> &'static str {
        match self {
            Newline::Lf => "\n",
            Newline::CrLf => "\r\n",
            Newline::Cr => "\r",
        }
    }

    /// The newline style used in `text` (first of `\r\n`, `\n`, `\r` found; default `\n`).
    pub fn detect(text: &str) -> Newline {
        if text.contains("\r\n") {
            Newline::CrLf
        } else if text.contains('\n') {
            Newline::Lf
        } else if text.contains('\r') {
            Newline::Cr
        } else {
            Newline::Lf
        }
    }
}

/// Split on `\r\n`, `\n` or `\r`. A trailing separator yields a final empty line and an empty
/// string yields one empty line (Textual's `splitlines()` + trailing `""` rule).
pub fn split_lines(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let bytes = text.as_bytes();
    let mut start = 0;
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'\n' => {
                out.push(text[start..i].to_string());
                start = i + 1;
            }
            b'\r' => {
                out.push(text[start..i].to_string());
                if bytes.get(i + 1) == Some(&b'\n') {
                    i += 1;
                }
                start = i + 1;
            }
            _ => {}
        }
        i += 1;
    }
    out.push(text[start..].to_string());
    out
}

/// Number of characters in `s`.
pub fn char_len(s: &str) -> usize {
    if s.is_ascii() {
        s.len()
    } else {
        s.chars().count()
    }
}

/// Byte index of character `col` (clamped to the end).
pub fn byte_index(s: &str, col: usize) -> usize {
    if s.is_ascii() {
        return col.min(s.len());
    }
    s.char_indices().nth(col).map(|(b, _)| b).unwrap_or(s.len())
}

/// Characters `start..end` of `s` (clamped).
pub fn slice_chars(s: &str, start: usize, end: usize) -> &str {
    let a = byte_index(s, start);
    let b = byte_index(s, end.max(start));
    &s[a..b]
}

/// Cell width of every character in `line`, with tabs expanded to `tab_width` tab stops.
///
/// The width of a grapheme cluster is attributed to its first character (the other characters
/// of the cluster are zero-width), so the sum matches how the line is rendered. Control
/// characters other than tab are rendered as `�` (one cell).
pub fn line_cells(line: &str, tab_width: usize) -> Vec<i32> {
    let tab_width = tab_width.max(1) as i32;
    let mut out = Vec::with_capacity(line.len());
    let mut pos = 0i32;
    if line.is_ascii() {
        for b in line.bytes() {
            let w = if b == b'\t' {
                tab_width - pos % tab_width
            } else {
                1
            };
            out.push(w);
            pos += w;
        }
        return out;
    }
    for g in graphemes(line) {
        let mut chars = g.chars();
        let first = chars.next().unwrap_or(' ');
        let w = if first == '\t' {
            tab_width - pos % tab_width
        } else if first.is_control() {
            1
        } else {
            grapheme_width(g)
        };
        out.push(w);
        pos += w;
        out.extend(std::iter::repeat_n(0, chars.count()));
    }
    out
}

/// The text to draw for character `c` occupying `width` cells (tabs → spaces, controls → `�`).
pub fn display_char(c: char, width: i32, out: &mut String) {
    if c == '\t' {
        out.extend(std::iter::repeat_n(' ', width.max(0) as usize));
    } else if c.is_control() {
        out.push('\u{fffd}');
    } else {
        out.push(c);
    }
}

/// Column (character index) of the character covering cell `x` within a line whose
/// per-character widths are `cells` (Textual's `cell_width_to_column_index`).
/// Cells past the end of the line map to the line length.
pub fn cell_to_column(cells: &[i32], x: i32) -> usize {
    let mut total = 0;
    for (i, w) in cells.iter().enumerate() {
        total += w;
        if total > x {
            // Land on the start of a grapheme cluster.
            let mut i = i;
            while i > 0 && cells[i] == 0 {
                i -= 1;
            }
            return i;
        }
    }
    cells.len()
}

/// Python-style `\w`: alphanumeric or underscore.
pub fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// A document: lines of text plus a newline style.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Document {
    lines: Vec<String>,
    newline: Newline,
}

impl Default for Document {
    fn default() -> Self {
        Document::new("")
    }
}

impl Document {
    pub fn new(text: &str) -> Self {
        Document {
            lines: split_lines(text),
            newline: Newline::detect(text),
        }
    }

    /// The lines (without separators). Never empty.
    pub fn lines(&self) -> &[String] {
        &self.lines
    }

    /// The full text, lines joined with the document's newline.
    pub fn text(&self) -> String {
        self.lines.join(self.newline.as_str())
    }

    pub fn newline(&self) -> Newline {
        self.newline
    }

    pub fn set_newline(&mut self, newline: Newline) {
        self.newline = newline;
    }

    pub fn line_count(&self) -> usize {
        self.lines.len()
    }

    /// Line `index` (empty string if out of range).
    pub fn get_line(&self, index: usize) -> &str {
        self.lines.get(index).map(String::as_str).unwrap_or("")
    }

    /// Length (in characters) of line `index`.
    pub fn line_len(&self, index: usize) -> usize {
        char_len(self.get_line(index))
    }

    /// `(0, 0)`.
    pub fn start(&self) -> Location {
        (0, 0)
    }

    /// The location after the last character.
    pub fn end(&self) -> Location {
        let last = self.lines.len() - 1;
        (last, self.line_len(last))
    }

    /// True if the document contains no text.
    pub fn is_empty(&self) -> bool {
        self.lines.len() == 1 && self.lines[0].is_empty()
    }

    /// Clamp a location to a valid position in the document.
    pub fn clamp(&self, location: Location) -> Location {
        let row = location.0.min(self.lines.len() - 1);
        (row, location.1.min(self.line_len(row)))
    }

    /// Size of the document's bounding box in cells: (max line width, line count).
    pub fn get_size(&self, tab_width: usize) -> (i32, usize) {
        let width = self
            .lines
            .iter()
            .map(|l| line_cells(l, tab_width).iter().sum::<i32>())
            .max()
            .unwrap_or(0);
        (width, self.lines.len())
    }

    /// Replace the text between `start` and `end` (in either order) with `text`.
    /// This is the only way the document is modified.
    pub fn replace_range(&mut self, start: Location, end: Location, text: &str) -> EditResult {
        let (top, bottom) = if start <= end {
            (start, end)
        } else {
            (end, start)
        };
        let (top, bottom) = (self.clamp(top), self.clamp(bottom));
        let replaced_text = self.get_text_range(top, bottom);
        let mut insert_lines = split_lines(text);

        let top_line = &self.lines[top.0];
        let before = slice_chars(top_line, 0, top.1).to_string();
        let bottom_line = &self.lines[bottom.0];
        let after = bottom_line[byte_index(bottom_line, bottom.1)..].to_string();

        insert_lines[0].insert_str(0, &before);
        let last = insert_lines.last_mut().expect("split_lines is never empty");
        let destination_column = char_len(last);
        last.push_str(&after);
        let destination_row = top.0 + insert_lines.len() - 1;
        self.lines.splice(top.0..=bottom.0, insert_lines);
        EditResult {
            end_location: (destination_row, destination_column),
            replaced_text,
        }
    }

    /// The text between two locations (in either order), joined with the document newline.
    pub fn get_text_range(&self, start: Location, end: Location) -> String {
        if start == end {
            return String::new();
        }
        let (top, bottom) = if start <= end {
            (start, end)
        } else {
            (end, start)
        };
        let (top, bottom) = (self.clamp(top), self.clamp(bottom));
        if top.0 == bottom.0 {
            return slice_chars(&self.lines[top.0], top.1, bottom.1).to_string();
        }
        let nl = self.newline.as_str();
        let first = &self.lines[top.0];
        let mut out = first[byte_index(first, top.1)..].to_string();
        for row in top.0 + 1..bottom.0 {
            out.push_str(nl);
            out.push_str(&self.lines[row]);
        }
        out.push_str(nl);
        out.push_str(slice_chars(&self.lines[bottom.0], 0, bottom.1));
        out
    }

    /// Character index into [`Document::text`] for a location.
    pub fn get_index_from_location(&self, location: Location) -> usize {
        let (row, column) = self.clamp(location);
        let nl = self.newline.as_str().len();
        let mut index = row * nl + column;
        for line in &self.lines[..row] {
            index += char_len(line);
        }
        index
    }

    /// Location of a character index into [`Document::text`] (`None` if out of range).
    pub fn get_location_from_index(&self, index: usize) -> Option<Location> {
        let nl = self.newline.as_str().len();
        let mut column_index = 0;
        for (row, line) in self.lines.iter().enumerate() {
            let len = char_len(line);
            if index <= column_index + len {
                return Some((row, index - column_index));
            }
            column_index += len + nl;
            if index < column_index {
                // Inside a CRLF pair.
                return None;
            }
        }
        None
    }
}

impl std::ops::Index<usize> for Document {
    type Output = str;
    fn index(&self, index: usize) -> &str {
        &self.lines[index]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_and_join() {
        assert_eq!(split_lines(""), vec![""]);
        assert_eq!(split_lines("a\n"), vec!["a", ""]);
        assert_eq!(split_lines("a\r\nb\rc"), vec!["a", "b", "c"]);
        let d = Document::new("x\r\ny");
        assert_eq!(d.newline(), Newline::CrLf);
        assert_eq!(d.text(), "x\r\ny");
        assert_eq!(d.line_count(), 2);
        assert_eq!(Document::new("abc\n").end(), (1, 0));
    }

    #[test]
    fn replace_range_insert_delete() {
        let mut d = Document::new("hello\nworld");
        let r = d.replace_range((0, 5), (0, 5), " there");
        assert_eq!(r.end_location, (0, 11));
        assert_eq!(d.text(), "hello there\nworld");
        let r = d.replace_range((0, 5), (1, 2), "");
        assert_eq!(r.replaced_text, " there\nwo");
        assert_eq!(r.end_location, (0, 5));
        assert_eq!(d.text(), "hellorld");
        let r = d.replace_range((0, 2), (0, 2), "X\nY\n");
        assert_eq!(r.end_location, (2, 0));
        assert_eq!(d.text(), "heX\nY\nllorld");
        // Reversed range.
        let r = d.replace_range((2, 2), (0, 2), "-");
        assert_eq!(r.replaced_text, "X\nY\nll");
        assert_eq!(d.text(), "he-orld");
    }

    #[test]
    fn unicode_columns() {
        let mut d = Document::new("日本語🎉x");
        assert_eq!(d.line_len(0), 5);
        d.replace_range((0, 1), (0, 2), "");
        assert_eq!(d.text(), "日語🎉x");
        assert_eq!(d.get_text_range((0, 1), (0, 3)), "語🎉");
        assert_eq!(line_cells("日a\tb", 4), vec![2, 1, 1, 1]);
        assert_eq!(line_cells("\tx", 4), vec![4, 1]);
        assert_eq!(cell_to_column(&[2, 1, 1], 1), 0);
        assert_eq!(cell_to_column(&[2, 1, 1], 2), 1);
        assert_eq!(cell_to_column(&[2, 1, 1], 9), 3);
    }

    #[test]
    fn index_location_roundtrip() {
        let d = Document::new("ab\ncde\n\nf");
        for i in 0..=d.text().chars().count() {
            let loc = d.get_location_from_index(i).unwrap();
            assert_eq!(d.get_index_from_location(loc), i, "{i} {loc:?}");
        }
        assert_eq!(d.get_location_from_index(100), None);
    }

    #[test]
    fn selection_helpers() {
        let s = Selection::new((2, 1), (0, 3));
        assert_eq!(s.sorted(), ((0, 3), (2, 1)));
        assert!(s.contains_line(1));
        assert!(!s.contains_line(3));
        assert!(Selection::cursor((1, 1)).is_empty());
    }
}

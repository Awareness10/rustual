//! Soft wrapping (port of `textual/document/_wrapped_document.py` and `textual/_wrap.py`).
//!
//! A [`WrappedDocument`] caches, for every document line, the character offsets where the line
//! is broken when wrapped at a width, plus the line's cell width. It maps between document
//! [`Location`]s and visual `(x, y)` offsets. It is updated incrementally after edits
//! ([`WrappedDocument::wrap_range`]).

use super::document::{cell_to_column, line_cells, Document, Location};

/// Wrap offsets (character indices where new visual lines start) for a line with per-character
/// widths `cells`, wrapping at `width` cells. Words are kept together where possible (a
/// "chunk" is a word plus its trailing whitespace); longer words are folded.
pub fn compute_wrap_offsets(chars: &[char], cells: &[i32], width: i32) -> Vec<usize> {
    let mut breaks: Vec<usize> = Vec::new();
    if width <= 0 {
        return breaks;
    }
    let push = |breaks: &mut Vec<usize>, at: usize| {
        if at > 0 && breaks.last() != Some(&at) {
            breaks.push(at);
        }
    };
    let mut cell_offset = 0;
    let n = chars.len();
    let mut start = 0;
    while start < n {
        // Chunk: `\S+\s*|\s+`.
        let mut end = start;
        if !chars[start].is_whitespace() {
            while end < n && !chars[end].is_whitespace() {
                end += 1;
            }
        }
        while end < n && chars[end].is_whitespace() {
            end += 1;
        }
        let chunk_width: i32 = cells[start..end].iter().sum();
        if width - cell_offset >= chunk_width {
            cell_offset += chunk_width;
        } else if chunk_width > width {
            // Fold the chunk over several lines.
            let mut line_start = start;
            let mut total = 0;
            push(&mut breaks, start);
            for (i, &w) in cells.iter().enumerate().take(end).skip(start) {
                if total + w > width && i > line_start {
                    push(&mut breaks, i);
                    line_start = i;
                    total = w;
                } else {
                    total += w;
                }
            }
            cell_offset = total;
        } else if cell_offset > 0 && start > 0 {
            push(&mut breaks, start);
            cell_offset = chunk_width;
        } else {
            cell_offset += chunk_width;
        }
        start = end;
    }
    breaks
}

/// A wrapped view of a [`Document`].
#[derive(Debug, Clone, Default)]
pub struct WrappedDocument {
    /// Width the document is wrapped at (0 = no wrapping).
    width: i32,
    tab_width: usize,
    /// Per line: character offsets where wrapped sections start (excluding 0).
    wrap_offsets: Vec<Vec<usize>>,
    /// Per line: total cell width.
    line_widths: Vec<i32>,
    /// Per line: the y offset of the line's first section; one extra entry holds the height.
    line_y: Vec<usize>,
    /// Whether the cache reflects the document (false until the first `wrap`).
    valid: bool,
}

impl WrappedDocument {
    pub fn new(document: &Document, width: i32, tab_width: usize) -> Self {
        let mut w = WrappedDocument::default();
        w.wrap(document, width, tab_width);
        w
    }

    pub fn width(&self) -> i32 {
        self.width
    }

    pub fn tab_width(&self) -> usize {
        self.tab_width
    }

    /// Mark the cache stale (the next `ensure` re-wraps everything).
    pub fn invalidate(&mut self) {
        self.valid = false;
    }

    /// Re-wrap if the width or tab width changed (or the cache is stale).
    pub fn ensure(&mut self, document: &Document, width: i32, tab_width: usize) {
        if !self.valid
            || self.width != width
            || self.tab_width != tab_width
            || self.line_widths.len() != document.line_count()
        {
            self.wrap(document, width, tab_width);
        }
    }

    fn wrap_line(&self, line: &str) -> (Vec<usize>, i32) {
        let cells = line_cells(line, self.tab_width);
        let total = cells.iter().sum();
        if self.width <= 0 || total <= self.width {
            return (Vec::new(), total);
        }
        let chars: Vec<char> = line.chars().collect();
        (compute_wrap_offsets(&chars, &cells, self.width), total)
    }

    /// Wrap every line at `width` (0 = no wrapping).
    pub fn wrap(&mut self, document: &Document, width: i32, tab_width: usize) {
        self.width = width.max(0);
        self.tab_width = tab_width.max(1);
        let n = document.line_count();
        self.wrap_offsets = Vec::with_capacity(n);
        self.line_widths = Vec::with_capacity(n);
        for line in document.lines() {
            let (offsets, w) = self.wrap_line(line);
            self.wrap_offsets.push(offsets);
            self.line_widths.push(w);
        }
        self.rebuild_y();
        self.valid = true;
    }

    fn rebuild_y(&mut self) {
        self.line_y.clear();
        self.line_y.reserve(self.wrap_offsets.len() + 1);
        let mut y = 0;
        for offsets in &self.wrap_offsets {
            self.line_y.push(y);
            y += offsets.len() + 1;
        }
        self.line_y.push(y);
    }

    /// Incrementally re-wrap after an edit replaced document rows `top..=old_bottom` with
    /// rows `top..=new_bottom` (must be called after the document changed).
    pub fn wrap_range(
        &mut self,
        document: &Document,
        top: usize,
        old_bottom: usize,
        new_bottom: usize,
    ) {
        if !self.valid || self.wrap_offsets.is_empty() {
            self.wrap(document, self.width, self.tab_width.max(1));
            return;
        }
        let old_max = self.wrap_offsets.len() - 1;
        let new_max = document.line_count() - 1;
        let top = top.min(old_max).min(new_max);
        let old_bottom = old_bottom.clamp(top, old_max);
        let new_bottom = new_bottom.clamp(top, new_max);
        // Sanity check: the line count must change by exactly the edited range delta.
        if old_max as isize - old_bottom as isize != new_max as isize - new_bottom as isize {
            self.wrap(document, self.width, self.tab_width);
            return;
        }
        let mut offsets = Vec::with_capacity(new_bottom - top + 1);
        let mut widths = Vec::with_capacity(new_bottom - top + 1);
        for row in top..=new_bottom {
            let (o, w) = self.wrap_line(document.get_line(row));
            offsets.push(o);
            widths.push(w);
        }
        self.wrap_offsets.splice(top..=old_bottom, offsets);
        self.line_widths.splice(top..=old_bottom, widths);
        self.rebuild_y();
    }

    /// Number of visual lines.
    pub fn height(&self) -> usize {
        self.line_y.last().copied().unwrap_or(0)
    }

    /// Maximum line width in cells.
    pub fn max_width(&self) -> i32 {
        self.line_widths.iter().copied().max().unwrap_or(0)
    }

    /// Wrap offsets of a line (empty if not wrapped / out of range).
    pub fn get_offsets(&self, line: usize) -> &[usize] {
        self.wrap_offsets
            .get(line)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    /// Number of sections (visual lines) of a document line.
    pub fn section_count(&self, line: usize) -> usize {
        self.get_offsets(line).len() + 1
    }

    /// The y offset of a line's first section.
    pub fn line_y(&self, line: usize) -> usize {
        self.line_y
            .get(line)
            .copied()
            .unwrap_or_else(|| self.height())
    }

    /// `(line index, section index)` at visual line `y`.
    pub fn line_info(&self, y: usize) -> Option<(usize, usize)> {
        if y >= self.height() {
            return None;
        }
        // Last line whose first y <= y.
        let line = self.line_y.partition_point(|&ly| ly <= y) - 1;
        Some((line, y - self.line_y[line]))
    }

    /// Character range `(start, end)` of section `section` of `line` (`line_len` = line length).
    pub fn section_range(&self, line: usize, section: usize, line_len: usize) -> (usize, usize) {
        let offsets = self.get_offsets(line);
        let start = if section == 0 {
            0
        } else {
            offsets.get(section - 1).copied().unwrap_or(line_len)
        };
        let end = offsets.get(section).copied().unwrap_or(line_len);
        (start, end)
    }

    /// Index of the section containing `column` (a column at a wrap offset belongs to the
    /// following section).
    pub fn section_of(&self, line: usize, column: usize) -> usize {
        self.get_offsets(line).partition_point(|&o| o <= column)
    }

    /// Visual `(x, y)` offset of a document location.
    pub fn location_to_offset(&self, document: &Document, location: Location) -> (i32, usize) {
        let (row, col) = document.clamp(location);
        let section = self.section_of(row, col);
        let line = document.get_line(row);
        let cells = line_cells(line, self.tab_width);
        let start = if section == 0 {
            0
        } else {
            self.get_offsets(row)[section - 1]
        };
        let x: i32 = cells[start.min(cells.len())..col.min(cells.len())]
            .iter()
            .sum();
        (x, self.line_y(row) + section)
    }

    /// The document location at visual offset `(x, y)` (clamped to the document).
    pub fn offset_to_location(&self, document: &Document, x: i32, y: i32) -> Location {
        let x = x.max(0);
        let y = y.max(0) as usize;
        let (line, section) = match self.line_info(y) {
            Some(info) => info,
            None => {
                let last = document.line_count() - 1;
                (last, self.section_count(last) - 1)
            }
        };
        (line, self.target_column(document, line, x, section))
    }

    /// The column within `line` at cell offset `x` of section `section`.
    pub fn target_column(&self, document: &Document, line: usize, x: i32, section: usize) -> usize {
        let text = document.get_line(line);
        let cells = line_cells(text, self.tab_width);
        let sections = self.section_count(line);
        let section = section.min(sections - 1);
        let (start, end) = self.section_range(line, section, cells.len());
        let column = start + cell_to_column(&cells[start..end], x);
        if section + 1 < sections {
            // Not the last section: the cursor can't rest past the section's last character.
            column.min(end.saturating_sub(1)).max(start)
        } else {
            column
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn offsets(s: &str, width: i32) -> Vec<usize> {
        let chars: Vec<char> = s.chars().collect();
        let cells = line_cells(s, 4);
        compute_wrap_offsets(&chars, &cells, width)
    }

    #[test]
    fn wrap_offsets_words_and_folding() {
        assert_eq!(offsets("hello world", 20), Vec::<usize>::new());
        assert_eq!(offsets("hello world", 8), vec![6]);
        assert_eq!(offsets("abcdefghij", 4), vec![4, 8]);
        assert_eq!(offsets("ab cdefghij", 4), vec![3, 7]);
        // Wide characters never straddle the edge.
        assert_eq!(offsets("日本語", 5), vec![2]);
        assert_eq!(offsets("日本語", 1), vec![1, 2]);
    }

    #[test]
    fn wrapped_document_mapping() {
        let doc = Document::new("hello world\nab\nabcdefghij");
        let w = WrappedDocument::new(&doc, 6, 4);
        assert_eq!(w.height(), 2 + 1 + 2);
        assert_eq!(w.line_info(0), Some((0, 0)));
        assert_eq!(w.line_info(1), Some((0, 1)));
        assert_eq!(w.line_info(2), Some((1, 0)));
        assert_eq!(w.line_info(4), Some((2, 1)));
        assert_eq!(w.line_info(5), None);
        assert_eq!(w.location_to_offset(&doc, (0, 7)), (1, 1));
        assert_eq!(w.location_to_offset(&doc, (0, 6)), (0, 1));
        assert_eq!(w.location_to_offset(&doc, (2, 10)), (4, 4));
        assert_eq!(w.offset_to_location(&doc, 1, 1), (0, 7));
        // Clicking past the end of a non-final section stays within it.
        assert_eq!(w.offset_to_location(&doc, 50, 0), (0, 5));
        assert_eq!(w.offset_to_location(&doc, 50, 4), (2, 10));
        assert_eq!(w.offset_to_location(&doc, 0, 99), (2, 6));
    }

    #[test]
    fn incremental_wrap_matches_full() {
        let mut doc = Document::new("one two three\nfour\nfive six seven eight");
        let mut w = WrappedDocument::new(&doc, 7, 4);
        let r = doc.replace_range((0, 4), (1, 2), "X\nYY ZZZZZZ QQ\n");
        w.wrap_range(&doc, 0, 1, r.end_location.0);
        let full = WrappedDocument::new(&doc, 7, 4);
        assert_eq!(w.wrap_offsets, full.wrap_offsets);
        assert_eq!(w.line_y, full.line_y);
        assert_eq!(w.line_widths, full.line_widths);
        let r = doc.replace_range((0, 0), (3, 3), "");
        w.wrap_range(&doc, 0, 3, r.end_location.0);
        let full = WrappedDocument::new(&doc, 7, 4);
        assert_eq!(w.wrap_offsets, full.wrap_offsets);
        assert_eq!(w.line_y, full.line_y);
    }

    #[test]
    fn unwrapped_mapping_with_tabs() {
        let doc = Document::new("\tx\n日本");
        let w = WrappedDocument::new(&doc, 0, 4);
        assert_eq!(w.height(), 2);
        assert_eq!(w.max_width(), 5);
        assert_eq!(w.location_to_offset(&doc, (0, 1)), (4, 0));
        assert_eq!(w.offset_to_location(&doc, 2, 0), (0, 0));
        assert_eq!(w.offset_to_location(&doc, 4, 0), (0, 1));
        assert_eq!(w.offset_to_location(&doc, 3, 1), (1, 1));
    }
}

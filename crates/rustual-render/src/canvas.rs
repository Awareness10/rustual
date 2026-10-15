//! A 2D grid of terminal cells with compositing and diffing.

use crate::cells::{grapheme_width, graphemes};
use crate::color::Color;
use crate::geometry::{Region, Size};
use crate::strip::Strip;
use crate::style::{Attrs, Style};
use std::sync::Arc;

/// One terminal cell. Colors are always opaque once on a canvas.
/// A wide (2-cell) grapheme occupies its cell and the next cell has `symbol == ""`.
#[derive(Debug, Clone, PartialEq)]
pub struct Cell {
    pub symbol: String,
    pub fg: Color,
    pub bg: Color,
    pub attrs: Attrs,
    pub link: Option<Arc<str>>,
}

impl Cell {
    pub fn blank(bg: Color) -> Cell {
        Cell {
            symbol: " ".into(),
            fg: Color::WHITE,
            bg,
            attrs: Attrs::NONE,
            link: None,
        }
    }

    /// True for the second (placeholder) cell of a wide grapheme.
    pub fn is_continuation(&self) -> bool {
        self.symbol.is_empty()
    }
}

/// A horizontal run of changed cells, starting at (x, y).
#[derive(Debug, Clone, PartialEq)]
pub struct CellRun {
    pub x: i32,
    pub y: i32,
    pub cells: Vec<Cell>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Canvas {
    size: Size,
    cells: Vec<Cell>,
}

/// Unchanged cells allowed between two changed cells before a diff run is split.
const DIFF_MERGE_GAP: usize = 2;

impl Canvas {
    /// A canvas of blank cells. A translucent `bg` is made opaque by compositing it over black.
    pub fn new(size: Size, bg: Color) -> Canvas {
        let size = Size::new(size.width.max(0), size.height.max(0));
        let bg = if bg.a < 1.0 {
            Color::BLACK.composite(bg)
        } else {
            bg
        };
        Canvas {
            size,
            cells: vec![Cell::blank(bg); (size.width * size.height) as usize],
        }
    }

    pub fn size(&self) -> Size {
        self.size
    }

    fn index(&self, x: i32, y: i32) -> Option<usize> {
        (x >= 0 && y >= 0 && x < self.size.width && y < self.size.height)
            .then(|| (y * self.size.width + x) as usize)
    }

    pub fn get(&self, x: i32, y: i32) -> Option<&Cell> {
        self.index(x, y).map(|i| &self.cells[i])
    }

    pub fn get_mut(&mut self, x: i32, y: i32) -> Option<&mut Cell> {
        self.index(x, y).map(|i| &mut self.cells[i])
    }

    /// Row `y` as a slice of cells (empty if out of range).
    pub fn row(&self, y: i32) -> &[Cell] {
        if y < 0 || y >= self.size.height {
            return &[];
        }
        let w = self.size.width as usize;
        let start = y as usize * w;
        &self.cells[start..start + w]
    }

    /// Before cells `x0..x1` of row `y` are overwritten, replace the halves of wide
    /// characters that straddle either edge (and would be orphaned) with spaces.
    fn break_wide_edges(&mut self, y: i32, x0: i32, x1: i32) {
        if let Some(i) = self.index(x0, y) {
            if x0 > 0 && self.cells[i].is_continuation() {
                self.cells[i - 1].symbol = " ".into();
            }
        }
        if let Some(i) = self.index(x1, y) {
            if self.cells[i].is_continuation() {
                self.cells[i].symbol = " ".into();
            }
        }
    }

    /// Fill `region ∩ clip ∩ canvas` with spaces; the new bg is `existing_bg.composite(bg)`.
    /// fg is reset to `existing_bg.composite(bg)`'s contrast-neutral default (WHITE), attrs cleared.
    pub fn fill(&mut self, region: Region, clip: Region, bg: Color) {
        let area = region.intersection(&clip).intersection(&self.size.region());
        if area.is_empty() {
            return;
        }
        for y in area.y..area.bottom() {
            self.break_wide_edges(y, area.x, area.right());
            for x in area.x..area.right() {
                let i = self.index(x, y).expect("inside canvas");
                let new_bg = self.cells[i].bg.composite(bg);
                self.cells[i] = Cell::blank(new_bg);
            }
        }
    }

    /// Write a strip at (x, y), clipped to `clip`. Each span's style is `base.combine(span.style)`;
    /// a missing fg means WHITE, a missing bg keeps the cell's existing bg. Semi-transparent
    /// colors composite over the existing cell bg. Reverse attr is kept as an attr (not swapped).
    /// A wide grapheme that would straddle the clip edge is replaced by a space.
    /// Zero-width graphemes (e.g. control characters) are skipped.
    pub fn put_strip(&mut self, x: i32, y: i32, strip: &Strip, clip: Region, base: &Style) {
        let area = clip.intersection(&self.size.region());
        if area.is_empty() || y < area.y || y >= area.bottom() {
            return;
        }
        let mut cx = x;
        'spans: for span in &strip.spans {
            let style = base.combine(&span.style);
            let attrs = style.attrs();
            for g in graphemes(&span.text) {
                let w = grapheme_width(g);
                if w == 0 {
                    continue;
                }
                let (gx0, gx1) = (cx, cx + w);
                cx = gx1;
                if gx0 >= area.right() {
                    break 'spans;
                }
                if gx1 <= area.x {
                    continue;
                }
                let vx0 = gx0.max(area.x);
                let vx1 = gx1.min(area.right());
                let whole = vx0 == gx0 && vx1 == gx1;
                self.break_wide_edges(y, vx0, vx1);
                for px in vx0..vx1 {
                    let symbol = if !whole {
                        " "
                    } else if px == gx0 {
                        g
                    } else {
                        ""
                    };
                    let i = self.index(px, y).expect("inside canvas");
                    let cell = &mut self.cells[i];
                    let bg = match style.bg {
                        None => cell.bg,
                        Some(c) => cell.bg.composite(c),
                    };
                    let fg = match style.fg {
                        None => Color::WHITE,
                        Some(c) if c.a < 1.0 => bg.composite(c),
                        Some(c) => c,
                    };
                    cell.symbol.clear();
                    cell.symbol.push_str(symbol);
                    cell.fg = fg;
                    cell.bg = bg;
                    cell.attrs = attrs;
                    cell.link = style.link.clone();
                }
            }
        }
    }

    pub fn put_str(&mut self, x: i32, y: i32, s: &str, style: &Style, clip: Region) {
        self.put_strip(x, y, &Strip::from_str(s, Style::default()), clip, style)
    }

    /// Blend fg and bg of every cell in region towards `color` by `factor` (used for opacity).
    pub fn blend_region(&mut self, region: Region, color: Color, factor: f32) {
        self.map_region(region, |c| c.blend(color, factor).with_alpha(c.a));
    }

    /// Tint fg and bg of every cell in region (see [`Color::tint`]).
    pub fn tint_region(&mut self, region: Region, tint: Color) {
        self.map_region(region, |c| c.tint(tint));
    }

    fn map_region(&mut self, region: Region, f: impl Fn(Color) -> Color) {
        let area = region.intersection(&self.size.region());
        if area.is_empty() {
            return;
        }
        for y in area.y..area.bottom() {
            for x in area.x..area.right() {
                let i = self.index(x, y).expect("inside canvas");
                let cell = &mut self.cells[i];
                cell.fg = f(cell.fg);
                cell.bg = f(cell.bg);
            }
        }
    }

    /// Runs of cells that differ from `prev`. If sizes differ, every row is one full run.
    /// Runs separated by at most two unchanged cells are merged, and a run never starts on
    /// a wide-character continuation cell (it is extended left to include the wide char).
    pub fn diff(&self, prev: &Canvas) -> Vec<CellRun> {
        if self.size != prev.size {
            return self.full();
        }
        let mut out = Vec::new();
        for y in 0..self.size.height {
            let row = self.row(y);
            let prev_row = prev.row(y);
            let mut runs: Vec<(usize, usize)> = Vec::new();
            for x in 0..row.len() {
                if row[x] == prev_row[x] {
                    continue;
                }
                let mut start = x;
                while start > 0 && row[start].is_continuation() {
                    start -= 1;
                }
                match runs.last_mut() {
                    Some(last) if start <= last.1 + DIFF_MERGE_GAP => last.1 = last.1.max(x + 1),
                    _ => runs.push((start, x + 1)),
                }
            }
            out.extend(runs.into_iter().map(|(s, e)| CellRun {
                x: s as i32,
                y,
                cells: row[s..e].to_vec(),
            }));
        }
        out
    }

    /// Every row as a full run (for the first frame).
    pub fn full(&self) -> Vec<CellRun> {
        (0..self.size.height)
            .filter(|_| self.size.width > 0)
            .map(|y| CellRun {
                x: 0,
                y,
                cells: self.row(y).to_vec(),
            })
            .collect()
    }

    /// Plain-text rows (wide-char continuation cells omitted), no trailing-space trimming.
    pub fn to_lines(&self) -> Vec<String> {
        (0..self.size.height)
            .map(|y| self.row(y).iter().map(|c| c.symbol.as_str()).collect())
            .collect()
    }

    /// Rows with trailing whitespace trimmed, joined with `\n`. Handy for snapshot tests.
    pub fn to_text(&self) -> String {
        self.to_lines()
            .iter()
            .map(|l| l.trim_end())
            .collect::<Vec<_>>()
            .join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::Span;

    const BG: Color = Color::rgb(0, 0, 100);

    fn canvas(w: i32, h: i32) -> Canvas {
        Canvas::new(Size::new(w, h), BG)
    }

    fn all(c: &Canvas) -> Region {
        c.size().region()
    }

    #[test]
    fn new_and_get() {
        let c = canvas(3, 2);
        assert_eq!(c.size(), Size::new(3, 2));
        assert_eq!(c.get(2, 1).unwrap().bg, BG);
        assert!(c.get(3, 0).is_none());
        assert!(c.get(-1, 0).is_none());
        assert_eq!(c.to_lines(), vec!["   ", "   "]);
        let t = Canvas::new(Size::new(1, 1), Color::rgba(255, 0, 0, 0.5));
        assert_eq!(t.get(0, 0).unwrap().bg.a, 1.0);
        let e = Canvas::new(Size::new(-1, 3), BG);
        assert_eq!(e.size(), Size::new(0, 3));
        assert!(e.full().is_empty());
    }

    #[test]
    fn get_mut_modifies() {
        let mut c = canvas(2, 1);
        c.get_mut(1, 0).unwrap().symbol = "x".into();
        assert_eq!(c.to_text(), " x");
    }

    #[test]
    fn fill_composites() {
        let mut c = canvas(4, 3);
        c.put_str(0, 0, "abcd", &Style::new().bold(), all(&c));
        c.fill(
            Region::new(1, 0, 2, 2),
            all(&c),
            Color::rgba(255, 255, 255, 0.5),
        );
        assert_eq!(c.to_lines()[0], "a  d");
        let cell = c.get(1, 0).unwrap();
        assert_eq!(cell.attrs, Attrs::NONE);
        assert_eq!(cell.fg, Color::WHITE);
        assert_eq!(cell.bg, BG.composite(Color::rgba(255, 255, 255, 0.5)));
        assert_eq!(c.get(0, 0).unwrap().bg, BG);
        // clip limits the fill
        c.fill(all(&c), Region::new(0, 2, 1, 1), Color::rgb(1, 2, 3));
        assert_eq!(c.get(0, 2).unwrap().bg, Color::rgb(1, 2, 3));
        assert_eq!(c.get(1, 2).unwrap().bg, BG);
        // out of bounds is harmless
        c.fill(
            Region::new(-5, -5, 100, 100),
            Region::new(-5, -5, 100, 100),
            BG,
        );
        assert_eq!(c.get(1, 1).unwrap().bg, BG);
    }

    #[test]
    fn put_strip_styles() {
        let mut c = canvas(6, 1);
        let red = Color::rgb(255, 0, 0);
        let strip = Strip::new(vec![
            Span::new("ab", Style::new().fg(red)),
            Span::new("cd", Style::new().bg(Color::rgb(0, 255, 0)).reverse()),
            Span::new("e", Style::new().link("app.bell")),
        ]);
        c.put_strip(1, 0, &strip, all(&c), &Style::new().bold());
        assert_eq!(c.to_lines()[0], " abcde");
        let a = c.get(1, 0).unwrap();
        assert_eq!(a.fg, red);
        assert_eq!(a.bg, BG);
        assert_eq!(a.attrs, Attrs::BOLD);
        let cc = c.get(3, 0).unwrap();
        assert_eq!(cc.fg, Color::WHITE);
        assert_eq!(cc.bg, Color::rgb(0, 255, 0));
        assert!(cc.attrs.contains(Attrs::REVERSE));
        assert!(cc.attrs.contains(Attrs::BOLD));
        assert_eq!(c.get(5, 0).unwrap().link.as_deref(), Some("app.bell"));
        assert_eq!(c.get(0, 0).unwrap().symbol, " ");
    }

    #[test]
    fn put_strip_alpha() {
        let mut c = canvas(2, 1);
        let style = Style::new()
            .bg(Color::rgba(255, 255, 255, 0.5))
            .fg(Color::rgba(0, 0, 0, 0.5));
        c.put_str(0, 0, "x", &style, all(&c));
        let cell = c.get(0, 0).unwrap();
        let bg = BG.composite(Color::rgba(255, 255, 255, 0.5));
        assert_eq!(cell.bg, bg);
        assert_eq!(cell.fg, bg.composite(Color::rgba(0, 0, 0, 0.5)));
        assert_eq!(cell.fg.a, 1.0);
    }

    #[test]
    fn put_strip_clip() {
        let mut c = canvas(6, 2);
        let clip = Region::new(1, 0, 3, 1);
        c.put_str(0, 0, "abcdef", &Style::default(), clip);
        assert_eq!(c.to_lines()[0], " bcd  ");
        c.put_str(0, 1, "zz", &Style::default(), clip);
        assert_eq!(c.to_lines()[1], "      ");
        c.put_str(-2, 0, "123456789", &Style::default(), all(&c));
        assert_eq!(c.to_lines()[0], "345678");
    }

    #[test]
    fn wide_chars() {
        let mut c = canvas(5, 1);
        c.put_str(0, 0, "中a", &Style::default(), all(&c));
        assert_eq!(c.get(0, 0).unwrap().symbol, "中");
        assert_eq!(c.get(1, 0).unwrap().symbol, "");
        assert_eq!(c.to_lines()[0], "中a  ");
        assert_eq!(c.to_text(), "中a");
    }

    #[test]
    fn wide_char_straddling_clip() {
        let mut c = canvas(5, 1);
        c.put_str(0, 0, "a中b", &Style::default(), Region::new(0, 0, 2, 1));
        assert_eq!(c.to_lines()[0], "a    ");
        let mut c = canvas(5, 1);
        c.put_str(0, 0, "a中b", &Style::default(), Region::new(2, 0, 3, 1));
        assert_eq!(c.to_lines()[0], "   b ");
        // straddling the canvas edge
        let mut c = canvas(3, 1);
        c.put_str(2, 0, "中", &Style::default(), all(&c));
        assert_eq!(c.to_lines()[0], "   ");
        assert_eq!(c.get(2, 0).unwrap().symbol, " ");
    }

    #[test]
    fn overwrite_half_of_wide_char() {
        // overwrite the lead half
        let mut c = canvas(4, 1);
        c.put_str(0, 0, "中文", &Style::default(), all(&c));
        c.put_str(2, 0, "x", &Style::default(), all(&c));
        assert_eq!(c.to_lines()[0], "中x ");
        assert_eq!(c.get(3, 0).unwrap().symbol, " ");
        // overwrite the continuation half
        let mut c = canvas(4, 1);
        c.put_str(0, 0, "中文", &Style::default(), all(&c));
        c.put_str(1, 0, "y", &Style::default(), all(&c));
        assert_eq!(c.to_lines()[0], " y文");
        // wide over wide, offset by one
        let mut c = canvas(4, 1);
        c.put_str(0, 0, "中文", &Style::default(), all(&c));
        c.put_str(1, 0, "字", &Style::default(), all(&c));
        assert_eq!(c.to_lines()[0], " 字 ");
        // fill over half a wide char
        let mut c = canvas(4, 1);
        c.put_str(0, 0, "中文", &Style::default(), all(&c));
        c.fill(Region::new(1, 0, 2, 1), all(&c), BG);
        assert_eq!(c.to_lines()[0], "    ");
        for x in 0..4 {
            assert!(!c.get(x, 0).unwrap().is_continuation());
        }
    }

    #[test]
    fn blend_and_tint() {
        let mut c = canvas(2, 1);
        c.blend_region(Region::new(0, 0, 1, 1), Color::BLACK, 1.0);
        assert_eq!(c.get(0, 0).unwrap().bg, Color::BLACK);
        assert_eq!(c.get(0, 0).unwrap().fg, Color::BLACK);
        assert_eq!(c.get(1, 0).unwrap().bg, BG);
        c.tint_region(all(&c), Color::rgba(255, 0, 0, 1.0));
        assert_eq!(c.get(1, 0).unwrap().bg, Color::rgb(255, 0, 0));
        c.blend_region(Region::new(-3, -3, 1, 1), Color::BLACK, 1.0);
    }

    #[test]
    fn diff_runs() {
        let prev = canvas(10, 2);
        let mut c = prev.clone();
        assert!(c.diff(&prev).is_empty());
        c.put_str(1, 0, "ab", &Style::default(), all(&c));
        c.put_str(8, 0, "z", &Style::default(), all(&c));
        c.put_str(5, 1, "q", &Style::default(), all(&c));
        let runs = c.diff(&prev);
        assert_eq!(runs.len(), 3);
        assert_eq!((runs[0].x, runs[0].y, runs[0].cells.len()), (1, 0, 2));
        assert_eq!((runs[1].x, runs[1].y, runs[1].cells.len()), (8, 0, 1));
        assert_eq!((runs[2].x, runs[2].y), (5, 1));
        assert_eq!(runs[2].cells[0].symbol, "q");
    }

    #[test]
    fn diff_merges_small_gaps() {
        let prev = canvas(10, 1);
        let mut c = prev.clone();
        c.put_str(0, 0, "a", &Style::default(), all(&c));
        c.put_str(3, 0, "b", &Style::default(), all(&c));
        let runs = c.diff(&prev);
        assert_eq!(runs.len(), 1);
        assert_eq!((runs[0].x, runs[0].cells.len()), (0, 4));
    }

    #[test]
    fn diff_never_starts_on_continuation() {
        let mut prev = canvas(6, 1);
        prev.put_str(2, 0, "中", &Style::default(), all(&prev));
        let mut c = prev.clone();
        // change only the continuation cell's colour
        c.get_mut(3, 0).unwrap().bg = Color::rgb(9, 9, 9);
        let runs = c.diff(&prev);
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].x, 2);
        assert_eq!(runs[0].cells[0].symbol, "中");
    }

    #[test]
    fn diff_size_change_is_full() {
        let a = canvas(3, 2);
        let b = canvas(4, 2);
        let runs = b.diff(&a);
        assert_eq!(runs, b.full());
        assert_eq!(runs.len(), 2);
        assert_eq!(runs[1].cells.len(), 4);
        assert_eq!(runs[1].y, 1);
    }

    #[test]
    fn to_text_trims() {
        let mut c = canvas(5, 2);
        c.put_str(0, 1, "hi", &Style::default(), all(&c));
        assert_eq!(c.to_text(), "\nhi");
    }
}

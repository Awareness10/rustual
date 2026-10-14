//! `MarkdownTable`: a GFM table drawn with thin box lines (Textual draws its tables with a
//! grid layout and `keyline: thin`; this widget renders the same look itself).

use super::inline::{InlineStyles, LinkMap, LinkTracker};
use super::parse::{ColumnAlign, Inline};
use crate::context::Ctx;
use crate::events::Event;
use crate::message::Flow;
use crate::widget::{Content, RenderCtx, Widget};
use textual_render::{Justify, Span, Strip, Style, Text, TextOverflow};

const DEFAULT_CSS: &str = "
MarkdownTable {
    width: 1fr;
    height: auto;
    margin-bottom: 1;
    &:light {
        background: white 30%;
    }
}
MarkdownTable > .markdown-table--header {
    text-style: bold;
    color: $primary;
}
MarkdownTable > .markdown-table--lines {
    color: $foreground 20%;
}
";

/// A table block. Columns are sized to their content and expanded to fill the width; when
/// the table is too wide, the widest columns shrink and their cells wrap.
#[derive(Debug, Clone)]
pub struct MarkdownTable {
    aligns: Vec<ColumnAlign>,
    header: Vec<Inline>,
    rows: Vec<Vec<Inline>>,
    links: LinkTracker,
}

fn plain_text(inline: &Inline) -> Text {
    let mut t = Text::new();
    for s in &inline.spans {
        t.push(s.text.clone(), Style::default());
    }
    t
}

impl MarkdownTable {
    pub fn new(aligns: Vec<ColumnAlign>, header: Vec<Inline>, rows: Vec<Vec<Inline>>) -> Self {
        MarkdownTable {
            aligns,
            header,
            rows,
            links: LinkTracker::default(),
        }
    }

    pub fn column_count(&self) -> usize {
        self.rows
            .iter()
            .map(Vec::len)
            .chain([self.header.len(), self.aligns.len()])
            .max()
            .unwrap_or(0)
    }

    pub fn row_count(&self) -> usize {
        self.rows.len()
    }

    /// Plain text of the header cells.
    pub fn headers(&self) -> Vec<String> {
        self.header.iter().map(Inline::plain).collect()
    }

    /// Plain text of a body cell.
    pub fn cell(&self, row: usize, column: usize) -> Option<String> {
        self.rows.get(row)?.get(column).map(Inline::plain)
    }

    fn justify(&self, col: usize) -> Justify {
        match self.aligns.get(col).copied().unwrap_or_default() {
            ColumnAlign::Center => Justify::Center,
            ColumnAlign::Right => Justify::Right,
            ColumnAlign::None | ColumnAlign::Left => Justify::Left,
        }
    }

    /// All rows (header first), padded to the column count.
    fn all_rows(&self) -> Vec<Vec<Option<&Inline>>> {
        let n = self.column_count();
        let mut out = Vec::new();
        fn pad(r: &[Inline], n: usize) -> Vec<Option<&Inline>> {
            (0..n).map(|c| r.get(c)).collect()
        }
        if !self.header.is_empty() {
            out.push(pad(&self.header, n));
        }
        for r in &self.rows {
            out.push(pad(r, n));
        }
        out
    }

    /// Content width of each column for a table `width` cells wide.
    fn column_widths(&self, width: i32) -> Vec<i32> {
        let n = self.column_count();
        if n == 0 {
            return Vec::new();
        }
        let mut widths = vec![1; n];
        for row in self.all_rows() {
            for (c, cell) in row.iter().enumerate() {
                if let Some(cell) = cell {
                    widths[c] = widths[c].max(plain_text(cell).cell_len());
                }
            }
        }
        // Each column has one cell of padding either side, plus n + 1 vertical lines.
        let avail = (width - (n as i32 + 1) - 2 * n as i32).max(n as i32);
        let total: i32 = widths.iter().sum();
        if total < avail {
            let extra = avail - total;
            for (i, w) in widths.iter_mut().enumerate() {
                *w += extra / n as i32 + i32::from((i as i32) < extra % n as i32);
            }
        } else {
            let mut total = total;
            while total > avail {
                let (i, _) = widths
                    .iter()
                    .enumerate()
                    .max_by_key(|(i, w)| (**w, std::cmp::Reverse(*i)))
                    .expect("columns");
                if widths[i] <= 1 {
                    break;
                }
                widths[i] -= 1;
                total -= 1;
            }
        }
        widths
    }

    fn cell_lines(
        cell: Option<&Inline>,
        styles: Option<&InlineStyles>,
        width: i32,
        justify: Justify,
    ) -> Vec<Strip> {
        let Some(cell) = cell else {
            return Vec::new();
        };
        let text = match styles {
            Some(s) => s.text(cell, None),
            None => plain_text(cell),
        };
        if text.is_empty() {
            return Vec::new();
        }
        text.wrap(width, justify, TextOverflow::Fold, false)
    }

    fn row_height(&self, row: &[Option<&Inline>], widths: &[i32]) -> i32 {
        row.iter()
            .zip(widths)
            .enumerate()
            .map(|(c, (cell, w))| Self::cell_lines(*cell, None, *w, self.justify(c)).len() as i32)
            .max()
            .unwrap_or(0)
            .max(1)
    }

    fn height_for(&self, width: i32) -> i32 {
        let widths = self.column_widths(width);
        if widths.is_empty() {
            return 0;
        }
        let rows = self.all_rows();
        let body: i32 = rows.iter().map(|r| self.row_height(r, &widths)).sum();
        body + rows.len() as i32 + 1
    }
}

impl Widget for MarkdownTable {
    fn type_names(&self) -> &'static [&'static str] {
        &["MarkdownTable", "MarkdownBlock", "Static"]
    }

    fn default_css(&self) -> &'static str {
        DEFAULT_CSS
    }

    fn on_event(&mut self, event: &Event, ctx: &mut Ctx) -> Flow {
        self.links.handle_event(event, ctx).unwrap_or_default()
    }

    fn content_height(&self, _ctx: &RenderCtx, width: i32) -> Option<i32> {
        Some(self.height_for(width))
    }

    fn render(&self, ctx: &RenderCtx) -> Content {
        let widths = self.column_widths(ctx.size.width);
        if widths.is_empty() {
            return Content::Empty;
        }
        let line_style = ctx.component_style("markdown-table--lines");
        let header_style = ctx.component_style("markdown-table--header");
        let inline = InlineStyles::new(ctx);
        let hline = |l: &str, m: &str, r: &str| {
            let mut s = String::from(l);
            for (i, w) in widths.iter().enumerate() {
                if i > 0 {
                    s.push_str(m);
                }
                s.push_str(&"─".repeat((*w + 2) as usize));
            }
            s.push_str(r);
            Strip::from_str(&s, line_style.clone())
        };
        let mut lines = vec![hline("┌", "┬", "┐")];
        let mut map = LinkMap::default();
        let rows = self.all_rows();
        let has_header = !self.header.is_empty();
        for (ri, row) in rows.iter().enumerate() {
            let is_header = has_header && ri == 0;
            let cells: Vec<Vec<Strip>> = row
                .iter()
                .enumerate()
                .map(|(c, cell)| {
                    let mut l = Self::cell_lines(*cell, Some(&inline), widths[c], self.justify(c));
                    if is_header {
                        l = l
                            .iter()
                            .map(|s| s.apply_base_style(&header_style))
                            .collect();
                    }
                    l
                })
                .collect();
            let height = cells.iter().map(Vec::len).max().unwrap_or(0).max(1);
            let top = lines.len() as i32;
            for y in 0..height {
                let mut strip = Strip::default();
                let mut x = 0;
                strip.push(Span::new("│", line_style.clone()));
                x += 1;
                for (c, cell) in cells.iter().enumerate() {
                    // Header cells are bottom-aligned (Textual: `content-align: left bottom`).
                    let offset = if is_header { height - cell.len() } else { 0 };
                    strip.push(Span::new(" ", Style::default()));
                    x += 1;
                    let line = y
                        .checked_sub(offset)
                        .and_then(|i| cell.get(i))
                        .cloned()
                        .unwrap_or_default();
                    map.add_lines(std::slice::from_ref(&line), x, top + y as i32);
                    let line = line.set_cell_len(widths[c], &Style::default());
                    strip.spans.extend(line.spans);
                    x += widths[c];
                    strip.push(Span::new(" ", Style::default()));
                    strip.push(Span::new("│", line_style.clone()));
                    x += 2;
                }
                lines.push(strip);
            }
            if ri + 1 < rows.len() {
                lines.push(hline("├", "┼", "┤"));
            }
        }
        lines.push(hline("└", "┴", "┘"));
        map.finish(&lines);
        self.links.set(map);
        Content::Lines(lines)
    }
}

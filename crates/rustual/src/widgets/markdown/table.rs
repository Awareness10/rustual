//! `MarkdownTable`: a GFM table drawn with thin box lines (Textual draws its tables with a
//! grid layout and `keyline: thin`; this widget renders the same look itself).

use super::inline::{InlineStyles, LinkMap, LinkTracker};
use super::parse::{ColumnAlign, Inline};
use crate::context::Ctx;
use crate::events::Event;
use crate::message::Flow;
use crate::widget::{Content, RenderCtx, Widget};
use rustual_render::cells::cell_len;
use rustual_render::{Justify, Span, Strip, Style, Text, TextOverflow};

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

/// A table block. Columns are sized as in Textual's grid: to their content, expanded in
/// proportion to fill the width, and shrunk (down to their longest word) when too wide; cells
/// wrap, and words too long for a column end with "…".
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

    /// Content width of each column for a table `width` cells wide: Textual's grid with
    /// `expand` and `shrink` (`_resolve.resolve`), with exact fractions as in Textual. Columns
    /// start at their content width plus padding; extra space is shared in proportion to those
    /// widths; when too wide, columns shrink in order down to their longest word, then all
    /// proportionally.
    fn column_widths(&self, width: i32) -> Vec<i32> {
        let n = self.column_count();
        if n == 0 {
            return Vec::new();
        }
        // Width with one cell of padding either side, and the minimum (longest word + padding).
        let mut natural = vec![2i128; n];
        let mut minimums = vec![1i128; n];
        for row in self.all_rows() {
            for (c, cell) in row.iter().enumerate() {
                if let Some(cell) = cell {
                    let text = plain_text(cell).plain_text();
                    let widest = text.lines().map(cell_len).max().unwrap_or(0);
                    natural[c] = natural[c].max(widest as i128 + 2);
                    let longest = text.split_whitespace().map(cell_len).max().unwrap_or(0);
                    if longest > 0 {
                        minimums[c] = minimums[c].max(longest as i128 + 2);
                    }
                }
            }
        }
        // n + 1 vertical lines: the outer frame and a one-cell gutter between columns.
        let total = Ratio::int((width - (n as i32 + 1)).max(0) as i128);
        let mut widths: Vec<Ratio> = natural.iter().map(|w| Ratio::int(*w)).collect();
        let used: Ratio = widths.iter().fold(Ratio::int(0), |a, w| a.add(*w));
        let remaining = total.sub(used);
        if remaining.n > 0 {
            widths = widths
                .iter()
                .map(|w| w.add(w.div(used).mul(remaining)))
                .collect();
        }
        let mut used = widths.iter().fold(Ratio::int(0), |a, w| a.add(*w));
        let mut excess = used.sub(total);
        if excess.n > 0 {
            for i in 0..n {
                let w = widths[i];
                let remove = w.div(used).max(Ratio::int(1)).mul(excess);
                let updated = Ratio::int(minimums[i]).max(w.sub(remove));
                widths[i] = updated;
                used = used.sub(w).add(updated);
                excess = used.sub(total);
                if excess.n <= 0 {
                    break;
                }
            }
            used = widths.iter().fold(Ratio::int(0), |a, w| a.add(*w));
            excess = used.sub(total);
            if excess.n > 0 {
                widths = widths
                    .iter()
                    .map(|w| w.sub(w.div(used).mul(excess)))
                    .collect();
            }
        }
        // Cell edges are the floors of the running totals (gutters included).
        let mut acc = Ratio::int(0);
        let mut result = Vec::with_capacity(n);
        for w in widths {
            let start = acc.floor();
            acc = acc.add(w);
            result.push(((acc.floor() - start) as i32 - 2).max(1));
            acc = acc.add(Ratio::int(1));
        }
        result
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
        // Textual's cells have `text-overflow: ellipsis`: a word too long for the column is
        // cut with "…" rather than broken across lines.
        text.wrap(width, justify, TextOverflow::Ellipsis, false)
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

/// An exact fraction (Textual resolves grid sizes with `fractions.Fraction`).
#[derive(Debug, Clone, Copy)]
struct Ratio {
    n: i128,
    d: i128,
}

impl Ratio {
    fn new(n: i128, d: i128) -> Self {
        fn gcd(a: i128, b: i128) -> i128 {
            if b == 0 {
                a.abs().max(1)
            } else {
                gcd(b, a % b)
            }
        }
        let (n, d) = if d < 0 { (-n, -d) } else { (n, d.max(1)) };
        let g = gcd(n, d);
        Ratio { n: n / g, d: d / g }
    }

    fn int(n: i128) -> Self {
        Ratio { n, d: 1 }
    }

    fn add(self, o: Ratio) -> Ratio {
        Ratio::new(self.n * o.d + o.n * self.d, self.d * o.d)
    }

    fn sub(self, o: Ratio) -> Ratio {
        Ratio::new(self.n * o.d - o.n * self.d, self.d * o.d)
    }

    fn mul(self, o: Ratio) -> Ratio {
        Ratio::new(self.n * o.n, self.d * o.d)
    }

    fn div(self, o: Ratio) -> Ratio {
        if o.n == 0 {
            return Ratio::int(0);
        }
        Ratio::new(self.n * o.d, self.d * o.n)
    }

    fn max(self, o: Ratio) -> Ratio {
        if self.n * o.d >= o.n * self.d {
            self
        } else {
            o
        }
    }

    fn floor(self) -> i128 {
        self.n.div_euclid(self.d)
    }
}

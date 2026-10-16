//! `Columns` (port of `rich/columns.py`).

use super::table::Table;
use super::{measure, Align, AnyRenderable, Constrain, IntoRenderable, RenderOptions, Renderable};
use crate::strip::Strip;
use crate::text::{Justify, Text};
use crate::Spacing;

/// Renderables laid out in as many columns as fit the width, like `ls` (`rich.columns.Columns`).
#[derive(Debug, Clone, Default)]
pub struct Columns {
    renderables: Vec<AnyRenderable>,
    padding: Spacing,
    width: Option<i32>,
    expand: bool,
    equal: bool,
    column_first: bool,
    right_to_left: bool,
    align: Option<Justify>,
    title: Option<String>,
}

impl Columns {
    pub fn new<I, R>(renderables: I) -> Self
    where
        I: IntoIterator<Item = R>,
        R: IntoRenderable,
    {
        Columns {
            renderables: renderables
                .into_iter()
                .map(IntoRenderable::into_renderable)
                .collect(),
            padding: Spacing::symmetric(0, 1),
            ..Columns::default()
        }
    }

    pub fn push(&mut self, renderable: impl IntoRenderable) {
        self.renderables.push(renderable.into_renderable());
    }

    /// Space around each cell (default: one cell left and right, collapsed between columns).
    pub fn padding(mut self, padding: Spacing) -> Self {
        self.padding = padding;
        self
    }
    /// Fixed column width.
    pub fn width(mut self, width: i32) -> Self {
        self.width = Some(width);
        self
    }
    /// Spread the columns over the full width.
    pub fn expand(mut self, expand: bool) -> Self {
        self.expand = expand;
        self
    }
    /// Make every column as wide as the widest item.
    pub fn equal(mut self, equal: bool) -> Self {
        self.equal = equal;
        self
    }
    /// Fill columns top to bottom instead of rows left to right.
    pub fn column_first(mut self, column_first: bool) -> Self {
        self.column_first = column_first;
        self
    }
    pub fn right_to_left(mut self, right_to_left: bool) -> Self {
        self.right_to_left = right_to_left;
        self
    }
    /// Align items within their column.
    pub fn align(mut self, align: Justify) -> Self {
        self.align = Some(align);
        self
    }
    /// Title above the columns (markup).
    pub fn title(mut self, title: &str) -> Self {
        self.title = Some(title.to_string());
        self
    }

    /// Items in display order for `column_count` columns (with `None` padding the last row).
    fn order(&self, widths: &[i32], column_count: usize) -> Vec<(i32, Option<usize>)> {
        let item_count = self.renderables.len();
        let mut out: Vec<(i32, Option<usize>)> = Vec::with_capacity(item_count);
        if self.column_first {
            let mut lengths = vec![item_count / column_count; column_count];
            for length in lengths.iter_mut().take(item_count % column_count) {
                *length += 1;
            }
            let row_count = item_count.div_ceil(column_count);
            let mut cells = vec![vec![None; column_count]; row_count];
            let (mut row, mut col) = (0, 0);
            for index in 0..item_count {
                cells[row][col] = Some(index);
                lengths[col] -= 1;
                if lengths[col] > 0 {
                    row += 1;
                } else {
                    col += 1;
                    row = 0;
                }
            }
            for index in cells.into_iter().flatten() {
                let Some(index) = index else { break };
                out.push((widths[index], Some(index)));
            }
        } else {
            out.extend((0..item_count).map(|i| (widths[i], Some(i))));
        }
        if item_count % column_count != 0 {
            for _ in 0..column_count - item_count % column_count {
                out.push((0, None));
            }
        }
        out
    }

    fn table(&self, options: &RenderOptions) -> Option<Table> {
        if self.renderables.is_empty() {
            return None;
        }
        let width_padding = self.padding.left.max(self.padding.right);
        let max_width = options.max_width;
        let mut widths: Vec<i32> = self
            .renderables
            .iter()
            .map(|r| measure(r.as_ref(), options).maximum)
            .collect();
        if self.equal {
            let widest = widths.iter().copied().max().unwrap_or(0);
            widths = vec![widest; widths.len()];
        }
        let mut table = Table::grid()
            .padding(self.padding)
            .collapse_padding(true)
            .pad_edge(false)
            .expand(self.expand);
        if let Some(title) = &self.title {
            table = table.title(title);
        }
        let mut column_count = self.renderables.len();
        if let Some(width) = self.width {
            column_count = (max_width / (width + width_padding)).max(0) as usize;
            for _ in 0..column_count {
                table.add_column_with("", |c| c.width(width));
            }
        } else {
            while column_count > 1 {
                let mut column_widths: Vec<i32> = Vec::new();
                let mut column_no = 0;
                let mut fits = true;
                for (width, _) in self.order(&widths, column_count) {
                    if column_no == column_widths.len() {
                        column_widths.push(0);
                    }
                    column_widths[column_no] = column_widths[column_no].max(width);
                    let total = column_widths.iter().sum::<i32>()
                        + width_padding * (column_widths.len() as i32 - 1);
                    if total > max_width {
                        column_count = column_widths.len() - 1;
                        fits = false;
                        break;
                    }
                    column_no = (column_no + 1) % column_count;
                }
                if fits {
                    break;
                }
            }
        }
        if column_count == 0 {
            return Some(table);
        }
        let items: Vec<Option<AnyRenderable>> = self
            .order(&widths, column_count)
            .into_iter()
            .map(|(_, index)| {
                let mut r = self.renderables[index?].clone();
                if self.equal {
                    r = Constrain {
                        renderable: r,
                        width: widths[0],
                    }
                    .into_renderable();
                }
                if let Some(align) = self.align {
                    r = Align {
                        renderable: r,
                        justify: align,
                    }
                    .into_renderable();
                }
                Some(r)
            })
            .collect();
        for row in items.chunks(column_count) {
            let mut row: Vec<AnyRenderable> = row
                .iter()
                .map(|r| r.clone().unwrap_or_else(|| Text::new().into_renderable()))
                .collect();
            if self.right_to_left {
                row.reverse();
            }
            table.add_row(row);
        }
        Some(table)
    }
}

/// Like `rich`, `Columns` measures as "any width" (it fills what it is given).
impl Renderable for Columns {
    fn render(&self, options: &RenderOptions) -> Vec<Strip> {
        match self.table(options) {
            Some(table) => table.render(options),
            None => Vec::new(),
        }
    }
}

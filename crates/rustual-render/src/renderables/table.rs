//! `Table` (port of `rich/table.py`).

use super::boxes::{BoxChars, RowLevel, HEAVY_HEAD};
use super::ratio::{ratio_distribute, ratio_reduce};
use super::{
    align_vertical, measure, render_lines_styled, set_shape, AnyRenderable, IntoRenderable,
    Measurement, Padding, RenderOptions, Renderable,
};
use crate::strip::Strip;
use crate::style::Style;
use crate::text::{Justify, Span, Text, TextOverflow};
use crate::Spacing;

/// Vertical alignment of cells shorter than their row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VerticalAlign {
    #[default]
    Top,
    Middle,
    Bottom,
}

/// A table column (`rich.table.Column`). Configure it with [`Table::add_column_with`].
#[derive(Debug, Clone)]
pub struct Column {
    header: AnyRenderable,
    footer: AnyRenderable,
    header_style: Style,
    footer_style: Style,
    style: Style,
    justify: Justify,
    vertical: VerticalAlign,
    overflow: TextOverflow,
    width: Option<i32>,
    min_width: Option<i32>,
    max_width: Option<i32>,
    ratio: Option<i32>,
    no_wrap: bool,
    cells: Vec<AnyRenderable>,
}

impl Column {
    pub fn new(header: impl IntoRenderable) -> Self {
        Column {
            header: header.into_renderable(),
            footer: "".into_renderable(),
            header_style: Style::default(),
            footer_style: Style::default(),
            style: Style::default(),
            justify: Justify::Left,
            vertical: VerticalAlign::Top,
            overflow: TextOverflow::Ellipsis,
            width: None,
            min_width: None,
            max_width: None,
            ratio: None,
            no_wrap: false,
            cells: Vec::new(),
        }
    }

    pub fn footer(mut self, footer: impl IntoRenderable) -> Self {
        self.footer = footer.into_renderable();
        self
    }
    pub fn header_style(mut self, style: Style) -> Self {
        self.header_style = style;
        self
    }
    pub fn footer_style(mut self, style: Style) -> Self {
        self.footer_style = style;
        self
    }
    /// Style of the column's cells.
    pub fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }
    pub fn justify(mut self, justify: Justify) -> Self {
        self.justify = justify;
        self
    }
    pub fn vertical(mut self, vertical: VerticalAlign) -> Self {
        self.vertical = vertical;
        self
    }
    /// Overflow of text that doesn't fit (default: ellipsis).
    pub fn overflow(mut self, overflow: TextOverflow) -> Self {
        self.overflow = overflow;
        self
    }
    /// Fixed content width.
    pub fn width(mut self, width: i32) -> Self {
        self.width = Some(width);
        self
    }
    pub fn min_width(mut self, width: i32) -> Self {
        self.min_width = Some(width);
        self
    }
    pub fn max_width(mut self, width: i32) -> Self {
        self.max_width = Some(width);
        self
    }
    /// Share of the spare width when the table expands (makes the column flexible).
    pub fn ratio(mut self, ratio: i32) -> Self {
        self.ratio = Some(ratio);
        self
    }
    /// Never wrap this column's text.
    pub fn no_wrap(mut self, no_wrap: bool) -> Self {
        self.no_wrap = no_wrap;
        self
    }

    fn flexible(&self) -> bool {
        self.ratio.is_some()
    }
}

#[derive(Debug, Clone, Default)]
struct Row {
    style: Style,
    end_section: bool,
}

/// A table with headers, rows, an optional footer, title and caption, drawn with box
/// characters (`rich.table.Table`). Columns size themselves to their content, shrink (wrapping
/// cells) when the width is short, and can expand to fill it.
#[derive(Debug, Clone)]
pub struct Table {
    columns: Vec<Column>,
    rows: Vec<Row>,
    title: Option<Text>,
    caption: Option<Text>,
    width: Option<i32>,
    min_width: Option<i32>,
    box_chars: Option<BoxChars>,
    padding: Spacing,
    collapse_padding: bool,
    pad_edge: bool,
    expand: bool,
    show_header: bool,
    show_footer: bool,
    show_edge: bool,
    show_lines: bool,
    leading: i32,
    style: Style,
    row_styles: Vec<Style>,
    header_style: Style,
    footer_style: Style,
    border_style: Style,
    title_style: Style,
    caption_style: Style,
    title_justify: Justify,
    caption_justify: Justify,
}

impl Default for Table {
    fn default() -> Self {
        Self::new()
    }
}

/// A rendered cell before alignment: style and renderable (wrapped in its padding).
struct Cell {
    style: Style,
    renderable: AnyRenderable,
    vertical: VerticalAlign,
}

impl Table {
    /// An empty table with `rich`'s defaults (`HEAVY_HEAD` box, padding `(0, 1)`, bold header).
    pub fn new() -> Self {
        Table {
            columns: Vec::new(),
            rows: Vec::new(),
            title: None,
            caption: None,
            width: None,
            min_width: None,
            box_chars: Some(HEAVY_HEAD),
            padding: Spacing::symmetric(0, 1),
            collapse_padding: false,
            pad_edge: true,
            expand: false,
            show_header: true,
            show_footer: false,
            show_edge: true,
            show_lines: false,
            leading: 0,
            style: Style::default(),
            row_styles: Vec::new(),
            header_style: Style::new().bold(),
            footer_style: Style::new().bold(),
            border_style: Style::default(),
            title_style: Style::new().italic(),
            caption_style: Style::new().italic().dim(),
            title_justify: Justify::Center,
            caption_justify: Justify::Center,
        }
    }

    /// A borderless, headerless table for layout (`Table.grid`).
    pub fn grid() -> Self {
        Table {
            box_chars: None,
            padding: Spacing::ZERO,
            collapse_padding: true,
            pad_edge: false,
            show_header: false,
            show_edge: false,
            ..Table::new()
        }
    }

    /// Title above the table (markup), in `title_style` (italic by default).
    pub fn title(mut self, title: &str) -> Self {
        self.title = Some(Text::from_markup(title));
        self
    }
    /// Caption below the table (markup), in `caption_style` (dim italic by default).
    pub fn caption(mut self, caption: &str) -> Self {
        self.caption = Some(Text::from_markup(caption));
        self
    }
    /// Fixed total width.
    pub fn width(mut self, width: i32) -> Self {
        self.width = Some(width);
        self
    }
    pub fn min_width(mut self, width: i32) -> Self {
        self.min_width = Some(width);
        self
    }
    /// Box characters (see [`super::box_styles`]), or `None` for no lines.
    pub fn box_style(mut self, box_chars: Option<BoxChars>) -> Self {
        self.box_chars = box_chars;
        self
    }
    /// Cell padding (default: one cell left and right).
    pub fn padding(mut self, padding: Spacing) -> Self {
        self.padding = padding;
        self
    }
    pub fn collapse_padding(mut self, collapse: bool) -> Self {
        self.collapse_padding = collapse;
        self
    }
    /// Pad the outer edges of edge cells (default `true`).
    pub fn pad_edge(mut self, pad_edge: bool) -> Self {
        self.pad_edge = pad_edge;
        self
    }
    /// Fill the available width.
    pub fn expand(mut self, expand: bool) -> Self {
        self.expand = expand;
        self
    }
    pub fn show_header(mut self, show: bool) -> Self {
        self.show_header = show;
        self
    }
    pub fn show_footer(mut self, show: bool) -> Self {
        self.show_footer = show;
        self
    }
    /// Draw the outer border (default `true`).
    pub fn show_edge(mut self, show: bool) -> Self {
        self.show_edge = show;
        self
    }
    /// Draw a line between every row.
    pub fn show_lines(mut self, show: bool) -> Self {
        self.show_lines = show;
        self
    }
    /// Blank lines between rows.
    pub fn leading(mut self, leading: i32) -> Self {
        self.leading = leading.max(0);
        self
    }
    /// Base style of the table (beneath cells and borders).
    pub fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }
    /// Styles cycled across rows (e.g. `[Style::new().dim(), Style::new()]` for zebra stripes).
    pub fn row_styles(mut self, styles: Vec<Style>) -> Self {
        self.row_styles = styles;
        self
    }
    pub fn header_style(mut self, style: Style) -> Self {
        self.header_style = style;
        self
    }
    pub fn footer_style(mut self, style: Style) -> Self {
        self.footer_style = style;
        self
    }
    pub fn border_style(mut self, style: Style) -> Self {
        self.border_style = style;
        self
    }
    pub fn title_style(mut self, style: Style) -> Self {
        self.title_style = style;
        self
    }
    pub fn caption_style(mut self, style: Style) -> Self {
        self.caption_style = style;
        self
    }
    pub fn title_justify(mut self, justify: Justify) -> Self {
        self.title_justify = justify;
        self
    }
    pub fn caption_justify(mut self, justify: Justify) -> Self {
        self.caption_justify = justify;
        self
    }

    /// Add a column with a header (markup or any renderable).
    pub fn add_column(&mut self, header: impl IntoRenderable) -> &mut Self {
        self.add_column_with(header, |c| c)
    }

    /// Add a column, configured by `f` (e.g. `|c| c.justify(Justify::Right).no_wrap(true)`).
    pub fn add_column_with(
        &mut self,
        header: impl IntoRenderable,
        f: impl FnOnce(Column) -> Column,
    ) -> &mut Self {
        let mut column = f(Column::new(header));
        column.cells = vec!["".into_renderable(); self.rows.len()];
        self.columns.push(column);
        self
    }

    /// Add a row of cells (markup strings or renderables). Missing cells are blank; extra
    /// cells add columns.
    pub fn add_row<I, R>(&mut self, cells: I) -> &mut Self
    where
        I: IntoIterator<Item = R>,
        R: IntoRenderable,
    {
        self.add_row_styled(cells, Style::default(), false)
    }

    /// [`Table::add_row`] with a row style, optionally ending a section (a line below it).
    pub fn add_row_styled<I, R>(&mut self, cells: I, style: Style, end_section: bool) -> &mut Self
    where
        I: IntoIterator<Item = R>,
        R: IntoRenderable,
    {
        let mut cells: Vec<AnyRenderable> = cells
            .into_iter()
            .map(IntoRenderable::into_renderable)
            .collect();
        while cells.len() < self.columns.len() {
            cells.push("".into_renderable());
        }
        for (index, cell) in cells.into_iter().enumerate() {
            if index == self.columns.len() {
                let mut column = Column::new("");
                column.cells = vec!["".into_renderable(); self.rows.len()];
                self.columns.push(column);
            }
            self.columns[index].cells.push(cell);
        }
        self.rows.push(Row { style, end_section });
        self
    }

    /// Draw a line after the last row added so far.
    pub fn add_section(&mut self) -> &mut Self {
        if let Some(row) = self.rows.last_mut() {
            row.end_section = true;
        }
        self
    }

    pub fn row_count(&self) -> usize {
        self.rows.len()
    }

    pub fn column_count(&self) -> usize {
        self.columns.len()
    }

    fn extra_width(&self) -> i32 {
        let mut width = 0;
        if self.box_chars.is_some() && self.show_edge {
            width += 2;
        }
        if self.box_chars.is_some() {
            width += self.columns.len() as i32 - 1;
        }
        width
    }

    fn padding_width(&self, index: usize) -> i32 {
        let Spacing {
            right: mut pad_right,
            left: mut pad_left,
            ..
        } = self.padding;
        if self.collapse_padding {
            pad_left = 0;
            pad_right = (pad_left - pad_right).abs();
        }
        if !self.pad_edge {
            if index == 0 {
                pad_left = 0;
            }
            if index == self.columns.len() - 1 {
                pad_right = 0;
            }
        }
        pad_left + pad_right
    }

    /// All cells of a column (header, body, footer), each wrapped in its padding.
    fn cells(&self, index: usize) -> Vec<Cell> {
        let column = &self.columns[index];
        let first_column = index == 0;
        let last_column = index == self.columns.len() - 1;
        let padding_for = |first_row: bool, last_row: bool| {
            let Spacing {
                mut top,
                mut right,
                mut bottom,
                mut left,
            } = self.padding;
            if self.collapse_padding {
                if !first_column {
                    left = (left - right).max(0);
                }
                if !last_row {
                    bottom = (top - bottom).max(0);
                }
            }
            if !self.pad_edge {
                if first_column {
                    left = 0;
                }
                if last_column {
                    right = 0;
                }
                if first_row {
                    top = 0;
                }
                if last_row {
                    bottom = 0;
                }
            }
            Spacing::new(top, right, bottom, left)
        };
        let mut raw: Vec<(Style, AnyRenderable)> = Vec::new();
        if self.show_header {
            raw.push((
                self.header_style.combine(&column.header_style),
                column.header.clone(),
            ));
        }
        for cell in &column.cells {
            raw.push((column.style.clone(), cell.clone()));
        }
        if self.show_footer {
            raw.push((
                self.footer_style.combine(&column.footer_style),
                column.footer.clone(),
            ));
        }
        let any_padding = self.padding != Spacing::ZERO;
        let n = raw.len();
        raw.into_iter()
            .enumerate()
            .map(|(i, (style, renderable))| Cell {
                style,
                renderable: if any_padding {
                    Padding::new(renderable, padding_for(i == 0, i + 1 == n)).into_renderable()
                } else {
                    renderable
                },
                vertical: column.vertical,
            })
            .collect()
    }

    /// Minimum and maximum width of a column, including padding (`_measure_column`).
    fn measure_column(&self, options: &RenderOptions, index: usize) -> Measurement {
        let max_width = options.max_width;
        if max_width < 1 {
            return Measurement::new(0, 0);
        }
        let column = &self.columns[index];
        let padding_width = self.padding_width(index);
        if let Some(width) = column.width {
            return Measurement::new(width + padding_width, width + padding_width)
                .with_maximum(max_width);
        }
        let ms: Vec<Measurement> = self
            .cells(index)
            .iter()
            .map(|c| measure(c.renderable.as_ref(), options))
            .collect();
        Measurement::new(
            ms.iter().map(|m| m.minimum).max().unwrap_or(1),
            ms.iter().map(|m| m.maximum).max().unwrap_or(max_width),
        )
        .with_maximum(max_width)
        .clamp(
            column.min_width.map(|w| w + padding_width),
            column.max_width.map(|w| w + padding_width),
        )
    }

    /// Width of each column including padding, excluding borders (`_calculate_column_widths`).
    fn column_widths(&self, options: &RenderOptions) -> Vec<i32> {
        let max_width = options.max_width;
        let ranges: Vec<Measurement> = (0..self.columns.len())
            .map(|i| self.measure_column(options, i))
            .collect();
        let mut widths: Vec<i32> = ranges
            .iter()
            .map(|r| if r.maximum == 0 { 1 } else { r.maximum })
            .collect();
        if self.expand {
            let ratios: Vec<i32> = self
                .columns
                .iter()
                .filter(|c| c.flexible())
                .map(|c| c.ratio.unwrap_or(0))
                .collect();
            if ratios.iter().any(|r| *r != 0) {
                let fixed: Vec<i32> = ranges
                    .iter()
                    .zip(&self.columns)
                    .map(|(r, c)| if c.flexible() { 0 } else { r.maximum })
                    .collect();
                let flex_minimum: Vec<i32> = self
                    .columns
                    .iter()
                    .enumerate()
                    .filter(|(_, c)| c.flexible())
                    .map(|(i, c)| c.width.unwrap_or(1) + self.padding_width(i))
                    .collect();
                let flexible_width = max_width - fixed.iter().sum::<i32>();
                let mut flex =
                    ratio_distribute(flexible_width, &ratios, Some(&flex_minimum)).into_iter();
                for (i, column) in self.columns.iter().enumerate() {
                    if column.flexible() {
                        widths[i] = fixed[i] + flex.next().unwrap_or(0);
                    }
                }
            }
        }
        let mut table_width: i32 = widths.iter().sum();
        if table_width > max_width {
            let wrapable: Vec<bool> = self
                .columns
                .iter()
                .map(|c| c.width.is_none() && !c.no_wrap)
                .collect();
            widths = collapse_widths(&widths, &wrapable, max_width);
            table_width = widths.iter().sum();
            // Last resort: reduce all columns evenly.
            if table_width > max_width {
                let excess = table_width - max_width;
                widths = ratio_reduce(excess, &vec![1; widths.len()], &widths, &widths);
                table_width = widths.iter().sum();
            }
            widths = widths
                .iter()
                .enumerate()
                .map(|(i, w)| self.measure_column(&options.with_width(*w), i).maximum)
                .collect();
        }
        if (table_width < max_width && self.expand)
            || self
                .min_width
                .is_some_and(|m| table_width < m - self.extra_width())
        {
            let target = match self.min_width {
                None => max_width,
                Some(m) => (m - self.extra_width()).min(max_width),
            };
            let pad = ratio_distribute(target - table_width, &widths, None);
            widths = widths.iter().zip(pad).map(|(w, p)| w + p).collect();
        }
        widths
    }

    fn row_style(&self, index: usize) -> Style {
        let mut style = Style::default();
        if !self.row_styles.is_empty() {
            style = style.combine(&self.row_styles[index % self.row_styles.len()]);
        }
        style.combine(&self.rows[index].style)
    }

    fn render_annotation(
        &self,
        text: &Text,
        style: &Style,
        justify: Justify,
        width: i32,
    ) -> Vec<Strip> {
        let mut text = text.clone();
        text.stylize_base(style);
        text.wrap(width, justify, TextOverflow::Fold, false)
    }

    fn render_body(&self, widths: &[i32]) -> Vec<Strip> {
        let table_style = self.style.clone();
        let border_style = table_style.combine(&self.border_style);
        let column_cells: Vec<Vec<Cell>> = (0..self.columns.len()).map(|i| self.cells(i)).collect();
        let row_count = column_cells.first().map_or(0, Vec::len);
        let box_chars = self.box_chars.map(|b| {
            if self.show_header {
                b
            } else {
                b.plain_headed()
            }
        });
        let border = |s: String| Strip::new(vec![Span::new(s, border_style.clone())]);
        let mut out = Vec::new();
        if let (Some(b), true) = (box_chars, self.show_edge) {
            out.push(border(b.get_top(widths)));
        }
        for index in 0..row_count {
            let first = index == 0;
            let last = index + 1 == row_count;
            let header_row = first && self.show_header;
            let footer_row = last && self.show_footer;
            let row = if !header_row && !footer_row {
                Some(&self.rows[index - usize::from(self.show_header)])
            } else {
                None
            };
            let row_style = if row.is_some() {
                self.row_style(index - usize::from(self.show_header))
            } else {
                Style::default()
            };
            let mut cells: Vec<Vec<Strip>> = Vec::with_capacity(widths.len());
            for (width, (column, col_cells)) in
                widths.iter().zip(self.columns.iter().zip(&column_cells))
            {
                let cell = &col_cells[index];
                let render_options = RenderOptions {
                    max_width: *width,
                    height: None,
                    justify: Some(column.justify),
                    overflow: Some(column.overflow),
                    no_wrap: column.no_wrap,
                };
                let style = cell.style.combine(&row_style);
                cells.push(render_lines_styled(
                    cell.renderable.as_ref(),
                    &render_options,
                    &style,
                    true,
                ));
            }
            let row_height = cells.iter().map(Vec::len).max().unwrap_or(0).max(1);
            let cells: Vec<Vec<Strip>> = cells
                .into_iter()
                .zip(widths)
                .enumerate()
                .map(|(c, (lines, width))| {
                    let cell = &column_cells[c][index];
                    let vertical = if header_row {
                        VerticalAlign::Bottom
                    } else if footer_row {
                        VerticalAlign::Top
                    } else {
                        cell.vertical
                    };
                    let style = cell.style.combine(&row_style);
                    let aligned = align_vertical(lines, *width, row_height, vertical, &style);
                    set_shape(aligned, *width, row_height, &Style::default())
                })
                .collect();
            if let Some(b) = box_chars {
                if last && self.show_footer {
                    out.push(border(b.row(widths, RowLevel::Foot, self.show_edge)));
                }
                let (left, right, divider) = if first {
                    b.head_vertical()
                } else if last {
                    b.foot_vertical()
                } else {
                    b.mid_vertical()
                };
                // A whitespace divider also takes the row background.
                let divider_style = if divider.is_whitespace() {
                    Style {
                        bg: row_style.bg,
                        ..Style::default()
                    }
                    .combine(&border_style)
                } else {
                    border_style.clone()
                };
                for line_no in 0..row_height {
                    let mut strip = Strip::default();
                    if self.show_edge {
                        strip.push(Span::new(left.to_string(), border_style.clone()));
                    }
                    for (c, cell) in cells.iter().enumerate() {
                        strip.spans.extend(cell[line_no].spans.iter().cloned());
                        if c + 1 < cells.len() {
                            strip.push(Span::new(divider.to_string(), divider_style.clone()));
                        }
                    }
                    if self.show_edge {
                        strip.push(Span::new(right.to_string(), border_style.clone()));
                    }
                    out.push(strip);
                }
            } else {
                for line_no in 0..row_height {
                    let mut strip = Strip::default();
                    for cell in &cells {
                        strip.spans.extend(cell[line_no].spans.iter().cloned());
                    }
                    out.push(strip);
                }
            }
            if let (Some(b), true) = (box_chars, first && self.show_header) {
                out.push(border(b.row(widths, RowLevel::Head, self.show_edge)));
            }
            let end_section = row.is_some_and(|r| r.end_section);
            if let Some(b) = box_chars {
                if (self.show_lines || self.leading > 0 || end_section)
                    && !last
                    && !(self.show_footer && index + 2 >= row_count)
                    && !header_row
                {
                    if self.leading > 0 {
                        for _ in 0..self.leading {
                            out.push(border(b.row(widths, RowLevel::Mid, self.show_edge)));
                        }
                    } else {
                        out.push(border(b.row(widths, RowLevel::Row, self.show_edge)));
                    }
                }
            }
        }
        if let (Some(b), true) = (box_chars, self.show_edge) {
            out.push(border(b.get_bottom(widths)));
        }
        out
    }
}

/// Shrink the widest wrapable columns until the total fits (`Table._collapse_widths`).
fn collapse_widths(widths: &[i32], wrapable: &[bool], max_width: i32) -> Vec<i32> {
    let mut widths = widths.to_vec();
    let mut total: i32 = widths.iter().sum();
    let mut excess = total - max_width;
    if wrapable.iter().any(|w| *w) {
        while total != 0 && excess > 0 {
            let max_column = widths
                .iter()
                .zip(wrapable)
                .filter(|(_, w)| **w)
                .map(|(w, _)| *w)
                .max()
                .unwrap_or(0);
            let second_max = widths
                .iter()
                .zip(wrapable)
                .map(|(w, a)| if *a && *w != max_column { *w } else { 0 })
                .max()
                .unwrap_or(0);
            let difference = max_column - second_max;
            let ratios: Vec<i32> = widths
                .iter()
                .zip(wrapable)
                .map(|(w, a)| i32::from(*w == max_column && *a))
                .collect();
            if !ratios.iter().any(|r| *r != 0) || difference == 0 {
                break;
            }
            let max_reduce = vec![excess.min(difference); widths.len()];
            widths = ratio_reduce(excess, &ratios, &max_reduce, &widths);
            total = widths.iter().sum();
            excess = total - max_width;
        }
    }
    widths
}

impl Renderable for Table {
    fn render(&self, options: &RenderOptions) -> Vec<Strip> {
        if self.columns.is_empty() {
            return vec![Strip::default()];
        }
        let max_width = self.width.unwrap_or(options.max_width);
        let extra = self.extra_width();
        let widths = self.column_widths(&options.with_width(max_width - extra));
        let table_width = widths.iter().sum::<i32>() + extra;
        let mut out = Vec::new();
        if let Some(title) = &self.title {
            out.extend(self.render_annotation(
                title,
                &self.title_style,
                self.title_justify,
                table_width,
            ));
        }
        out.extend(self.render_body(&widths));
        if let Some(caption) = &self.caption {
            out.extend(self.render_annotation(
                caption,
                &self.caption_style,
                self.caption_justify,
                table_width,
            ));
        }
        out
    }

    fn measure(&self, options: &RenderOptions) -> Measurement {
        let max_width = self.width.unwrap_or(options.max_width);
        if max_width < 0 {
            return Measurement::new(0, 0);
        }
        let extra = self.extra_width();
        let max_width: i32 = self
            .column_widths(&options.with_width(max_width - extra))
            .iter()
            .sum();
        let ms: Vec<Measurement> = (0..self.columns.len())
            .map(|i| self.measure_column(&options.with_width(max_width), i))
            .collect();
        let minimum = ms.iter().map(|m| m.minimum).sum::<i32>() + extra;
        let maximum = match self.width {
            None => ms.iter().map(|m| m.maximum).sum::<i32>() + extra,
            Some(w) => w,
        };
        Measurement::new(minimum, maximum).clamp(self.min_width, None)
    }
}

//! `DataTable` (port of `textual/widgets/_data_table.py`): a self-scrolling table with a fixed
//! header, optional row labels, fixed rows / columns, zebra stripes and a cell / row / column cursor.
//!
//! ```
//! # use rustual::prelude::*;
//! let mut table = DataTable::new().zebra_stripes(true).cursor_type(CursorType::Row);
//! table.add_columns(&["Name", "Age"]);
//! table.add_row(["Ada", "36"]);
//! ```
//!
//! Data methods (`add_row`, `update_cell`, `sort`, ...) don't need a ctx; methods that move the
//! cursor post messages and scroll, so they take the table's [`Ctx`]
//! (`ctx.with::<DataTable, _>("#t", |t, ctx| t.move_cursor(Some(5), None, ctx))`).
//!
//! Rendering is O(visible rows): rows are found by binary search over cumulative row offsets.

use crate::binding::Binding;
use crate::context::Ctx;
use crate::events::{Event, MouseEvent};
use crate::message::Flow;
use crate::widget::{Content, RenderCtx, Widget};
use rustual_css::Overflow;
use rustual_render::cells::cell_len;
pub use rustual_render::Justify;
use rustual_render::{Region, Size, Span, Strip, Style, Text, TextOverflow};
use std::cmp::Ordering;
use std::collections::HashMap;
use std::fmt;

const DEFAULT_CSS: &str = r#"
DataTable {
    width: 1fr;
    height: 1fr;
    overflow-y: auto;
    overflow-x: auto;
}
DataTable {
    background: $surface;
    color: $foreground;
    height: auto;
    max-height: 100%;
    &.datatable--fixed-cursor {
        background: $block-cursor-blurred-background;
    }

    &:focus {
        background-tint: $foreground 5%;
        & > .datatable--cursor {
            background: $block-cursor-background;
            color: $block-cursor-foreground;
            text-style: $block-cursor-text-style;
        }

        & > .datatable--header {
            background-tint: $foreground 5%;
        }

        & > .datatable--fixed-cursor {
            color: $block-cursor-foreground;
            background: $block-cursor-background;
        }
    }

    &:dark {
        & > .datatable--even-row {
            background: $surface-darken-1 40%;
        }
    }

    & > .datatable--header {
        text-style: bold;
        background: $panel;
        color: $foreground;
    }

    & > .datatable--fixed {
        background: $secondary-muted;
        color: $foreground;
    }

    & > .datatable--odd-row {

    }

    & > .datatable--even-row {
        background: $surface-lighten-1 50%;
    }

    & > .datatable--cursor {
        background: $block-cursor-blurred-background;
        color: $block-cursor-blurred-foreground;
        text-style: $block-cursor-blurred-text-style;
    }

    & > .datatable--fixed-cursor {
        background: $block-cursor-blurred-background;
        color: $foreground;
    }

    & > .datatable--header-cursor {
        background: $accent-darken-1;
        color: $foreground;
    }

    & > .datatable--header-hover {
        background: $accent 30%;
    }

    & > .datatable--hover {
        background: $block-hover-background;
    }
}

/* ANSI themes (Textual `:ansi`). */
DataTable {
    &:ansi > .datatable--header {
        background: ansi_bright_blue;
        color: ansi_default;
    }
}
"#;

// ---------------------------------------------------------------------- keys & values

/// Identifies a row regardless of its current position (Textual: `RowKey`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RowKey(pub String);

/// Identifies a column regardless of its current position (Textual: `ColumnKey`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ColumnKey(pub String);

impl RowKey {
    pub fn value(&self) -> &str {
        &self.0
    }
}

impl ColumnKey {
    pub fn value(&self) -> &str {
        &self.0
    }
}

impl From<&str> for RowKey {
    fn from(s: &str) -> Self {
        RowKey(s.to_string())
    }
}

impl From<String> for RowKey {
    fn from(s: String) -> Self {
        RowKey(s)
    }
}

impl From<&RowKey> for RowKey {
    fn from(k: &RowKey) -> Self {
        k.clone()
    }
}

impl From<&str> for ColumnKey {
    fn from(s: &str) -> Self {
        ColumnKey(s.to_string())
    }
}

impl From<String> for ColumnKey {
    fn from(s: String) -> Self {
        ColumnKey(s)
    }
}

impl From<&ColumnKey> for ColumnKey {
    fn from(k: &ColumnKey) -> Self {
        k.clone()
    }
}

/// A (row key, column key) pair.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CellKey {
    pub row_key: RowKey,
    pub column_key: ColumnKey,
}

/// A (row, column) position in the table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, PartialOrd, Ord)]
pub struct Coordinate {
    pub row: usize,
    pub column: usize,
}

impl Coordinate {
    pub fn new(row: usize, column: usize) -> Self {
        Coordinate { row, column }
    }
}

/// What the cursor highlights.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CursorType {
    #[default]
    Cell,
    Row,
    Column,
    None,
}

/// The data of a cell.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum CellValue {
    #[default]
    Empty,
    /// Content markup.
    Markup(String),
    /// Pre-styled text.
    Text(Text),
    Int(i64),
    /// Displayed with two decimals (as Textual's `default_cell_formatter`).
    Float(f64),
    /// A value displayed with an explicit justification (like a Rich `Text(justify=...)`
    /// cell in Textual). Build with [`CellValue::justify`], [`CellValue::right`] or
    /// [`CellValue::center`]. Sorting and [`CellValue::plain`] use the inner value.
    Aligned(Box<CellValue>, Justify),
}

impl CellValue {
    /// This value displayed with `justify` within its cell. Values are left-aligned by
    /// default, numbers included (as in Textual, whose `default_cell_formatter` renders
    /// numbers as plain left-justified text).
    pub fn justify(self, justify: Justify) -> CellValue {
        match self {
            CellValue::Aligned(inner, _) => CellValue::Aligned(inner, justify),
            v => CellValue::Aligned(Box::new(v), justify),
        }
    }

    /// Right-aligned value: `CellValue::right(12.5)`.
    pub fn right(value: impl Into<CellValue>) -> CellValue {
        value.into().justify(Justify::Right)
    }

    /// Centered value.
    pub fn center(value: impl Into<CellValue>) -> CellValue {
        value.into().justify(Justify::Center)
    }

    /// The justification of the value in its cell ([`Justify::Left`] unless
    /// [`CellValue::Aligned`]).
    pub fn get_justify(&self) -> Justify {
        match self {
            CellValue::Aligned(_, j) => *j,
            _ => Justify::Left,
        }
    }

    /// The value without its alignment wrapper.
    pub fn inner(&self) -> &CellValue {
        match self {
            CellValue::Aligned(inner, _) => inner.inner(),
            v => v,
        }
    }

    /// Text as displayed (markup parsed with `markup`).
    fn to_text(&self, markup: &dyn Fn(&str) -> Text) -> Text {
        match self {
            CellValue::Empty => Text::new(),
            CellValue::Markup(m) => markup(m),
            CellValue::Text(t) => t.clone(),
            CellValue::Int(i) => Text::plain(i.to_string()),
            CellValue::Float(f) => Text::plain(format!("{f:.2}")),
            CellValue::Aligned(inner, _) => inner.to_text(markup),
        }
    }

    /// Displayed text without styles.
    pub fn plain(&self) -> String {
        self.to_text(&|s| Text::from_markup(s)).plain_text()
    }

    /// (width, line count) when rendered without wrapping.
    fn measure(&self) -> (i32, usize) {
        let t = self.to_text(&|s| Text::from_markup(s));
        (t.cell_len(), t.lines().len().max(1))
    }

    fn as_number(&self) -> Option<f64> {
        match self {
            CellValue::Int(i) => Some(*i as f64),
            CellValue::Float(f) => Some(*f),
            CellValue::Markup(_) | CellValue::Text(_) => self.plain().trim().parse().ok(),
            CellValue::Empty => None,
            CellValue::Aligned(inner, _) => inner.as_number(),
        }
    }

    /// Ordering used by [`DataTable::sort`]: empty cells first, then numbers (numerically,
    /// including numeric strings), then strings (by their plain text).
    pub fn compare(&self, other: &CellValue) -> Ordering {
        let (this, other) = (self.inner(), other.inner());
        match (this, other) {
            (CellValue::Empty, CellValue::Empty) => return Ordering::Equal,
            (CellValue::Empty, _) => return Ordering::Less,
            (_, CellValue::Empty) => return Ordering::Greater,
            _ => {}
        }
        match (this.as_number(), other.as_number()) {
            (Some(a), Some(b)) => a.partial_cmp(&b).unwrap_or(Ordering::Equal),
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (None, None) => this.plain().cmp(&other.plain()),
        }
    }
}

impl fmt::Display for CellValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.plain())
    }
}

impl From<&str> for CellValue {
    fn from(s: &str) -> Self {
        CellValue::Markup(s.to_string())
    }
}

impl From<String> for CellValue {
    fn from(s: String) -> Self {
        CellValue::Markup(s)
    }
}

impl From<&String> for CellValue {
    fn from(s: &String) -> Self {
        CellValue::Markup(s.clone())
    }
}

impl From<Text> for CellValue {
    fn from(t: Text) -> Self {
        CellValue::Text(t)
    }
}

macro_rules! int_cell {
    ($($t:ty),*) => {$(
        impl From<$t> for CellValue {
            fn from(v: $t) -> Self {
                CellValue::Int(v as i64)
            }
        }
    )*};
}
int_cell!(i8, i16, i32, i64, u8, u16, u32, u64, usize, isize);

impl From<f64> for CellValue {
    fn from(v: f64) -> Self {
        CellValue::Float(v)
    }
}

impl From<f32> for CellValue {
    fn from(v: f32) -> Self {
        CellValue::Float(v as f64)
    }
}

impl<T: Into<CellValue>> From<Option<T>> for CellValue {
    fn from(v: Option<T>) -> Self {
        v.map(Into::into).unwrap_or_default()
    }
}

/// Errors from table lookups and mutations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DataTableError {
    DuplicateKey(String),
    RowDoesNotExist(String),
    ColumnDoesNotExist(String),
    CellDoesNotExist(String),
    /// More cells than columns were given to `add_row`.
    TooManyCells,
}

impl fmt::Display for DataTableError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DataTableError::DuplicateKey(k) => write!(f, "the key {k:?} already exists"),
            DataTableError::RowDoesNotExist(k) => write!(f, "row key {k:?} is not valid"),
            DataTableError::ColumnDoesNotExist(k) => write!(f, "column key {k:?} is not valid"),
            DataTableError::CellDoesNotExist(c) => write!(f, "no cell exists at {c}"),
            DataTableError::TooManyCells => {
                f.write_str("more values provided than there are columns")
            }
        }
    }
}

impl std::error::Error for DataTableError {}

// ---------------------------------------------------------------------- messages

/// Cursor moved to a new cell (cursor type `Cell`).
#[derive(Debug, Clone, PartialEq)]
pub struct CellHighlighted {
    pub value: CellValue,
    pub coordinate: Coordinate,
    pub cell_key: CellKey,
}

/// A cell was selected with enter or a click on the cursor cell (cursor type `Cell`).
#[derive(Debug, Clone, PartialEq)]
pub struct CellSelected {
    pub value: CellValue,
    pub coordinate: Coordinate,
    pub cell_key: CellKey,
}

/// Cursor moved to a new row (cursor type `Row`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RowHighlighted {
    pub cursor_row: usize,
    pub row_key: RowKey,
}

/// A row was selected (cursor type `Row`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RowSelected {
    pub cursor_row: usize,
    pub row_key: RowKey,
}

/// Cursor moved to a new column (cursor type `Column`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColumnHighlighted {
    pub cursor_column: usize,
    pub column_key: ColumnKey,
}

/// A column was selected (cursor type `Column`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColumnSelected {
    pub cursor_column: usize,
    pub column_key: ColumnKey,
}

/// A column header was clicked.
#[derive(Debug, Clone, PartialEq)]
pub struct HeaderSelected {
    pub column_key: ColumnKey,
    pub column_index: usize,
    pub label: Text,
}

/// A row label was clicked.
#[derive(Debug, Clone, PartialEq)]
pub struct RowLabelSelected {
    pub row_key: RowKey,
    pub row_index: usize,
    pub label: Text,
}

// ---------------------------------------------------------------------- table model

/// Column metadata.
#[derive(Debug, Clone)]
pub struct Column {
    pub key: ColumnKey,
    pub label: Text,
    /// Fixed width (when not `auto_width`).
    pub width: i32,
    /// Widest content (label and cells).
    pub content_width: i32,
    pub auto_width: bool,
}

impl Column {
    /// Width including cell padding.
    pub fn render_width(&self, cell_padding: i32) -> i32 {
        2 * cell_padding
            + if self.auto_width {
                self.content_width
            } else {
                self.width
            }
    }
}

#[derive(Debug, Clone)]
struct Row {
    key: RowKey,
    height: i32,
    label: Option<Text>,
    cells: Vec<CellValue>,
}

/// Which row a line belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RowRef {
    Header,
    Data(usize),
}

/// Result of mapping a content position to a cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Hit {
    /// -1 = header.
    row: isize,
    /// -1 = row label.
    column: isize,
    out_of_bounds: bool,
}

/// Styles resolved once per render.
struct Styles {
    header: Style,
    fixed: Style,
    even: Style,
    odd: Style,
    cursor: Style,
    hover: Style,
    header_cursor: Style,
    header_hover: Style,
    fixed_cursor: Style,
    background: rustual_render::Color,
}

/// A table of rows and columns.
#[derive(Debug, Clone)]
pub struct DataTable {
    columns: Vec<Column>,
    column_index: HashMap<ColumnKey, usize>,
    rows: Vec<Row>,
    row_index: HashMap<RowKey, usize>,
    /// Start y (excluding the header) of each row; `row_tops[i+1] - row_tops[i]` = height.
    row_tops: Vec<i32>,
    total_height: i32,
    label_content_width: i32,
    labelled_row_exists: bool,
    next_key: u64,

    show_header: bool,
    show_row_labels: bool,
    fixed_rows: usize,
    fixed_columns: usize,
    zebra_stripes: bool,
    header_height: i32,
    show_cursor: bool,
    cursor_type: CursorType,
    cell_padding: i32,
    cursor_foreground_css: bool,
    cursor_background_css: bool,

    cursor: Coordinate,
    hover: (isize, isize),
    show_hover_cursor: bool,
}

impl Default for DataTable {
    fn default() -> Self {
        Self::new()
    }
}

impl DataTable {
    pub fn new() -> Self {
        DataTable {
            columns: Vec::new(),
            column_index: HashMap::new(),
            rows: Vec::new(),
            row_index: HashMap::new(),
            row_tops: vec![0],
            total_height: 0,
            label_content_width: 0,
            labelled_row_exists: false,
            next_key: 0,
            show_header: true,
            show_row_labels: true,
            fixed_rows: 0,
            fixed_columns: 0,
            zebra_stripes: false,
            header_height: 1,
            show_cursor: true,
            cursor_type: CursorType::Cell,
            cell_padding: 1,
            cursor_foreground_css: true,
            cursor_background_css: false,
            cursor: Coordinate::default(),
            hover: (0, 0),
            show_hover_cursor: false,
        }
    }

    // ------------------------------------------------------------------ builders / options

    pub fn show_header(mut self, v: bool) -> Self {
        self.show_header = v;
        self
    }
    pub fn show_row_labels(mut self, v: bool) -> Self {
        self.show_row_labels = v;
        self
    }
    pub fn fixed_rows(mut self, v: usize) -> Self {
        self.fixed_rows = v;
        self
    }
    pub fn fixed_columns(mut self, v: usize) -> Self {
        self.fixed_columns = v;
        self
    }
    pub fn zebra_stripes(mut self, v: bool) -> Self {
        self.zebra_stripes = v;
        self
    }
    pub fn header_height(mut self, v: i32) -> Self {
        self.header_height = v.max(0);
        self
    }
    pub fn show_cursor(mut self, v: bool) -> Self {
        self.show_cursor = v;
        self
    }
    pub fn cursor_type(mut self, v: CursorType) -> Self {
        self.cursor_type = v;
        self
    }
    pub fn cell_padding(mut self, v: i32) -> Self {
        self.cell_padding = v.max(0);
        self
    }
    /// `true` (default, Textual `"css"`): the cursor's CSS color overrides cell colors.
    pub fn cursor_foreground_priority_css(mut self, v: bool) -> Self {
        self.cursor_foreground_css = v;
        self
    }
    /// `true` (Textual `"css"`): the cursor's CSS background overrides cell backgrounds.
    /// Default `false` (`"renderable"`).
    pub fn cursor_background_priority_css(mut self, v: bool) -> Self {
        self.cursor_background_css = v;
        self
    }

    pub fn set_show_header(&mut self, v: bool) {
        self.show_header = v;
    }
    pub fn set_show_row_labels(&mut self, v: bool) {
        self.show_row_labels = v;
    }
    pub fn set_fixed_rows(&mut self, v: usize) {
        self.fixed_rows = v;
    }
    pub fn set_fixed_columns(&mut self, v: usize) {
        self.fixed_columns = v;
    }
    pub fn set_zebra_stripes(&mut self, v: bool) {
        self.zebra_stripes = v;
    }
    pub fn set_cell_padding(&mut self, v: i32) {
        self.cell_padding = v.max(0);
    }
    pub fn get_zebra_stripes(&self) -> bool {
        self.zebra_stripes
    }
    pub fn get_cursor_type(&self) -> CursorType {
        self.cursor_type
    }
    pub fn get_show_cursor(&self) -> bool {
        self.show_cursor
    }
    pub fn get_show_header(&self) -> bool {
        self.show_header
    }
    pub fn get_show_row_labels(&self) -> bool {
        self.show_row_labels
    }
    pub fn get_fixed_rows(&self) -> usize {
        self.fixed_rows
    }
    pub fn get_fixed_columns(&self) -> usize {
        self.fixed_columns
    }
    pub fn get_header_height(&self) -> i32 {
        self.header_height
    }
    pub fn get_cell_padding(&self) -> i32 {
        self.cell_padding
    }

    /// Change the cursor type, posting the matching `*Highlighted` message.
    pub fn set_cursor_type(&mut self, cursor_type: CursorType, ctx: &mut Ctx) {
        self.cursor_type = cursor_type;
        self.show_hover_cursor = false;
        if self.show_cursor {
            self.highlight_cursor(ctx);
        }
        self.scroll_cursor_into_view(ctx);
    }

    /// Show / hide the cursor (showing it posts the `*Highlighted` message).
    pub fn set_show_cursor(&mut self, show: bool, ctx: &mut Ctx) {
        self.show_cursor = show;
        if show {
            self.highlight_cursor(ctx);
        }
    }

    // ------------------------------------------------------------------ queries

    pub fn row_count(&self) -> usize {
        self.rows.len()
    }

    pub fn column_count(&self) -> usize {
        self.columns.len()
    }

    /// Columns in display order.
    pub fn ordered_columns(&self) -> &[Column] {
        &self.columns
    }

    /// Row keys in display order.
    pub fn row_keys(&self) -> Vec<RowKey> {
        self.rows.iter().map(|r| r.key.clone()).collect()
    }

    pub fn cursor_coordinate(&self) -> Coordinate {
        self.cursor
    }

    pub fn cursor_row(&self) -> usize {
        self.cursor.row
    }

    pub fn cursor_column(&self) -> usize {
        self.cursor.column
    }

    /// Hovered (row, column); -1 means header / row label.
    pub fn hover_coordinate(&self) -> (isize, isize) {
        self.hover
    }

    pub fn is_valid_row_index(&self, row: usize) -> bool {
        row < self.rows.len()
    }

    pub fn is_valid_column_index(&self, column: usize) -> bool {
        column < self.columns.len()
    }

    pub fn is_valid_coordinate(&self, c: Coordinate) -> bool {
        self.is_valid_row_index(c.row) && self.is_valid_column_index(c.column)
    }

    pub fn get_row_index(&self, row_key: impl Into<RowKey>) -> Result<usize, DataTableError> {
        let k = row_key.into();
        self.row_index
            .get(&k)
            .copied()
            .ok_or(DataTableError::RowDoesNotExist(k.0))
    }

    pub fn get_column_index(
        &self,
        column_key: impl Into<ColumnKey>,
    ) -> Result<usize, DataTableError> {
        let k = column_key.into();
        self.column_index
            .get(&k)
            .copied()
            .ok_or(DataTableError::ColumnDoesNotExist(k.0))
    }

    pub fn get_cell(
        &self,
        row_key: impl Into<RowKey>,
        column_key: impl Into<ColumnKey>,
    ) -> Result<&CellValue, DataTableError> {
        let r = self.get_row_index(row_key)?;
        let c = self.get_column_index(column_key)?;
        Ok(&self.rows[r].cells[c])
    }

    pub fn get_cell_at(&self, c: Coordinate) -> Result<&CellValue, DataTableError> {
        self.rows
            .get(c.row)
            .and_then(|r| r.cells.get(c.column))
            .ok_or_else(|| DataTableError::CellDoesNotExist(format!("{c:?}")))
    }

    /// Cells of a row (in column order).
    pub fn get_row(&self, row_key: impl Into<RowKey>) -> Result<&[CellValue], DataTableError> {
        let r = self.get_row_index(row_key)?;
        Ok(&self.rows[r].cells)
    }

    pub fn get_row_at(&self, row: usize) -> Result<&[CellValue], DataTableError> {
        self.rows
            .get(row)
            .map(|r| r.cells.as_slice())
            .ok_or_else(|| DataTableError::RowDoesNotExist(format!("index {row}")))
    }

    /// Cells of a column (in row order).
    pub fn get_column(
        &self,
        column_key: impl Into<ColumnKey>,
    ) -> Result<Vec<&CellValue>, DataTableError> {
        let c = self.get_column_index(column_key)?;
        Ok(self.rows.iter().map(|r| &r.cells[c]).collect())
    }

    pub fn get_column_at(&self, column: usize) -> Result<Vec<&CellValue>, DataTableError> {
        if column >= self.columns.len() {
            return Err(DataTableError::ColumnDoesNotExist(format!(
                "index {column}"
            )));
        }
        Ok(self.rows.iter().map(|r| &r.cells[column]).collect())
    }

    pub fn get_row_height(&self, row_key: impl Into<RowKey>) -> Result<i32, DataTableError> {
        Ok(self.rows[self.get_row_index(row_key)?].height)
    }

    /// The key of the cell at `c`.
    pub fn coordinate_to_cell_key(&self, c: Coordinate) -> Result<CellKey, DataTableError> {
        if !self.is_valid_coordinate(c) {
            return Err(DataTableError::CellDoesNotExist(format!("{c:?}")));
        }
        Ok(CellKey {
            row_key: self.rows[c.row].key.clone(),
            column_key: self.columns[c.column].key.clone(),
        })
    }

    pub fn get_cell_coordinate(
        &self,
        row_key: impl Into<RowKey>,
        column_key: impl Into<ColumnKey>,
    ) -> Result<Coordinate, DataTableError> {
        Ok(Coordinate::new(
            self.get_row_index(row_key)?,
            self.get_column_index(column_key)?,
        ))
    }

    // ------------------------------------------------------------------ mutation

    fn gen_key(&mut self) -> String {
        self.next_key += 1;
        format!("\u{0}auto-{}", self.next_key)
    }

    /// Add an auto-width column with a markup label; returns its (generated) key.
    pub fn add_column(&mut self, label: &str) -> ColumnKey {
        self.add_column_with(label, None, None)
            .expect("generated keys are unique")
    }

    /// Add a column. `width: None` = fit content; `key: None` = generated.
    pub fn add_column_with(
        &mut self,
        label: &str,
        width: Option<i32>,
        key: Option<&str>,
    ) -> Result<ColumnKey, DataTableError> {
        self.add_column_text(Text::from_markup(label), width, key, CellValue::Empty)
    }

    /// Add a column with a styled label and a default value for existing rows.
    pub fn add_column_text(
        &mut self,
        label: Text,
        width: Option<i32>,
        key: Option<&str>,
        default: CellValue,
    ) -> Result<ColumnKey, DataTableError> {
        let key = ColumnKey(match key {
            Some(k) => k.to_string(),
            None => self.gen_key(),
        });
        if self.column_index.contains_key(&key) {
            return Err(DataTableError::DuplicateKey(key.0));
        }
        let label_width = label.cell_len();
        let (dw, dh) = default.measure();
        self.column_index.insert(key.clone(), self.columns.len());
        self.columns.push(Column {
            key: key.clone(),
            label,
            width: width.unwrap_or(label_width).max(0),
            content_width: if self.rows.is_empty() {
                label_width
            } else {
                label_width.max(dw)
            },
            auto_width: width.is_none(),
        });
        // Row heights are kept (as in Textual, which only measures new rows).
        let _ = dh;
        for row in &mut self.rows {
            row.cells.push(default.clone());
        }
        Ok(key)
    }

    /// Add several auto-width columns.
    pub fn add_columns(&mut self, labels: &[&str]) -> Vec<ColumnKey> {
        labels.iter().map(|l| self.add_column(l)).collect()
    }

    /// Add a row (height 1, generated key, no label).
    pub fn add_row<I, C>(&mut self, cells: I) -> RowKey
    where
        I: IntoIterator<Item = C>,
        C: Into<CellValue>,
    {
        self.add_row_with(cells, Some(1), None, None)
            .expect("too many cells for the number of columns")
    }

    /// Add a row. `height: None` = fit the tallest cell; `label` is markup shown at the left.
    pub fn add_row_with<I, C>(
        &mut self,
        cells: I,
        height: Option<i32>,
        key: Option<&str>,
        label: Option<&str>,
    ) -> Result<RowKey, DataTableError>
    where
        I: IntoIterator<Item = C>,
        C: Into<CellValue>,
    {
        let mut cells: Vec<CellValue> = cells.into_iter().map(Into::into).collect();
        if cells.len() > self.columns.len() {
            return Err(DataTableError::TooManyCells);
        }
        let key = RowKey(match key {
            Some(k) => k.to_string(),
            None => self.gen_key(),
        });
        if self.row_index.contains_key(&key) {
            return Err(DataTableError::DuplicateKey(key.0));
        }
        cells.resize(self.columns.len(), CellValue::Empty);
        let label = label.map(Text::from_markup);
        let mut auto_height = 1usize;
        for (col, cell) in self.columns.iter_mut().zip(&cells) {
            let (w, h) = cell.measure();
            col.content_width = col.content_width.max(w);
            auto_height = auto_height.max(h);
        }
        if let Some(l) = &label {
            self.labelled_row_exists = true;
            self.label_content_width = self.label_content_width.max(l.cell_len());
            auto_height = auto_height.max(l.lines().len());
        }
        let height = height.unwrap_or(auto_height as i32).max(1);
        self.row_index.insert(key.clone(), self.rows.len());
        self.rows.push(Row {
            key: key.clone(),
            height,
            label,
            cells,
        });
        self.total_height += height;
        self.row_tops.push(self.total_height);
        self.cursor = self.clamp(self.cursor);
        Ok(key)
    }

    /// Add several rows; returns their keys.
    pub fn add_rows<R, I, C>(&mut self, rows: R) -> Vec<RowKey>
    where
        R: IntoIterator<Item = I>,
        I: IntoIterator<Item = C>,
        C: Into<CellValue>,
    {
        rows.into_iter().map(|r| self.add_row(r)).collect()
    }

    fn rebuild_rows(&mut self) {
        self.row_index = self
            .rows
            .iter()
            .enumerate()
            .map(|(i, r)| (r.key.clone(), i))
            .collect();
        self.row_tops.clear();
        self.row_tops.push(0);
        let mut y = 0;
        for r in &self.rows {
            y += r.height;
            self.row_tops.push(y);
        }
        self.total_height = y;
    }

    fn recompute_column_width(&mut self, column: usize) {
        let col = &self.columns[column];
        let mut w = col.label.cell_len();
        for r in &self.rows {
            w = w.max(r.cells[column].measure().0);
        }
        self.columns[column].content_width = w;
    }

    /// Update a cell; with `update_width` an auto-width column is resized to fit.
    pub fn update_cell(
        &mut self,
        row_key: impl Into<RowKey>,
        column_key: impl Into<ColumnKey>,
        value: impl Into<CellValue>,
        update_width: bool,
    ) -> Result<(), DataTableError> {
        let c = self.get_cell_coordinate(row_key, column_key)?;
        self.update_cell_at(c, value, update_width)
    }

    pub fn update_cell_at(
        &mut self,
        c: Coordinate,
        value: impl Into<CellValue>,
        update_width: bool,
    ) -> Result<(), DataTableError> {
        if !self.is_valid_coordinate(c) {
            return Err(DataTableError::CellDoesNotExist(format!("{c:?}")));
        }
        let value = value.into();
        let (w, _) = value.measure();
        self.rows[c.row].cells[c.column] = value;
        if update_width {
            let col = &mut self.columns[c.column];
            if w < col.content_width {
                self.recompute_column_width(c.column);
            } else {
                col.content_width = w.max(col.label.cell_len());
            }
        }
        Ok(())
    }

    /// Remove a row by key.
    pub fn remove_row(&mut self, row_key: impl Into<RowKey>) -> Result<(), DataTableError> {
        let i = self.get_row_index(row_key)?;
        self.rows.remove(i);
        self.rebuild_rows();
        self.cursor = self.clamp(self.cursor);
        Ok(())
    }

    /// Remove a column by key.
    pub fn remove_column(
        &mut self,
        column_key: impl Into<ColumnKey>,
    ) -> Result<(), DataTableError> {
        let i = self.get_column_index(column_key)?;
        self.columns.remove(i);
        for r in &mut self.rows {
            r.cells.remove(i);
        }
        self.column_index = self
            .columns
            .iter()
            .enumerate()
            .map(|(i, c)| (c.key.clone(), i))
            .collect();
        self.cursor = self.clamp(self.cursor);
        Ok(())
    }

    /// Remove all rows (and columns if `columns`). The cursor returns to (0, 0).
    pub fn clear(&mut self, columns: bool) {
        self.rows.clear();
        self.rebuild_rows();
        if columns {
            self.columns.clear();
            self.column_index.clear();
        } else {
            for c in 0..self.columns.len() {
                self.columns[c].content_width = self.columns[c].label.cell_len();
            }
        }
        self.label_content_width = 0;
        self.labelled_row_exists = false;
        self.cursor = Coordinate::default();
        self.hover = (0, 0);
    }

    /// Sort rows by the values in `columns` (compared in order, see [`CellValue::compare`]);
    /// all columns when empty.
    pub fn sort(&mut self, columns: &[&str], reverse: bool) -> Result<(), DataTableError> {
        let idx: Vec<usize> = if columns.is_empty() {
            (0..self.columns.len()).collect()
        } else {
            columns
                .iter()
                .map(|c| self.get_column_index(*c))
                .collect::<Result<_, _>>()?
        };
        self.sort_by(|a, b| {
            for i in &idx {
                let o = a[*i].compare(&b[*i]);
                if o != Ordering::Equal {
                    return o;
                }
            }
            Ordering::Equal
        });
        if reverse {
            self.rows.reverse();
            // Keep the sort stable for equal keys when reversing (like Python's `reverse=True`).
            self.stable_fix_reverse(&idx);
            self.rebuild_rows();
        }
        Ok(())
    }

    /// Python's `sorted(reverse=True)` keeps equal elements in original order; a plain
    /// reverse flips them, so re-reverse runs of equal keys.
    fn stable_fix_reverse(&mut self, idx: &[usize]) {
        let eq = |a: &Row, b: &Row| {
            idx.iter()
                .all(|i| a.cells[*i].compare(&b.cells[*i]) == Ordering::Equal)
        };
        let mut start = 0;
        while start < self.rows.len() {
            let mut end = start + 1;
            while end < self.rows.len() && eq(&self.rows[start], &self.rows[end]) {
                end += 1;
            }
            self.rows[start..end].reverse();
            start = end;
        }
    }

    /// Sort rows with a comparison over their cells (stable).
    pub fn sort_by(&mut self, mut f: impl FnMut(&[CellValue], &[CellValue]) -> Ordering) {
        self.rows.sort_by(|a, b| f(&a.cells, &b.cells));
        self.rebuild_rows();
    }

    // ------------------------------------------------------------------ geometry

    fn clamp(&self, c: Coordinate) -> Coordinate {
        Coordinate {
            row: c.row.min(self.rows.len().saturating_sub(1)),
            column: c.column.min(self.columns.len().saturating_sub(1)),
        }
    }

    fn should_render_row_labels(&self) -> bool {
        self.labelled_row_exists && self.show_row_labels
    }

    fn label_width(&self) -> i32 {
        if self.should_render_row_labels() {
            self.label_content_width + 2 * self.cell_padding
        } else {
            0
        }
    }

    fn header_h(&self) -> i32 {
        if self.show_header {
            self.header_height
        } else {
            0
        }
    }

    fn col_width(&self, i: usize) -> i32 {
        self.columns[i].render_width(self.cell_padding)
    }

    fn table_width(&self) -> i32 {
        self.label_width()
            + (0..self.columns.len())
                .map(|i| self.col_width(i))
                .sum::<i32>()
    }

    fn fixed_columns_width(&self) -> i32 {
        (0..self.fixed_columns.min(self.columns.len()))
            .map(|i| self.col_width(i))
            .sum()
    }

    /// Space at the top / left occupied by the header, fixed rows, labels and fixed columns.
    fn fixed_offset(&self) -> (i32, i32) {
        let top = self.header_h() + self.row_tops[self.fixed_rows.min(self.rows.len())];
        let left = self.label_width() + self.fixed_columns_width();
        (top, left)
    }

    fn column_x(&self, column: usize) -> i32 {
        self.label_width() + (0..column).map(|i| self.col_width(i)).sum::<i32>()
    }

    /// Virtual-space region of a cell.
    fn cell_region(&self, c: Coordinate) -> Region {
        if !self.is_valid_coordinate(c) {
            return Region::default();
        }
        Region::new(
            self.column_x(c.column),
            self.header_h() + self.row_tops[c.row],
            self.col_width(c.column),
            self.rows[c.row].height,
        )
    }

    /// Row at virtual y (header included), if any.
    fn row_at_y(&self, y: i32) -> Option<(RowRef, i32)> {
        let hh = self.header_h();
        if y < 0 {
            return None;
        }
        if y < hh {
            return Some((RowRef::Header, y));
        }
        let y = y - hh;
        if y >= self.total_height {
            return None;
        }
        // row_tops is sorted: the row is the last top <= y.
        let i = self.row_tops.partition_point(|t| *t <= y) - 1;
        Some((RowRef::Data(i), y - self.row_tops[i]))
    }

    /// Map a viewport line to a virtual y.
    fn virtual_y(&self, vy: i32, scroll_y: i32) -> i32 {
        let (top, _) = self.fixed_offset();
        if vy >= top {
            vy + scroll_y
        } else {
            vy
        }
    }

    /// Map a content position (viewport coordinates) to a cell.
    fn hit_test(&self, vx: i32, vy: i32, scroll: (i32, i32)) -> Option<Hit> {
        let (row_ref, _) = self.row_at_y(self.virtual_y(vy, scroll.1))?;
        let row = match row_ref {
            RowRef::Header => -1,
            RowRef::Data(i) => i as isize,
        };
        let label_w = self.label_width();
        let (_, left) = self.fixed_offset();
        let x = if vx < left { vx } else { vx + scroll.0 };
        if x < label_w {
            return Some(Hit {
                row,
                column: -1,
                out_of_bounds: false,
            });
        }
        let mut pos = label_w;
        for i in 0..self.columns.len() {
            let w = self.col_width(i);
            if x < pos + w {
                return Some(Hit {
                    row,
                    column: i as isize,
                    out_of_bounds: false,
                });
            }
            pos += w;
        }
        Some(Hit {
            row,
            column: 0,
            out_of_bounds: true,
        })
    }

    // ------------------------------------------------------------------ cursor

    /// Move the cursor (posting a `*Highlighted` message if it moved) and scroll it into view.
    pub fn move_cursor(&mut self, row: Option<usize>, column: Option<usize>, ctx: &mut Ctx) {
        let dest = Coordinate {
            row: row.unwrap_or(self.cursor.row),
            column: column.unwrap_or(self.cursor.column),
        };
        self.set_cursor_coordinate(dest, ctx);
    }

    /// Set the cursor coordinate (clamped); posts a `*Highlighted` message if it changed and
    /// scrolls it into view.
    pub fn set_cursor_coordinate(&mut self, c: Coordinate, ctx: &mut Ctx) {
        let new = self.clamp(c);
        let old = self.cursor;
        self.cursor = new;
        if old != new {
            match self.cursor_type {
                CursorType::Cell => self.highlight_coordinate(new, ctx),
                CursorType::Row => self.highlight_row(new.row, ctx),
                CursorType::Column => self.highlight_column(new.column, ctx),
                CursorType::None => {}
            }
        }
        self.scroll_cursor_into_view(ctx);
    }

    fn highlight_coordinate(&self, c: Coordinate, ctx: &mut Ctx) {
        if let (Ok(value), Ok(cell_key)) = (self.get_cell_at(c), self.coordinate_to_cell_key(c)) {
            ctx.post_message(CellHighlighted {
                value: value.clone(),
                coordinate: c,
                cell_key,
            });
        }
    }

    fn highlight_row(&self, row: usize, ctx: &mut Ctx) {
        if let Some(r) = self.rows.get(row) {
            ctx.post_message(RowHighlighted {
                cursor_row: row,
                row_key: r.key.clone(),
            });
        }
    }

    fn highlight_column(&self, column: usize, ctx: &mut Ctx) {
        if let Some(c) = self.columns.get(column) {
            ctx.post_message(ColumnHighlighted {
                cursor_column: column,
                column_key: c.key.clone(),
            });
        }
    }

    /// Post the `*Highlighted` message for the current cursor.
    pub fn highlight_cursor(&self, ctx: &mut Ctx) {
        match self.cursor_type {
            CursorType::Cell => self.highlight_coordinate(self.cursor, ctx),
            CursorType::Row => self.highlight_row(self.cursor.row, ctx),
            CursorType::Column => self.highlight_column(self.cursor.column, ctx),
            CursorType::None => {}
        }
    }

    /// Scroll so the cursor is visible outside the fixed header / rows / columns.
    pub fn scroll_cursor_into_view(&self, ctx: &mut Ctx) {
        if self.rows.is_empty() && self.cursor_type != CursorType::Column {
            return;
        }
        let size = ctx.size();
        if size.area() == 0 {
            return;
        }
        let (top, left) = self.fixed_offset();
        let scroll = ctx.scroll_offset();
        let viewport = Region::new(
            scroll.x + left,
            scroll.y + top,
            (size.width - left).max(1),
            (size.height - top).max(1),
        );
        let c = self.cursor;
        let (target, horizontal, vertical) = match self.cursor_type {
            CursorType::Row => {
                let r = self.cell_region(Coordinate::new(c.row, 0));
                (Region::new(0, r.y, 1, r.height), false, true)
            }
            CursorType::Column => {
                if !self.is_valid_column_index(c.column) {
                    return;
                }
                let x = self.column_x(c.column);
                (Region::new(x, 0, self.col_width(c.column), 1), true, false)
            }
            _ => (self.cell_region(c), true, true),
        };
        let delta = viewport.get_scroll_to_visible(&target);
        let dx = if horizontal && c.column >= self.fixed_columns {
            delta.x
        } else {
            0
        };
        let dy = if vertical && c.row >= self.fixed_rows {
            delta.y
        } else {
            0
        };
        if dx != 0 || dy != 0 {
            ctx.scroll_to(scroll.x + dx, scroll.y + dy);
        }
    }

    fn post_selected(&self, ctx: &mut Ctx) {
        if self.rows.is_empty() {
            return;
        }
        let c = self.cursor;
        let Ok(cell_key) = self.coordinate_to_cell_key(c) else {
            return;
        };
        match self.cursor_type {
            CursorType::Cell => ctx.post_message(CellSelected {
                value: self.rows[c.row].cells[c.column].clone(),
                coordinate: c,
                cell_key,
            }),
            CursorType::Row => ctx.post_message(RowSelected {
                cursor_row: c.row,
                row_key: cell_key.row_key,
            }),
            CursorType::Column => ctx.post_message(ColumnSelected {
                cursor_column: c.column,
                column_key: cell_key.column_key,
            }),
            CursorType::None => {}
        }
    }

    fn cursor_moves_vertically(&self) -> bool {
        self.show_cursor && matches!(self.cursor_type, CursorType::Cell | CursorType::Row)
    }

    fn cursor_moves_horizontally(&self) -> bool {
        self.show_cursor && matches!(self.cursor_type, CursorType::Cell | CursorType::Column)
    }

    fn page_height(&self, ctx: &Ctx) -> i32 {
        (ctx.size().height - self.header_h()).max(1)
    }

    pub fn action_page_down(&mut self, ctx: &mut Ctx) {
        self.show_hover_cursor = false;
        let height = self.page_height(ctx);
        if self.cursor_moves_vertically() && !self.rows.is_empty() {
            let row = self.cursor.row;
            let (mut offset, mut n) = (0, 0);
            for r in &self.rows[row..] {
                offset += r.height;
                n += 1;
                if offset > height {
                    break;
                }
            }
            ctx.scroll_by(0, height);
            self.move_cursor(Some(row + n - 1), None, ctx);
        } else {
            ctx.scroll_by(0, height);
        }
    }

    pub fn action_page_up(&mut self, ctx: &mut Ctx) {
        self.show_hover_cursor = false;
        let height = self.page_height(ctx);
        if self.cursor_moves_vertically() && !self.rows.is_empty() {
            let row = self.cursor.row;
            let (mut offset, mut n) = (0, 0);
            for r in self.rows[..=row].iter() {
                offset += r.height;
                n += 1;
                if offset > height {
                    break;
                }
            }
            ctx.scroll_by(0, -height);
            self.move_cursor(Some(row + 1 - n), None, ctx);
        } else {
            ctx.scroll_by(0, -height);
        }
    }

    pub fn action_scroll_top(&mut self, ctx: &mut Ctx) {
        self.show_hover_cursor = false;
        if self.cursor_moves_vertically() {
            self.move_cursor(Some(0), None, ctx);
        } else {
            let x = ctx.scroll_offset().x;
            ctx.scroll_to(x, 0);
        }
    }

    pub fn action_scroll_bottom(&mut self, ctx: &mut Ctx) {
        self.show_hover_cursor = false;
        if self.cursor_moves_vertically() {
            self.move_cursor(Some(self.rows.len().saturating_sub(1)), None, ctx);
        } else {
            let x = ctx.scroll_offset().x;
            ctx.scroll_to(x, i32::MAX / 2);
        }
    }

    pub fn action_scroll_home(&mut self, ctx: &mut Ctx) {
        self.show_hover_cursor = false;
        if self.cursor_moves_horizontally() {
            self.move_cursor(None, Some(0), ctx);
        } else {
            let y = ctx.scroll_offset().y;
            ctx.scroll_to(0, y);
        }
    }

    pub fn action_scroll_end(&mut self, ctx: &mut Ctx) {
        self.show_hover_cursor = false;
        if self.cursor_moves_horizontally() {
            self.move_cursor(None, Some(self.columns.len().saturating_sub(1)), ctx);
        } else {
            let y = ctx.scroll_offset().y;
            ctx.scroll_to(i32::MAX / 2, y);
        }
    }

    pub fn action_cursor_up(&mut self, ctx: &mut Ctx) {
        self.show_hover_cursor = false;
        if self.cursor_moves_vertically() {
            let c = self.cursor;
            self.set_cursor_coordinate(Coordinate::new(c.row.saturating_sub(1), c.column), ctx);
        } else {
            ctx.scroll_by(0, -1);
        }
    }

    pub fn action_cursor_down(&mut self, ctx: &mut Ctx) {
        self.show_hover_cursor = false;
        if self.cursor_moves_vertically() {
            let c = self.cursor;
            self.set_cursor_coordinate(Coordinate::new(c.row + 1, c.column), ctx);
        } else {
            ctx.scroll_by(0, 1);
        }
    }

    pub fn action_cursor_left(&mut self, ctx: &mut Ctx) {
        self.show_hover_cursor = false;
        if self.cursor_moves_horizontally() {
            let c = self.cursor;
            self.set_cursor_coordinate(Coordinate::new(c.row, c.column.saturating_sub(1)), ctx);
        } else {
            ctx.scroll_by(-1, 0);
        }
    }

    pub fn action_cursor_right(&mut self, ctx: &mut Ctx) {
        self.show_hover_cursor = false;
        if self.cursor_moves_horizontally() {
            let c = self.cursor;
            self.set_cursor_coordinate(Coordinate::new(c.row, c.column + 1), ctx);
        } else {
            ctx.scroll_by(1, 0);
        }
    }

    pub fn action_select_cursor(&mut self, ctx: &mut Ctx) {
        self.show_hover_cursor = false;
        if self.show_cursor && self.cursor_type != CursorType::None {
            self.post_selected(ctx);
        }
    }

    fn mouse_hit(&self, m: &MouseEvent, ctx: &Ctx) -> Option<Hit> {
        let content = ctx.content_region();
        if !content.contains(m.screen_x, m.screen_y) {
            return None;
        }
        let s = ctx.scroll_offset();
        self.hit_test(m.screen_x - content.x, m.screen_y - content.y, (s.x, s.y))
    }

    fn on_click(&mut self, m: &MouseEvent, ctx: &mut Ctx) -> Flow {
        self.show_hover_cursor = true;
        let Some(hit) = self.mouse_hit(m, ctx) else {
            return Flow::Continue;
        };
        if self.cursor_type != CursorType::Row && hit.out_of_bounds {
            return Flow::Continue;
        }
        if self.show_header && hit.row == -1 {
            if hit.column >= 0 {
                let col = &self.columns[hit.column as usize];
                ctx.post_message(HeaderSelected {
                    column_key: col.key.clone(),
                    column_index: hit.column as usize,
                    label: col.label.clone(),
                });
            }
            Flow::Continue
        } else if self.show_row_labels && hit.column == -1 {
            if hit.row >= 0 {
                let row = &self.rows[hit.row as usize];
                ctx.post_message(RowLabelSelected {
                    row_key: row.key.clone(),
                    row_index: hit.row as usize,
                    label: row.label.clone().unwrap_or_default(),
                });
            }
            Flow::Continue
        } else if self.show_cursor && self.cursor_type != CursorType::None && hit.row >= 0 {
            let new = Coordinate::new(hit.row as usize, hit.column.max(0) as usize);
            let same = new == self.cursor;
            self.set_cursor_coordinate(new, ctx);
            if same {
                self.post_selected(ctx);
            }
            Flow::Stop
        } else {
            Flow::Continue
        }
    }

    // ------------------------------------------------------------------ rendering

    fn should_highlight(cursor: (isize, isize), cell: (isize, isize), t: CursorType) -> bool {
        match t {
            CursorType::Cell => cursor == cell,
            CursorType::Row => cursor.0 == cell.0,
            CursorType::Column => cursor.1 == cell.1,
            CursorType::None => false,
        }
    }

    fn resolve_styles(ctx: &RenderCtx) -> Styles {
        Styles {
            header: ctx.component_style("datatable--header"),
            fixed: ctx.component_style("datatable--fixed"),
            even: ctx.component_style("datatable--even-row"),
            odd: ctx.component_style("datatable--odd-row"),
            cursor: ctx.component_style("datatable--cursor"),
            hover: ctx.component_style("datatable--hover"),
            header_cursor: ctx.component_style("datatable--header-cursor"),
            header_hover: ctx.component_style("datatable--header-hover"),
            fixed_cursor: ctx.component_style("datatable--fixed-cursor"),
            background: ctx.background(),
        }
    }

    /// `_get_styles_to_render_cell`: (component style, post style).
    fn cell_component_style(
        &self,
        st: &Styles,
        is_header_or_label: bool,
        is_fixed: bool,
        hover: bool,
        cursor: bool,
    ) -> (Style, Style) {
        let mut comp = Style::default();
        if hover && self.show_cursor && self.show_hover_cursor {
            comp = comp.combine(&st.hover);
            if is_header_or_label {
                comp = comp.combine(&st.header_hover);
            }
        }
        if cursor && self.show_cursor {
            comp = comp.combine(&st.cursor);
            if is_header_or_label {
                comp = comp.combine(&st.header_cursor);
            } else if is_fixed {
                comp = comp.combine(&st.fixed_cursor);
            }
        }
        let mut post = Style::default();
        if self.cursor_foreground_css {
            post.fg = comp.fg;
        }
        if self.cursor_background_css {
            post.bg = comp.bg;
        }
        (comp, post)
    }

    /// `_get_row_style`.
    fn row_style(&self, st: &Styles, row: RowRef) -> Style {
        match row {
            RowRef::Header => st.header.clone(),
            RowRef::Data(i) if i < self.fixed_rows => st.fixed.clone(),
            RowRef::Data(i) if self.zebra_stripes => {
                if i % 2 == 1 {
                    st.odd.clone()
                } else {
                    st.even.clone()
                }
            }
            RowRef::Data(_) => Style::default(),
        }
    }

    /// Render one line of one cell to exactly `width` cells.
    #[allow(clippy::too_many_arguments)]
    fn render_cell_line(
        &self,
        text: &Text,
        justify: Justify,
        row_height: i32,
        line_no: i32,
        width: i32,
        base: &Style,
        comp: &Style,
        post: &Style,
    ) -> Strip {
        let pre = base.combine(comp);
        let pad = self.cell_padding.min(width / 2);
        let inner = (width - 2 * pad).max(0);
        let line = if row_height <= 1 {
            if line_no == 0 {
                text.lines().into_iter().next()
            } else {
                None
            }
        } else {
            text.wrap(inner.max(1), Justify::Left, TextOverflow::Fold, false)
                .into_iter()
                .nth(line_no as usize)
        }
        .unwrap_or_default();
        let mut out = Strip::default();
        if pad > 0 {
            out.push(Span::new(" ".repeat(pad as usize), pre.combine(post)));
        }
        let line = line.crop(0, inner);
        // Justify within the cell (trailing whitespace of wrapped lines doesn't count).
        let len = cell_len(line.plain_text().trim_end());
        let lead = match justify {
            Justify::Right => inner - len,
            Justify::Center => (inner - len) / 2,
            _ => 0,
        }
        .max(0);
        if lead > 0 {
            out.push(Span::new(" ".repeat(lead as usize), pre.combine(post)));
        }
        let line = if lead > 0 {
            line.crop(0, inner - lead)
        } else {
            line
        };
        let styled = Strip::new(
            line.spans
                .into_iter()
                .map(|s| Span::new(s.text, pre.combine(&s.style).combine(post)))
                .collect(),
        );
        out.spans
            .extend(styled.set_cell_len(inner - lead, &pre.combine(post)).spans);
        if pad > 0 {
            out.push(Span::new(" ".repeat(pad as usize), pre.combine(post)));
        }
        out
    }

    /// Render line `line_no` of `row`, horizontally scrolled to `scroll_x`, `width` cells wide.
    fn render_line(
        &self,
        ctx: &RenderCtx,
        st: &Styles,
        row: RowRef,
        line_no: i32,
        scroll_x: i32,
        width: i32,
    ) -> Strip {
        let markup = |s: &str| ctx.markup(s);
        let row_index: isize = match row {
            RowRef::Header => -1,
            RowRef::Data(i) => i as isize,
        };
        let row_height = match row {
            RowRef::Header => self.header_height,
            RowRef::Data(i) => self.rows[i].height,
        };
        let cursor = (self.cursor.row as isize, self.cursor.column as isize);
        let t = self.cursor_type;
        let cell_text = |c: usize| -> (Text, Justify) {
            match row {
                RowRef::Header => (self.columns[c].label.clone(), Justify::Left),
                RowRef::Data(i) => {
                    let cell = &self.rows[i].cells[c];
                    (cell.to_text(&markup), cell.get_justify())
                }
            }
        };
        let row_style = self.row_style(st, row);
        let render_col = |c: usize, base: &Style, is_fixed: bool| -> Strip {
            let cell = (row_index, c as isize);
            let (comp, post) = self.cell_component_style(
                st,
                row == RowRef::Header,
                is_fixed && row != RowRef::Header,
                Self::should_highlight(self.hover, cell, t),
                Self::should_highlight(cursor, cell, t),
            );
            let (text, justify) = cell_text(c);
            self.render_cell_line(
                &text,
                justify,
                row_height,
                line_no,
                self.col_width(c),
                base,
                &comp,
                &post,
            )
        };

        // Fixed part: row label + fixed columns.
        let mut fixed = Strip::default();
        if self.should_render_row_labels() {
            let label = match row {
                RowRef::Header => Text::new(),
                RowRef::Data(i) => self.rows[i].label.clone().unwrap_or_default(),
            };
            let cell = (row_index, -1);
            let (comp, post) = self.cell_component_style(
                st,
                true,
                false,
                Self::should_highlight(self.hover, cell, t),
                Self::should_highlight(cursor, cell, t),
            );
            fixed.spans.extend(
                self.render_cell_line(
                    &label,
                    Justify::Left,
                    row_height,
                    line_no,
                    self.label_width(),
                    &st.header,
                    &comp,
                    &post,
                )
                .spans,
            );
        }
        let n_fixed = self.fixed_columns.min(self.columns.len());
        let fixed_style = if row == RowRef::Header {
            st.header.clone()
        } else {
            st.fixed.clone()
        };
        for c in 0..n_fixed {
            fixed.spans.extend(render_col(c, &fixed_style, true).spans);
        }
        let fixed_w = fixed.cell_len();
        let avail = (width - fixed_w).max(0);

        // Scrollable part: only columns intersecting [scroll_x, scroll_x + avail).
        let x1 = scroll_x;
        let x2 = scroll_x + avail;
        let mut scrollable = Strip::default();
        let mut pos = 0;
        let mut first_pos: Option<i32> = None;
        for c in n_fixed..self.columns.len() {
            let w = self.col_width(c);
            if pos + w <= x1 {
                pos += w;
                continue;
            }
            if pos >= x2 {
                break;
            }
            first_pos.get_or_insert(pos);
            scrollable
                .spans
                .extend(render_col(c, &row_style, false).spans);
            pos += w;
        }
        // Extend the row style to fill the widget (`remaining_space`).
        let table_w: i32 = (n_fixed..self.columns.len())
            .map(|c| self.col_width(c))
            .sum();
        if table_w < x2 {
            let extend_style = if t == CursorType::Row {
                let (comp, _) = self.cell_component_style(
                    st,
                    row == RowRef::Header,
                    false,
                    Self::should_highlight(self.hover, (row_index, 0), t),
                    row_index == cursor.0,
                );
                row_style.combine(&comp)
            } else {
                let mut s = Style {
                    fg: row_style.fg,
                    bg: row_style.bg,
                    ..Style::default()
                };
                if let Some(bg) = row_style.bg {
                    s.bg = Some(bg.blend(st.background, 0.25));
                }
                s
            };
            first_pos.get_or_insert(pos.max(table_w));
            let start = pos.max(x1).max(table_w);
            if pos < start {
                // Columns ended before the window: pad up to the window start.
                scrollable.spans.push(Span::new(
                    " ".repeat((start - pos) as usize),
                    Style::default(),
                ));
            }
            scrollable.spans.push(Span::new(
                " ".repeat((x2 - start).max(0) as usize),
                extend_style,
            ));
        }
        let origin = first_pos.unwrap_or(x1);
        let visible = scrollable.crop(x1 - origin, x2 - origin);
        fixed.spans.extend(visible.spans);
        fixed.set_cell_len(width, &Style::default()).simplify()
    }
}

impl Widget for DataTable {
    fn type_names(&self) -> &'static [&'static str] {
        &["DataTable", "ScrollView"]
    }

    fn default_css(&self) -> &'static str {
        DEFAULT_CSS
    }

    fn can_focus(&self) -> bool {
        true
    }

    fn on_mount(&mut self, ctx: &mut Ctx) {
        if self.show_cursor && !self.rows.is_empty() && !self.columns.is_empty() {
            self.highlight_cursor(ctx);
        }
    }

    fn help(&self) -> Option<&str> {
        Some(
            "\
## Data table

- **Arrow keys** move the cursor, **page up** / **page down** scroll.
- **enter** selects the cell, row or column under the cursor.",
        )
    }

    fn bindings(&self) -> Vec<Binding> {
        vec![
            Binding::new("enter", "select_cursor", "Select").hidden(),
            Binding::new("up", "cursor_up", "Cursor up").hidden(),
            Binding::new("down", "cursor_down", "Cursor down").hidden(),
            Binding::new("right", "cursor_right", "Cursor right").hidden(),
            Binding::new("left", "cursor_left", "Cursor left").hidden(),
            Binding::new("pageup", "page_up", "Page up").hidden(),
            Binding::new("pagedown", "page_down", "Page down").hidden(),
            Binding::new("ctrl+home", "scroll_top", "Top").hidden(),
            Binding::new("ctrl+end", "scroll_bottom", "Bottom").hidden(),
            Binding::new("home", "scroll_home", "Home").hidden(),
            Binding::new("end", "scroll_end", "End").hidden(),
        ]
    }

    fn run_action(&mut self, action: &str, _args: &[String], ctx: &mut Ctx) -> bool {
        match action {
            "select_cursor" => self.action_select_cursor(ctx),
            "cursor_up" => self.action_cursor_up(ctx),
            "cursor_down" => self.action_cursor_down(ctx),
            "cursor_right" => self.action_cursor_right(ctx),
            "cursor_left" => self.action_cursor_left(ctx),
            "page_up" => self.action_page_up(ctx),
            "page_down" => self.action_page_down(ctx),
            "scroll_top" => self.action_scroll_top(ctx),
            "scroll_bottom" => self.action_scroll_bottom(ctx),
            "scroll_home" => self.action_scroll_home(ctx),
            "scroll_end" => self.action_scroll_end(ctx),
            _ => return false,
        }
        true
    }

    fn on_event(&mut self, event: &Event, ctx: &mut Ctx) -> Flow {
        match event {
            Event::Click(m) => self.on_click(m, ctx),
            Event::MouseMove(m) => {
                self.show_hover_cursor = true;
                match self.mouse_hit(m, ctx) {
                    None => self.show_hover_cursor = false,
                    Some(hit) if self.cursor_type != CursorType::Row && hit.out_of_bounds => {
                        self.show_hover_cursor = false
                    }
                    Some(hit) => {
                        if self.show_cursor && self.cursor_type != CursorType::None {
                            self.hover = (hit.row, hit.column);
                        }
                    }
                }
                Flow::Continue
            }
            Event::Leave => {
                self.show_hover_cursor = false;
                Flow::Continue
            }
            _ => Flow::Continue,
        }
    }

    fn virtual_size(&self, _ctx: &RenderCtx) -> Option<Size> {
        Some(Size::new(
            self.table_width(),
            self.total_height + self.header_h(),
        ))
    }

    fn content_width(&self, _ctx: &RenderCtx) -> Option<i32> {
        Some(self.table_width())
    }

    fn content_height(&self, ctx: &RenderCtx, width: i32) -> Option<i32> {
        let mut h = self.total_height + self.header_h();
        let st = ctx.styles();
        if st.get_overflow_x() == Overflow::Auto && self.table_width() > width {
            h += st.get_scrollbar_size_horizontal();
        }
        Some(h)
    }

    fn render(&self, ctx: &RenderCtx) -> Content {
        let Size { width, height } = ctx.size;
        let st = Self::resolve_styles(ctx);
        let scroll = ctx.scroll_offset;
        let mut out = Vec::with_capacity(height.max(0) as usize);
        for vy in 0..height.max(0) {
            let y = self.virtual_y(vy, scroll.y);
            match self.row_at_y(y) {
                Some((row, line_no)) => {
                    out.push(self.render_line(ctx, &st, row, line_no, scroll.x, width))
                }
                None => break,
            }
        }
        Content::Lines(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compare_values() {
        let a = CellValue::from("10");
        let b = CellValue::from(9);
        assert_eq!(a.compare(&b), Ordering::Greater);
        assert_eq!(
            CellValue::from("apple").compare(&"banana".into()),
            Ordering::Less
        );
        assert_eq!(CellValue::from(1).compare(&"a".into()), Ordering::Less);
        assert_eq!(CellValue::Empty.compare(&1.into()), Ordering::Less);
        assert_eq!(CellValue::from(1.5).plain(), "1.50");
    }

    #[test]
    fn row_lookup_and_sort() {
        let mut t = DataTable::new();
        t.add_columns(&["a", "b"]);
        t.add_row_with(["x", "3"], Some(1), Some("r1"), None)
            .unwrap();
        t.add_row_with(["y", "1"], Some(2), Some("r2"), None)
            .unwrap();
        t.add_row_with(["z", "2"], Some(1), Some("r3"), None)
            .unwrap();
        assert_eq!(t.row_at_y(1), Some((RowRef::Data(0), 0)));
        assert_eq!(t.row_at_y(3), Some((RowRef::Data(1), 1)));
        assert_eq!(t.row_at_y(4), Some((RowRef::Data(2), 0)));
        assert_eq!(t.row_at_y(5), None);
        let key = t.columns[1].key.clone();
        t.sort(&[key.value()], false).unwrap();
        assert_eq!(t.get_row_index("r2").unwrap(), 0);
        t.sort(&[key.value()], true).unwrap();
        assert_eq!(t.get_row_index("r1").unwrap(), 0);
        t.remove_row("r1").unwrap();
        assert_eq!(t.row_count(), 2);
        assert_eq!(t.get_row_index("r3").unwrap(), 0);
        assert!(t.add_row_with(["q"], None, Some("r2"), None).is_err());
    }
}

//! Data widgets: DataTable, ListView / ListItem, Tree, DirectoryTree, Log, RichLog, Pretty,
//! ProgressBar, Sparkline, Digits.
//!
//! Tables, trees and logs are often filled before they are mounted (`table.add_row(...)` in
//! `compose`), so the Python object may own a *detached* Rust widget ([`Detached`]) until it
//! mounts; the kind constructor then takes it over (the `_detached` kwarg). All methods go
//! through [`call_opt`], which gets a `Ctx` only while the widget is mounted: operations that
//! need one (posting messages, scrolling) fall back to their ctx-free form or do nothing.

use super::{arg, as_mut, as_ref, kw, new_py, none, to_py, Registry};
use pyo3::exceptions::{PyAttributeError, PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyBool, PyDict, PyFloat, PyInt, PyList, PyString, PyTuple};
use rustual::render::Justify;
use rustual::widgets::data_table::{self as dt, CellValue, Coordinate, CursorType, DataTableError};
use rustual::widgets::{self as w, directory_tree, list_view, pretty, tree};
use rustual::{Color, Ctx, Message, Text, Widget};
use std::cmp::Ordering;
use std::collections::HashMap;
use std::path::PathBuf;

/// The Python `Tree` widget: node data lives on the Python `TreeNode` objects.
type PyTree = w::Tree<()>;

pub(crate) fn register(r: &mut Registry) {
    r.kind("DataTable", |py, k, _| create(py, "DataTable", k));
    r.kind("Tree", |py, k, _| create(py, "Tree", k));
    r.kind("DirectoryTree", |py, k, _| create(py, "DirectoryTree", k));
    r.kind("Log", |py, k, _| create(py, "Log", k));
    r.kind("RichLog", |py, k, _| create(py, "RichLog", k));
    r.kind("Pretty", |py, k, _| create(py, "Pretty", k));
    r.kind("ProgressBar", |py, k, _| create(py, "ProgressBar", k));
    r.kind("Sparkline", |py, k, _| create(py, "Sparkline", k));
    r.kind("Digits", |py, k, _| create(py, "Digits", k));
    r.kind("ListView", |py, k, _| create(py, "ListView", k));
    r.kind("ListItem", |py, k, _| create(py, "ListItem", k));

    r.getter(get).caller(call);

    // DataTable
    r.message::<dt::CellHighlighted>("on_data_table_cell_highlighted", |py, m: &Message| {
        let p = m.get::<dt::CellHighlighted>().expect("CellHighlighted");
        cell_message(
            py,
            "DataTable.CellHighlighted",
            &p.value,
            p.coordinate,
            &p.cell_key,
        )
    });
    r.message::<dt::CellSelected>("on_data_table_cell_selected", |py, m: &Message| {
        let p = m.get::<dt::CellSelected>().expect("CellSelected");
        cell_message(
            py,
            "DataTable.CellSelected",
            &p.value,
            p.coordinate,
            &p.cell_key,
        )
    });
    r.message::<dt::RowHighlighted>("on_data_table_row_highlighted", |py, m: &Message| {
        let p = m.get::<dt::RowHighlighted>().expect("RowHighlighted");
        let d = PyDict::new(py);
        d.set_item("cursor_row", p.cursor_row)?;
        d.set_item("row_key", &p.row_key.0)?;
        new_py(py, "DataTable.RowHighlighted", &d)
    });
    r.message::<dt::RowSelected>("on_data_table_row_selected", |py, m: &Message| {
        let p = m.get::<dt::RowSelected>().expect("RowSelected");
        let d = PyDict::new(py);
        d.set_item("cursor_row", p.cursor_row)?;
        d.set_item("row_key", &p.row_key.0)?;
        new_py(py, "DataTable.RowSelected", &d)
    });
    r.message::<dt::ColumnHighlighted>("on_data_table_column_highlighted", |py, m: &Message| {
        let p = m.get::<dt::ColumnHighlighted>().expect("ColumnHighlighted");
        let d = PyDict::new(py);
        d.set_item("cursor_column", p.cursor_column)?;
        d.set_item("column_key", &p.column_key.0)?;
        new_py(py, "DataTable.ColumnHighlighted", &d)
    });
    r.message::<dt::ColumnSelected>("on_data_table_column_selected", |py, m: &Message| {
        let p = m.get::<dt::ColumnSelected>().expect("ColumnSelected");
        let d = PyDict::new(py);
        d.set_item("cursor_column", p.cursor_column)?;
        d.set_item("column_key", &p.column_key.0)?;
        new_py(py, "DataTable.ColumnSelected", &d)
    });
    r.message::<dt::HeaderSelected>("on_data_table_header_selected", |py, m: &Message| {
        let p = m.get::<dt::HeaderSelected>().expect("HeaderSelected");
        let d = PyDict::new(py);
        d.set_item("column_key", &p.column_key.0)?;
        d.set_item("column_index", p.column_index)?;
        d.set_item("label", p.label.plain_text())?;
        new_py(py, "DataTable.HeaderSelected", &d)
    });
    r.message::<dt::RowLabelSelected>("on_data_table_row_label_selected", |py, m: &Message| {
        let p = m.get::<dt::RowLabelSelected>().expect("RowLabelSelected");
        let d = PyDict::new(py);
        d.set_item("row_key", &p.row_key.0)?;
        d.set_item("row_index", p.row_index)?;
        d.set_item("label", p.label.plain_text())?;
        new_py(py, "DataTable.RowLabelSelected", &d)
    });

    // Tree (and DirectoryTree, which posts the same messages)
    r.message::<tree::NodeSelected>("on_tree_node_selected", |py, m: &Message| {
        let p = m.get::<tree::NodeSelected>().expect("NodeSelected");
        node_message(py, "Tree.NodeSelected", p.node)
    });
    r.message::<tree::NodeExpanded>("on_tree_node_expanded", |py, m: &Message| {
        let p = m.get::<tree::NodeExpanded>().expect("NodeExpanded");
        node_message(py, "Tree.NodeExpanded", p.node)
    });
    r.message::<tree::NodeCollapsed>("on_tree_node_collapsed", |py, m: &Message| {
        let p = m.get::<tree::NodeCollapsed>().expect("NodeCollapsed");
        node_message(py, "Tree.NodeCollapsed", p.node)
    });
    r.message::<tree::NodeHighlighted>("on_tree_node_highlighted", |py, m: &Message| {
        let p = m.get::<tree::NodeHighlighted>().expect("NodeHighlighted");
        node_message(py, "Tree.NodeHighlighted", p.node)
    });
    r.message::<directory_tree::FileSelected>(
        "on_directory_tree_file_selected",
        |py, m: &Message| {
            let p = m
                .get::<directory_tree::FileSelected>()
                .expect("FileSelected");
            path_message(py, "DirectoryTree.FileSelected", p.node, &p.path)
        },
    );
    r.message::<directory_tree::DirectorySelected>(
        "on_directory_tree_directory_selected",
        |py, m: &Message| {
            let p = m
                .get::<directory_tree::DirectorySelected>()
                .expect("DirectorySelected");
            path_message(py, "DirectoryTree.DirectorySelected", p.node, &p.path)
        },
    );

    // ListView
    r.message::<list_view::Highlighted>("on_list_view_highlighted", |py, m: &Message| {
        let p = m.get::<list_view::Highlighted>().expect("Highlighted");
        let d = PyDict::new(py);
        d.set_item("index", p.index)?;
        d.set_item("item", p.item.map(crate::bridge::node_to_int))?;
        new_py(py, "ListView.Highlighted", &d)
    });
    r.message::<list_view::Selected>("on_list_view_selected", |py, m: &Message| {
        let p = m.get::<list_view::Selected>().expect("Selected");
        let d = PyDict::new(py);
        d.set_item("index", p.index)?;
        d.set_item("item", crate::bridge::node_to_int(p.item))?;
        new_py(py, "ListView.Selected", &d)
    });
}

/// Register the module-level functions and classes of this group.
pub(crate) fn add_to_module(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Detached>()?;
    m.add_function(wrap_pyfunction!(detached_new, m)?)?;
    Ok(())
}

// ---------------------------------------------------------------------------- detached widgets

/// A built-in widget owned by a Python object that isn't mounted yet.
#[pyclass(unsendable, module = "rustual._native")]
pub struct Detached {
    widget: Option<Box<dyn Widget>>,
}

impl Detached {
    fn widget(&mut self) -> PyResult<&mut Box<dyn Widget>> {
        self.widget
            .as_mut()
            .ok_or_else(|| PyRuntimeError::new_err("the widget has been mounted"))
    }
}

#[pymethods]
impl Detached {
    /// Read an attribute (as `widget_get`).
    fn get(&mut self, py: Python<'_>, attr: &str) -> PyResult<Py<PyAny>> {
        let w = self.widget()?;
        super::get(py, w.as_ref(), attr)
    }

    /// Call a method (as `widget_call`, without a ctx).
    fn call(
        &mut self,
        py: Python<'_>,
        method: &str,
        args: &Bound<'_, PyTuple>,
    ) -> PyResult<Py<PyAny>> {
        let w = self.widget()?;
        call_opt(py, w.as_mut(), None, method, args).unwrap_or_else(|| {
            Err(PyAttributeError::new_err(format!(
                "no native method {method:?}"
            )))
        })
    }
}

/// Create a detached widget of `kind` from constructor kwargs.
#[pyfunction]
pub fn detached_new(py: Python<'_>, kind: &str, kwargs: &Bound<'_, PyDict>) -> PyResult<Detached> {
    Ok(Detached {
        widget: Some(build(py, kind, kwargs)?),
    })
}

fn create(py: Python<'_>, kind: &str, k: &Bound<'_, PyDict>) -> PyResult<Box<dyn Widget>> {
    if let Some(d) = k.get_item("_detached")? {
        if !d.is_none() {
            let d = d.cast::<Detached>()?;
            if let Some(widget) = d.borrow_mut().widget.take() {
                return Ok(widget);
            }
        }
    }
    build(py, kind, k)
}

fn build(_py: Python<'_>, kind: &str, k: &Bound<'_, PyDict>) -> PyResult<Box<dyn Widget>> {
    Ok(match kind {
        "DataTable" => {
            let mut t = w::DataTable::new()
                .show_header(kw(k, "show_header")?.unwrap_or(true))
                .show_row_labels(kw(k, "show_row_labels")?.unwrap_or(true))
                .fixed_rows(kw(k, "fixed_rows")?.unwrap_or(0))
                .fixed_columns(kw(k, "fixed_columns")?.unwrap_or(0))
                .zebra_stripes(kw(k, "zebra_stripes")?.unwrap_or(false))
                .header_height(kw(k, "header_height")?.unwrap_or(1))
                .show_cursor(kw(k, "show_cursor")?.unwrap_or(true))
                .cell_padding(kw(k, "cell_padding")?.unwrap_or(1));
            if let Some(c) = kw::<String>(k, "cursor_type")? {
                t = t.cursor_type(cursor_type(&c)?);
            }
            if let Some(p) = kw::<String>(k, "cursor_foreground_priority")? {
                t = t.cursor_foreground_priority_css(p == "css");
            }
            if let Some(p) = kw::<String>(k, "cursor_background_priority")? {
                t = t.cursor_background_priority_css(p == "css");
            }
            Box::new(t)
        }
        "Tree" => {
            let label: String = kw(k, "label")?.unwrap_or_default();
            Box::new(PyTree::new(label))
        }
        "DirectoryTree" => {
            let path: PathBuf = kw(k, "path")?.unwrap_or_else(|| PathBuf::from("."));
            let mut d = w::DirectoryTree::new(path);
            if let Some(filter) = kw::<Py<PyAny>>(k, "filter")? {
                d = d.filter_paths(move |p| {
                    Python::attach(|py| {
                        filter
                            .call1(py, (p.to_string_lossy().into_owned(),))
                            .and_then(|r| r.is_truthy(py))
                            .unwrap_or(true)
                    })
                });
            }
            Box::new(d)
        }
        "Log" => Box::new(
            w::Log::new()
                .max_lines(kw(k, "max_lines")?)
                .auto_scroll(kw(k, "auto_scroll")?.unwrap_or(true))
                .highlight(kw(k, "highlight")?.unwrap_or(false)),
        ),
        "RichLog" => Box::new(
            w::RichLog::new()
                .max_lines(kw(k, "max_lines")?)
                .min_width(kw(k, "min_width")?.unwrap_or(78))
                .wrap(kw(k, "wrap")?.unwrap_or(false))
                .markup(kw(k, "markup")?.unwrap_or(false))
                .highlight(kw(k, "highlight")?.unwrap_or(false))
                .auto_scroll(kw(k, "auto_scroll")?.unwrap_or(true)),
        ),
        "Pretty" => Box::new(w::Pretty::from_reprs(
            kw::<String>(k, "compact")?.unwrap_or_default(),
            kw::<String>(k, "expanded")?.unwrap_or_default(),
        )),
        "ProgressBar" => Box::new(
            w::ProgressBar::new(kw(k, "total")?)
                .show_bar(kw(k, "show_bar")?.unwrap_or(true))
                .show_percentage(kw(k, "show_percentage")?.unwrap_or(true))
                .show_eta(kw(k, "show_eta")?.unwrap_or(true)),
        ),
        "Sparkline" => {
            let data: Vec<f64> = kw(k, "data")?.unwrap_or_default();
            let mut s = w::Sparkline::new(data);
            if let Some(c) = kw::<String>(k, "min_color")? {
                s = s.min_color(color(&c)?);
            }
            if let Some(c) = kw::<String>(k, "max_color")? {
                s = s.max_color(color(&c)?);
            }
            if let Some(f) = kw::<Py<PyAny>>(k, "summary_function")? {
                s = s.summary_function(move |values| {
                    Python::attach(|py| {
                        f.call1(py, (values.to_vec(),))
                            .and_then(|r| r.extract::<f64>(py))
                            .unwrap_or(0.0)
                    })
                });
            }
            Box::new(s)
        }
        "Digits" => Box::new(w::Digits::new(
            kw::<String>(k, "value")?.unwrap_or_default(),
        )),
        "ListView" => {
            let index: Option<i64> = kw(k, "initial_index")?;
            Box::new(w::ListView::new().initial_index(index.map(|i| i.max(0) as usize)))
        }
        "ListItem" => Box::new(w::ListItem::new()),
        _ => {
            return Err(PyValueError::new_err(format!(
                "unknown data widget kind {kind:?}"
            )))
        }
    })
}

fn color(s: &str) -> PyResult<Color> {
    Color::parse(s).map_err(|e| PyValueError::new_err(format!("invalid color {s:?}: {e:?}")))
}

// ---------------------------------------------------------------------------- dispatch

fn get(py: Python<'_>, wd: &dyn Widget, attr: &str) -> Option<PyResult<Py<PyAny>>> {
    if let Some(t) = as_ref::<w::DataTable>(wd) {
        return table_get(py, t, attr);
    }
    if let Some(t) = as_ref::<PyTree>(wd) {
        return tree_get(py, t, attr);
    }
    if let Some(d) = as_ref::<w::DirectoryTree>(wd) {
        return match attr {
            "path" => Some(to_py(py, d.path().to_string_lossy().into_owned())),
            _ => tree_get(py, d.tree(), attr),
        };
    }
    if let Some(l) = as_ref::<w::Log>(wd) {
        return match attr {
            "lines" => Some(to_py(py, log_lines(l))),
            "line_count" => Some(to_py(py, l.line_count())),
            "max_lines" => Some(to_py(py, l.get_max_lines())),
            "auto_scroll" => Some(to_py(py, l.get_auto_scroll())),
            "highlight" => Some(to_py(py, l.is_highlight())),
            _ => None,
        };
    }
    if let Some(l) = as_ref::<w::RichLog>(wd) {
        return match attr {
            "lines" => Some(to_py(py, l.plain_lines())),
            "line_count" => Some(to_py(py, l.line_count())),
            "max_lines" => Some(to_py(py, l.get_max_lines())),
            "min_width" => Some(to_py(py, l.get_min_width())),
            "wrap" => Some(to_py(py, l.get_wrap())),
            "markup" => Some(to_py(py, l.is_markup())),
            "highlight" => Some(to_py(py, l.is_highlighting())),
            "auto_scroll" => Some(to_py(py, l.get_auto_scroll())),
            _ => None,
        };
    }
    if let Some(p) = as_ref::<w::ProgressBar>(wd) {
        return match attr {
            "total" => Some(to_py(py, p.total())),
            "progress" => Some(to_py(py, p.progress())),
            "percentage" => Some(to_py(py, p.percentage())),
            "eta" => Some(to_py(py, p.eta())),
            _ => None,
        };
    }
    if let Some(s) = as_ref::<w::Sparkline>(wd) {
        return match attr {
            "data" => Some(to_py(py, s.data().to_vec())),
            _ => None,
        };
    }
    if let Some(d) = as_ref::<w::Digits>(wd) {
        return match attr {
            "value" => Some(to_py(py, d.value())),
            _ => None,
        };
    }
    if let Some(l) = as_ref::<w::ListView>(wd) {
        return match attr {
            "index" => Some(to_py(py, l.index())),
            _ => None,
        };
    }
    None
}

fn call(
    py: Python<'_>,
    wd: &mut dyn Widget,
    ctx: &mut Ctx,
    method: &str,
    args: &Bound<'_, PyTuple>,
) -> Option<PyResult<Py<PyAny>>> {
    let r = call_opt(py, wd, Some(&mut *ctx), method, args);
    if r.is_some() {
        ctx.refresh();
    }
    r
}

/// Method dispatch for mounted (`ctx` is `Some`) and detached widgets.
fn call_opt(
    py: Python<'_>,
    wd: &mut dyn Widget,
    ctx: Option<&mut Ctx>,
    method: &str,
    args: &Bound<'_, PyTuple>,
) -> Option<PyResult<Py<PyAny>>> {
    if let Some(t) = as_mut::<w::DataTable>(wd) {
        return table_call(py, t, ctx, method, args);
    }
    if let Some(t) = as_mut::<PyTree>(wd) {
        return tree_call(py, t, ctx, method, args);
    }
    if let Some(d) = as_mut::<w::DirectoryTree>(wd) {
        return dir_tree_call(py, d, ctx, method, args);
    }
    if let Some(l) = as_mut::<w::Log>(wd) {
        return log_call(py, l, ctx, method, args);
    }
    if let Some(l) = as_mut::<w::RichLog>(wd) {
        return rich_log_call(py, l, ctx, method, args);
    }
    if let Some(p) = as_mut::<w::ProgressBar>(wd) {
        return match method {
            "set_total" => Some((|| {
                p.set_total(arg(args, 0)?);
                none(py)
            })()),
            "set_progress" => Some((|| {
                p.set_progress(arg(args, 0)?);
                none(py)
            })()),
            "advance" => Some((|| {
                p.advance(arg(args, 0)?);
                none(py)
            })()),
            _ => None,
        };
    }
    if let Some(s) = as_mut::<w::Sparkline>(wd) {
        return match method {
            "set_data" => Some((|| {
                s.set_data(arg::<Vec<f64>>(args, 0)?);
                none(py)
            })()),
            _ => None,
        };
    }
    if let Some(d) = as_mut::<w::Digits>(wd) {
        return match method {
            "update" => Some((|| {
                d.update(arg::<String>(args, 0)?);
                none(py)
            })()),
            _ => None,
        };
    }
    if let Some(p) = as_mut::<w::Pretty>(wd) {
        return match method {
            "update" => Some((|| {
                *p = w::Pretty::from_reprs(arg::<String>(args, 0)?, arg::<String>(args, 1)?);
                none(py)
            })()),
            _ => None,
        };
    }
    if let Some(l) = as_mut::<w::ListView>(wd) {
        return list_view_call(py, l, ctx, method, args);
    }
    None
}

// ---------------------------------------------------------------------------- DataTable

fn cursor_type(s: &str) -> PyResult<CursorType> {
    Ok(match s {
        "cell" => CursorType::Cell,
        "row" => CursorType::Row,
        "column" => CursorType::Column,
        "none" => CursorType::None,
        _ => return Err(PyValueError::new_err(format!("invalid cursor type {s:?}"))),
    })
}

fn cursor_type_name(c: CursorType) -> &'static str {
    match c {
        CursorType::Cell => "cell",
        CursorType::Row => "row",
        CursorType::Column => "column",
        CursorType::None => "none",
    }
}

/// Python cell value → table cell (as Textual's `default_cell_formatter`: strings are markup,
/// floats show two decimals, rich `Text` / `Content` keep their styles and justification,
/// anything else is shown with `str()`).
fn cell_from_py(v: &Bound<'_, PyAny>) -> PyResult<CellValue> {
    if v.is_none() {
        return Ok(CellValue::Empty);
    }
    if v.is_instance_of::<PyBool>() {
        return Ok(CellValue::Text(Text::plain(v.str()?.to_string())));
    }
    if v.is_instance_of::<PyInt>() {
        return Ok(match v.extract::<i64>() {
            Ok(i) => CellValue::Int(i),
            Err(_) => CellValue::Text(Text::plain(v.str()?.to_string())),
        });
    }
    if v.is_instance_of::<PyFloat>() {
        return Ok(CellValue::Float(v.extract()?));
    }
    if let Ok(s) = v.cast::<PyString>() {
        return Ok(CellValue::Markup(s.to_string()));
    }
    let markup = match v.getattr("__rs_markup__") {
        Ok(m) if m.is_instance_of::<PyString>() => Some(m.extract::<String>()?),
        _ => match (v.getattr("markup"), v.hasattr("plain")?) {
            (Ok(m), true) if m.is_instance_of::<PyString>() => Some(m.extract::<String>()?),
            _ => None,
        },
    };
    let Some(markup) = markup else {
        return Ok(CellValue::Text(Text::plain(v.str()?.to_string())));
    };
    let value = CellValue::Markup(markup);
    let justify = v
        .getattr("justify")
        .ok()
        .and_then(|j| j.extract::<String>().ok());
    Ok(match justify.as_deref() {
        Some("right") => value.justify(Justify::Right),
        Some("center") => value.justify(Justify::Center),
        _ => value,
    })
}

fn cell_to_py(py: Python<'_>, v: &CellValue) -> PyResult<Py<PyAny>> {
    match v {
        CellValue::Empty => none(py),
        CellValue::Markup(m) => to_py(py, m),
        CellValue::Text(t) => to_py(py, t.plain_text()),
        CellValue::Int(i) => to_py(py, *i),
        CellValue::Float(f) => to_py(py, *f),
        CellValue::Aligned(inner, _) => cell_to_py(py, inner),
    }
}

fn cells_to_py<'a>(
    py: Python<'_>,
    cells: impl IntoIterator<Item = &'a CellValue>,
) -> PyResult<Py<PyAny>> {
    let items = cells
        .into_iter()
        .map(|c| cell_to_py(py, c))
        .collect::<PyResult<Vec<_>>>()?;
    Ok(PyList::new(py, items)?.into_any().unbind())
}

/// `DataTableError` → the matching Python exception (`rustual.widgets.data_table`).
fn table_err(py: Python<'_>, e: DataTableError) -> PyErr {
    let key = match &e {
        DataTableError::DuplicateKey(_) => "DataTable.DuplicateKey",
        DataTableError::RowDoesNotExist(_) => "DataTable.RowDoesNotExist",
        DataTableError::ColumnDoesNotExist(_) => "DataTable.ColumnDoesNotExist",
        DataTableError::CellDoesNotExist(_) => "DataTable.CellDoesNotExist",
        DataTableError::TooManyCells => return PyValueError::new_err(e.to_string()),
    };
    let d = PyDict::new(py);
    if let Err(err) = d.set_item("message", e.to_string()) {
        return err;
    }
    match new_py(py, key, &d) {
        Ok(exc) => PyErr::from_value(exc),
        Err(err) => err,
    }
}

fn coordinate(args: &Bound<'_, PyTuple>, i: usize) -> PyResult<Coordinate> {
    let (row, column): (usize, usize) = arg(args, i)?;
    Ok(Coordinate::new(row, column))
}

fn cell_message<'py>(
    py: Python<'py>,
    key: &str,
    value: &CellValue,
    c: Coordinate,
    cell_key: &dt::CellKey,
) -> PyResult<Bound<'py, PyAny>> {
    let d = PyDict::new(py);
    d.set_item("value", cell_to_py(py, value)?)?;
    d.set_item("coordinate", (c.row, c.column))?;
    d.set_item("cell_key", (&cell_key.row_key.0, &cell_key.column_key.0))?;
    new_py(py, key, &d)
}

fn table_get(py: Python<'_>, t: &w::DataTable, attr: &str) -> Option<PyResult<Py<PyAny>>> {
    Some(match attr {
        "row_count" => to_py(py, t.row_count()),
        "column_count" => to_py(py, t.column_count()),
        "cursor_coordinate" => {
            let c = t.cursor_coordinate();
            to_py(py, (c.row, c.column))
        }
        "hover_coordinate" => to_py(py, t.hover_coordinate()),
        "cursor_type" => to_py(py, cursor_type_name(t.get_cursor_type())),
        "show_cursor" => to_py(py, t.get_show_cursor()),
        "zebra_stripes" => to_py(py, t.get_zebra_stripes()),
        "show_header" => to_py(py, t.get_show_header()),
        "show_row_labels" => to_py(py, t.get_show_row_labels()),
        "fixed_rows" => to_py(py, t.get_fixed_rows()),
        "fixed_columns" => to_py(py, t.get_fixed_columns()),
        "header_height" => to_py(py, t.get_header_height()),
        "cell_padding" => to_py(py, t.get_cell_padding()),
        "row_keys" => to_py(
            py,
            t.row_keys().into_iter().map(|k| k.0).collect::<Vec<_>>(),
        ),
        // (key, label, width, content_width, auto_width) in display order
        "columns" => to_py(
            py,
            t.ordered_columns()
                .iter()
                .map(|c| {
                    (
                        c.key.0.clone(),
                        c.label.plain_text(),
                        c.width,
                        c.content_width,
                        c.auto_width,
                    )
                })
                .collect::<Vec<_>>(),
        ),
        _ => return None,
    })
}

fn table_call(
    py: Python<'_>,
    t: &mut w::DataTable,
    ctx: Option<&mut Ctx>,
    method: &str,
    args: &Bound<'_, PyTuple>,
) -> Option<PyResult<Py<PyAny>>> {
    let e = |err| table_err(py, err);
    // Builder-only options of a detached table.
    let rebuild = |t: &mut w::DataTable, f: &dyn Fn(w::DataTable) -> w::DataTable| {
        *t = f(std::mem::take(t));
    };
    Some((|| match method {
        // add_column(label, width, key, default) -> key
        "add_column" => {
            let label: String = arg(args, 0)?;
            let width: Option<i32> = arg(args, 1)?;
            let key: Option<String> = arg(args, 2)?;
            let default = cell_from_py(&args.get_item(3)?)?;
            let k = t
                .add_column_text(Text::from_markup(&label), width, key.as_deref(), default)
                .map_err(e)?;
            to_py(py, k.0)
        }
        // add_row(cells, height, key, label) -> key
        "add_row" => {
            let cells = args
                .get_item(0)?
                .try_iter()?
                .map(|c| cell_from_py(&c?))
                .collect::<PyResult<Vec<_>>>()?;
            let height: Option<i32> = arg(args, 1)?;
            let key: Option<String> = arg(args, 2)?;
            let label: Option<String> = arg(args, 3)?;
            let k = t
                .add_row_with(cells, height, key.as_deref(), label.as_deref())
                .map_err(e)?;
            to_py(py, k.0)
        }
        "update_cell" => {
            let (r, c): (String, String) = (arg(args, 0)?, arg(args, 1)?);
            let value = cell_from_py(&args.get_item(2)?)?;
            t.update_cell(r, c, value, arg(args, 3)?).map_err(e)?;
            none(py)
        }
        "update_cell_at" => {
            let c = coordinate(args, 0)?;
            let value = cell_from_py(&args.get_item(1)?)?;
            t.update_cell_at(c, value, arg(args, 2)?).map_err(e)?;
            none(py)
        }
        "get_cell" => {
            let v = t
                .get_cell(arg::<String>(args, 0)?, arg::<String>(args, 1)?)
                .map_err(e)?;
            cell_to_py(py, v)
        }
        "get_cell_at" => cell_to_py(py, t.get_cell_at(coordinate(args, 0)?).map_err(e)?),
        "get_row" => cells_to_py(py, t.get_row(arg::<String>(args, 0)?).map_err(e)?),
        "get_row_at" => cells_to_py(py, t.get_row_at(arg(args, 0)?).map_err(e)?),
        "get_column" => cells_to_py(py, t.get_column(arg::<String>(args, 0)?).map_err(e)?),
        "get_column_at" => cells_to_py(py, t.get_column_at(arg(args, 0)?).map_err(e)?),
        "get_row_index" => to_py(py, t.get_row_index(arg::<String>(args, 0)?).map_err(e)?),
        "get_column_index" => to_py(py, t.get_column_index(arg::<String>(args, 0)?).map_err(e)?),
        "get_row_height" => to_py(py, t.get_row_height(arg::<String>(args, 0)?).map_err(e)?),
        "get_cell_coordinate" => {
            let c = t
                .get_cell_coordinate(arg::<String>(args, 0)?, arg::<String>(args, 1)?)
                .map_err(e)?;
            to_py(py, (c.row, c.column))
        }
        "coordinate_to_cell_key" => {
            let k = t.coordinate_to_cell_key(coordinate(args, 0)?).map_err(e)?;
            to_py(py, (k.row_key.0, k.column_key.0))
        }
        "remove_row" => {
            t.remove_row(arg::<String>(args, 0)?).map_err(e)?;
            none(py)
        }
        "remove_column" => {
            t.remove_column(arg::<String>(args, 0)?).map_err(e)?;
            none(py)
        }
        "clear" => {
            t.clear(arg(args, 0)?);
            none(py)
        }
        // sort(columns, key, reverse)
        "sort" => {
            let columns: Vec<String> = arg(args, 0)?;
            let key = args.get_item(1)?;
            let reverse: bool = arg(args, 2)?;
            if key.is_none() {
                let cols: Vec<&str> = columns.iter().map(String::as_str).collect();
                t.sort(&cols, reverse).map_err(e)?;
            } else {
                sort_with_key(py, t, &columns, &key, reverse)?;
            }
            none(py)
        }
        "set_cursor_type" => {
            let c = cursor_type(&arg::<String>(args, 0)?)?;
            match ctx {
                Some(ctx) => t.set_cursor_type(c, ctx),
                None => rebuild(t, &|t| t.cursor_type(c)),
            }
            none(py)
        }
        "set_show_cursor" => {
            let v: bool = arg(args, 0)?;
            match ctx {
                Some(ctx) => t.set_show_cursor(v, ctx),
                None => rebuild(t, &|t| t.show_cursor(v)),
            }
            none(py)
        }
        "set_header_height" => {
            let v: i32 = arg(args, 0)?;
            rebuild(t, &|t| t.header_height(v));
            none(py)
        }
        "set_zebra_stripes" => {
            t.set_zebra_stripes(arg(args, 0)?);
            none(py)
        }
        "set_show_header" => {
            t.set_show_header(arg(args, 0)?);
            none(py)
        }
        "set_show_row_labels" => {
            t.set_show_row_labels(arg(args, 0)?);
            none(py)
        }
        "set_fixed_rows" => {
            t.set_fixed_rows(arg(args, 0)?);
            none(py)
        }
        "set_fixed_columns" => {
            t.set_fixed_columns(arg(args, 0)?);
            none(py)
        }
        "set_cell_padding" => {
            t.set_cell_padding(arg(args, 0)?);
            none(py)
        }
        // Cursor movement needs a mounted table (it posts messages and scrolls).
        "set_cursor_coordinate" => {
            let c = coordinate(args, 0)?;
            if let Some(ctx) = ctx {
                t.set_cursor_coordinate(c, ctx);
            }
            none(py)
        }
        "move_cursor" => {
            let row: Option<usize> = arg(args, 0)?;
            let column: Option<usize> = arg(args, 1)?;
            if let Some(ctx) = ctx {
                t.move_cursor(row, column, ctx);
            }
            none(py)
        }
        _ => Err(PyAttributeError::new_err("")),
    })())
    .filter(|r| !is_unknown(py, r))
}

/// Sort with a Python key function (as Textual: the key receives the values of `columns` —
/// a single value for one column — or a tuple of all values).
fn sort_with_key(
    py: Python<'_>,
    t: &mut w::DataTable,
    columns: &[String],
    key: &Bound<'_, PyAny>,
    reverse: bool,
) -> PyResult<()> {
    let idx: Vec<usize> = columns
        .iter()
        .map(|c| t.get_column_index(c.as_str()))
        .collect::<Result<_, _>>()
        .map_err(|e| table_err(py, e))?;
    // Rows are identified by the (stable) address of their cell buffer while sorting.
    let mut keys: HashMap<usize, Py<PyAny>> = HashMap::new();
    for row in 0..t.row_count() {
        let cells = t.get_row_at(row).map_err(|e| table_err(py, e))?;
        let values: Vec<Py<PyAny>> = if idx.is_empty() {
            cells
                .iter()
                .map(|c| cell_to_py(py, c))
                .collect::<PyResult<_>>()?
        } else {
            idx.iter()
                .map(|i| cell_to_py(py, &cells[*i]))
                .collect::<PyResult<_>>()?
        };
        let value = if values.len() == 1 && !idx.is_empty() {
            values.into_iter().next().expect("one value")
        } else {
            PyTuple::new(py, values)?.into_any().unbind()
        };
        keys.insert(cells.as_ptr() as usize, key.call1((value,))?.unbind());
    }
    let mut error: Option<PyErr> = None;
    let mut compare = |a: &[CellValue], b: &[CellValue]| -> Ordering {
        let (Some(ka), Some(kb)) = (
            keys.get(&(a.as_ptr() as usize)),
            keys.get(&(b.as_ptr() as usize)),
        ) else {
            return Ordering::Equal;
        };
        let (ka, kb) = (ka.bind(py), kb.bind(py));
        let lt = |x: &Bound<'_, PyAny>, y: &Bound<'_, PyAny>| x.lt(y);
        match (lt(ka, kb), lt(kb, ka)) {
            (Ok(true), _) => Ordering::Less,
            (_, Ok(true)) => Ordering::Greater,
            (Ok(false), Ok(false)) => Ordering::Equal,
            (Err(err), _) | (_, Err(err)) => {
                error.get_or_insert(err);
                Ordering::Equal
            }
        }
    };
    if reverse {
        t.sort_by(|a, b| compare(b, a));
    } else {
        t.sort_by(|a, b| compare(a, b));
    }
    match error {
        Some(err) => Err(err),
        None => Ok(()),
    }
}

// ---------------------------------------------------------------------------- Tree

fn node_message<'py>(
    py: Python<'py>,
    key: &str,
    node: tree::TreeNodeId,
) -> PyResult<Bound<'py, PyAny>> {
    let d = PyDict::new(py);
    d.set_item("node_id", node.0)?;
    new_py(py, key, &d)
}

fn path_message<'py>(
    py: Python<'py>,
    key: &str,
    node: tree::TreeNodeId,
    path: &std::path::Path,
) -> PyResult<Bound<'py, PyAny>> {
    let d = PyDict::new(py);
    d.set_item("node_id", node.0)?;
    d.set_item("path", path.to_string_lossy().into_owned())?;
    new_py(py, key, &d)
}

fn tree_get<T: 'static>(py: Python<'_>, t: &w::Tree<T>, attr: &str) -> Option<PyResult<Py<PyAny>>> {
    Some(match attr {
        "root" => to_py(py, t.root().0),
        "cursor_line" => to_py(py, t.cursor_line()),
        "cursor_node" => to_py(py, t.cursor_node().map(|n| n.0)),
        "last_line" => to_py(py, t.last_line()),
        "line_count" => to_py(py, t.line_count()),
        "show_root" => to_py(py, t.is_show_root()),
        "show_guides" => to_py(py, t.is_show_guides()),
        "guide_depth" => to_py(py, t.get_guide_depth()),
        "auto_expand" => to_py(py, t.is_auto_expand()),
        _ => return None,
    })
}

/// Ids of `id` and all its descendants.
fn subtree<T: 'static>(t: &w::Tree<T>, id: tree::TreeNodeId) -> Vec<usize> {
    let mut out = Vec::new();
    let mut stack = vec![id];
    while let Some(n) = stack.pop() {
        if let Some(node) = t.node(n) {
            out.push(n.0);
            stack.extend(node.children().iter().copied());
        }
    }
    out
}

/// Expand / collapse without posting messages (detached trees).
fn set_expanded_rec<T: 'static>(
    t: &mut w::Tree<T>,
    id: tree::TreeNodeId,
    expanded: bool,
    all: bool,
) {
    t.set_expanded(id, expanded);
    if all {
        let children = t
            .node(id)
            .map(|n| n.children().to_vec())
            .unwrap_or_default();
        for c in children {
            set_expanded_rec(t, c, expanded, true);
        }
    }
}

fn tree_call<T: 'static>(
    py: Python<'_>,
    t: &mut w::Tree<T>,
    ctx: Option<&mut Ctx>,
    method: &str,
    args: &Bound<'_, PyTuple>,
) -> Option<PyResult<Py<PyAny>>> {
    let id = |i: usize| -> PyResult<tree::TreeNodeId> { Ok(tree::TreeNodeId(arg(args, i)?)) };
    let opt_id = |i: usize| -> PyResult<Option<tree::TreeNodeId>> {
        Ok(arg::<Option<usize>>(args, i)?.map(tree::TreeNodeId))
    };
    Some((|| match method {
        // node_info(id) -> (parent, children, label markup, label plain, expanded, allow_expand, line)
        "node_info" => match t.node(id(0)?) {
            Some(n) => to_py(
                py,
                (
                    n.parent().map(|p| p.0),
                    n.children().iter().map(|c| c.0).collect::<Vec<_>>(),
                    n.label().to_string(),
                    n.label_text(),
                    n.is_expanded(),
                    n.allow_expand(),
                    n.line(),
                ),
            ),
            None => none(py),
        },
        // add(parent, index, label, expand, allow_expand) -> id
        "add" => {
            let parent = id(0)?;
            if t.node(parent).is_none() {
                return Err(PyValueError::new_err(format!(
                    "unknown tree node {}",
                    parent.0
                )));
            }
            let index: Option<usize> = arg(args, 1)?;
            let label: String = arg(args, 2)?;
            let node = t.insert(parent, index, label, None, arg(args, 3)?, arg(args, 4)?);
            to_py(py, node.0)
        }
        "remove" => {
            let node = id(0)?;
            let removed = subtree(t, node);
            t.remove(node);
            to_py(py, removed)
        }
        "remove_children" => {
            let node = id(0)?;
            let mut removed = subtree(t, node);
            removed.retain(|n| *n != node.0);
            t.remove_children(node);
            to_py(py, removed)
        }
        "set_label" => {
            t.set_label(id(0)?, arg::<String>(args, 1)?);
            none(py)
        }
        "set_allow_expand" => {
            t.set_allow_expand(id(0)?, arg(args, 1)?);
            none(py)
        }
        // expand / collapse / toggle(id, all)
        "expand" | "collapse" | "toggle" => {
            let node = id(0)?;
            let all: bool = arg(args, 1)?;
            match (method, ctx, all) {
                ("expand", Some(ctx), false) => t.expand(node, ctx),
                ("expand", Some(ctx), true) => t.expand_all(node, ctx),
                ("collapse", Some(ctx), false) => t.collapse(node, ctx),
                ("collapse", Some(ctx), true) => t.collapse_all(node, ctx),
                ("toggle", Some(ctx), false) => t.toggle(node, ctx),
                ("toggle", Some(ctx), true) => t.toggle_all(node, ctx),
                (_, None, all) => {
                    let expanded = match method {
                        "expand" => true,
                        "collapse" => false,
                        _ => !t.node(node).is_some_and(|n| n.is_expanded()),
                    };
                    set_expanded_rec(t, node, expanded, all);
                }
                _ => unreachable!(),
            }
            none(py)
        }
        "clear" => {
            t.clear();
            to_py(py, t.root().0)
        }
        "reset" => {
            t.reset(arg::<String>(args, 0)?, None);
            to_py(py, t.root().0)
        }
        "set_cursor_line" => {
            if let Some(ctx) = ctx {
                t.set_cursor_line(arg(args, 0)?, ctx);
            }
            none(py)
        }
        "move_cursor" => {
            if let Some(ctx) = ctx {
                t.move_cursor(opt_id(0)?, ctx);
            }
            none(py)
        }
        "select_node" => {
            if let Some(ctx) = ctx {
                t.select_node(opt_id(0)?, ctx);
            }
            none(py)
        }
        "unselect" => {
            t.unselect();
            none(py)
        }
        "scroll_to_node" => {
            if let Some(ctx) = ctx {
                t.scroll_to_node(id(0)?, ctx);
            }
            none(py)
        }
        "scroll_to_line" => {
            if let Some(ctx) = ctx {
                t.scroll_to_line(arg(args, 0)?, ctx);
            }
            none(py)
        }
        "node_at_line" => to_py(py, t.node_at_line(arg(args, 0)?).map(|n| n.0)),
        "set_show_root" => {
            t.set_show_root(arg(args, 0)?);
            none(py)
        }
        "set_show_guides" => {
            t.set_show_guides(arg(args, 0)?);
            none(py)
        }
        "set_guide_depth" => {
            t.set_guide_depth(arg(args, 0)?);
            none(py)
        }
        "set_auto_expand" => {
            t.set_auto_expand(arg(args, 0)?);
            none(py)
        }
        _ => Err(PyAttributeError::new_err("")),
    })())
    .filter(|r| !is_unknown(py, r))
}

fn dir_tree_call(
    py: Python<'_>,
    d: &mut w::DirectoryTree,
    ctx: Option<&mut Ctx>,
    method: &str,
    args: &Bound<'_, PyTuple>,
) -> Option<PyResult<Py<PyAny>>> {
    match method {
        // node_path(id) -> (path, loaded) | None
        "node_path" => Some((|| {
            let id = tree::TreeNodeId(arg(args, 0)?);
            match d.tree().data(id) {
                Some(e) => to_py(py, (e.path.to_string_lossy().into_owned(), e.loaded)),
                None => none(py),
            }
        })()),
        "set_path" => Some((|| {
            let path: PathBuf = arg(args, 0)?;
            match ctx {
                Some(ctx) => d.set_path(path, ctx),
                None => *d = w::DirectoryTree::new(path),
            }
            none(py)
        })()),
        "reload" => Some({
            if let Some(ctx) = ctx {
                d.reload(ctx);
            }
            none(py)
        }),
        "reload_node" => Some((|| {
            let id = tree::TreeNodeId(arg(args, 0)?);
            if let Some(ctx) = ctx {
                d.reload_node(id, ctx);
            }
            none(py)
        })()),
        _ => tree_call(py, d.tree_mut(), ctx, method, args),
    }
}

/// The marker error used by the `match` blocks above for "not my method".
fn is_unknown(py: Python<'_>, r: &PyResult<Py<PyAny>>) -> bool {
    matches!(r, Err(err) if err.is_instance_of::<PyAttributeError>(py)
        && err.value(py).to_string().is_empty())
}

// ---------------------------------------------------------------------------- logs

fn log_lines(l: &w::Log) -> Vec<String> {
    l.lines().take(l.line_count()).map(String::from).collect()
}

fn log_call(
    py: Python<'_>,
    l: &mut w::Log,
    ctx: Option<&mut Ctx>,
    method: &str,
    args: &Bound<'_, PyTuple>,
) -> Option<PyResult<Py<PyAny>>> {
    Some((|| match method {
        // write(data, scroll_end)
        "write" => {
            let data: String = arg(args, 0)?;
            match ctx {
                Some(ctx) => l.write_with(&data, arg(args, 1)?, ctx),
                None => l.push(&data),
            }
            none(py)
        }
        // write_lines(lines, scroll_end)
        "write_lines" => {
            let lines: Vec<String> = arg(args, 0)?;
            let lines = lines.iter().map(String::as_str);
            match ctx {
                Some(ctx) => l.write_lines_with(lines, arg(args, 1)?, ctx),
                None => l.push_lines(lines),
            }
            none(py)
        }
        "clear" => {
            match ctx {
                Some(ctx) => l.clear(ctx),
                None => {
                    *l = w::Log::new()
                        .max_lines(l.get_max_lines())
                        .auto_scroll(l.get_auto_scroll())
                        .highlight(l.is_highlight())
                }
            }
            none(py)
        }
        "set_max_lines" => {
            l.set_max_lines(arg(args, 0)?);
            none(py)
        }
        "set_auto_scroll" => {
            l.set_auto_scroll(arg(args, 0)?);
            none(py)
        }
        "set_highlight" => {
            l.set_highlight(arg(args, 0)?);
            none(py)
        }
        _ => Err(PyAttributeError::new_err("")),
    })())
    .filter(|r| !is_unknown(py, r))
}

fn rich_log_call(
    py: Python<'_>,
    l: &mut w::RichLog,
    ctx: Option<&mut Ctx>,
    method: &str,
    args: &Bound<'_, PyTuple>,
) -> Option<PyResult<Py<PyAny>>> {
    Some((|| match method {
        // write(content, mode, scroll_end): mode "auto" (a str: markup if enabled,
        // highlighted if enabled), "text" (styled markup, not highlighted) or "plain"
        // (plain text, highlighted if enabled).
        "write" => {
            let content: String = arg(args, 0)?;
            let mode: String = arg(args, 1)?;
            let scroll_end: Option<bool> = arg(args, 2)?;
            let markup = mode == "text" || (mode == "auto" && l.is_markup());
            let mut text = if markup {
                Text::from_markup(&content)
            } else {
                Text::plain(content)
            };
            if mode != "text" && l.is_highlighting() {
                let dark = ctx.as_ref().is_none_or(|c| c.is_dark());
                text = pretty::highlight_text(&text, &|t| pretty::repr_style(t, dark));
            }
            match ctx {
                Some(ctx) => l.write_text_with(text, scroll_end, ctx),
                None => l.push_text(text),
            }
            none(py)
        }
        // write_rich(renderable, width, scroll_end): a Rich renderable rendered through Rich.
        "write_rich" => {
            let renderable = args.get_item(0)?;
            let width: Option<i32> = arg(args, 1)?;
            let scroll_end: Option<bool> = arg(args, 2)?;
            let width = width.unwrap_or_else(|| {
                let w = ctx.as_ref().map_or(0, |c| c.size().width);
                w.max(l.get_min_width())
            });
            let mut text = Text::new();
            for (i, strip) in super::rich::rich_lines(py, &renderable, width, None)?
                .into_iter()
                .enumerate()
            {
                if i > 0 {
                    text.push("\n", Default::default());
                }
                for span in strip.spans {
                    text.push(span.text, span.style);
                }
            }
            match ctx {
                Some(ctx) => l.write_text_with(text, scroll_end, ctx),
                None => l.push_text(text),
            }
            none(py)
        }
        "clear" => {
            match ctx {
                Some(ctx) => l.clear(ctx),
                None => {
                    *l = w::RichLog::new()
                        .max_lines(l.get_max_lines())
                        .min_width(l.get_min_width())
                        .wrap(l.get_wrap())
                        .markup(l.is_markup())
                        .highlight(l.is_highlighting())
                        .auto_scroll(l.get_auto_scroll())
                }
            }
            none(py)
        }
        "set_max_lines" => {
            l.set_max_lines(arg(args, 0)?);
            none(py)
        }
        "set_min_width" => {
            l.set_min_width(arg(args, 0)?);
            none(py)
        }
        "set_wrap" => {
            l.set_wrap(arg(args, 0)?);
            none(py)
        }
        "set_markup" => {
            l.set_markup(arg(args, 0)?);
            none(py)
        }
        "set_highlight" => {
            l.set_highlight(arg(args, 0)?);
            none(py)
        }
        "set_auto_scroll" => {
            l.set_auto_scroll(arg(args, 0)?);
            none(py)
        }
        _ => Err(PyAttributeError::new_err("")),
    })())
    .filter(|r| !is_unknown(py, r))
}

// ---------------------------------------------------------------------------- ListView

fn list_view_call(
    py: Python<'_>,
    l: &mut w::ListView,
    ctx: Option<&mut Ctx>,
    method: &str,
    args: &Bound<'_, PyTuple>,
) -> Option<PyResult<Py<PyAny>>> {
    // ListView methods work on its children, so they need a mounted list.
    let Some(ctx) = ctx else {
        return Some(Err(PyRuntimeError::new_err("the ListView is not mounted")));
    };
    let ints = |nodes: Vec<rustual::NodeId>| -> Vec<u64> {
        nodes.into_iter().map(crate::bridge::node_to_int).collect()
    };
    Some((|| match method {
        "set_index" => {
            let index: Option<i64> = arg(args, 0)?;
            l.set_index(index.map(|i| i.max(0) as usize), ctx);
            none(py)
        }
        "clear" => {
            l.clear(ctx);
            none(py)
        }
        "pop" => {
            let node = l.pop(arg(args, 0)?, ctx);
            to_py(py, node.map(crate::bridge::node_to_int))
        }
        "remove_items" => {
            let indices: Vec<isize> = arg(args, 0)?;
            to_py(py, ints(l.remove_items(&indices, ctx)))
        }
        "action_cursor_up" => {
            l.action_cursor_up(ctx);
            none(py)
        }
        "action_cursor_down" => {
            l.action_cursor_down(ctx);
            none(py)
        }
        "action_select_cursor" => {
            l.action_select_cursor(ctx);
            none(py)
        }
        _ => Err(PyAttributeError::new_err("")),
    })())
    .filter(|r| !is_unknown(py, r))
}

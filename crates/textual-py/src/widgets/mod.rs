//! Built-in widget bindings: constructors, attribute access, methods and message conversion.
//!
//! Each widget module has a `register(&mut Registry)` function that adds:
//! - a *kind* constructor (`"Button"` → `Box<dyn Widget>`) used when a Python widget mounts,
//! - getter / setter / caller functions, each returning `None` when the widget isn't theirs,
//! - message converters (Rust message payload type → Python message object).
//!
//! See `docs/python-bindings.md` for how to add a widget.

use pyo3::exceptions::{PyAttributeError, PyKeyError, PyTypeError};
use pyo3::prelude::*;
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyDict, PyTuple};
use pyo3::BoundObject;
use std::any::TypeId;
use std::collections::HashMap;
use std::sync::OnceLock;
use textual::{Ctx, Message, Widget};

mod basic;
mod data;
pub(crate) mod rich;

/// Builds a base widget from the Python constructor kwargs. May consume `children` (e.g.
/// tab panes) — whatever is left is mounted as ordinary children.
pub(crate) type CreateFn =
    fn(Python<'_>, &Bound<'_, PyDict>, &mut Vec<Bound<'_, PyAny>>) -> PyResult<Box<dyn Widget>>;
pub(crate) type GetFn = fn(Python<'_>, &dyn Widget, &str) -> Option<PyResult<Py<PyAny>>>;
pub(crate) type SetFn =
    fn(Python<'_>, &mut dyn Widget, &mut Ctx, &str, &Bound<'_, PyAny>) -> Option<PyResult<()>>;
pub(crate) type CallFn = fn(
    Python<'_>,
    &mut dyn Widget,
    &mut Ctx,
    &str,
    &Bound<'_, PyTuple>,
) -> Option<PyResult<Py<PyAny>>>;
pub(crate) type BuildFn = for<'py> fn(Python<'py>, &Message) -> PyResult<Bound<'py, PyAny>>;

#[derive(Default)]
pub(crate) struct Registry {
    kinds: HashMap<&'static str, CreateFn>,
    getters: Vec<GetFn>,
    setters: Vec<SetFn>,
    callers: Vec<CallFn>,
    messages: HashMap<TypeId, MessageConv>,
}

#[derive(Clone, Copy)]
pub(crate) struct MessageConv {
    /// Python handler name, e.g. `"on_button_pressed"`.
    pub handler: &'static str,
    pub build: BuildFn,
}

impl Registry {
    pub fn kind(&mut self, name: &'static str, f: CreateFn) -> &mut Self {
        self.kinds.insert(name, f);
        self
    }
    pub fn getter(&mut self, f: GetFn) -> &mut Self {
        self.getters.push(f);
        self
    }
    pub fn setter(&mut self, f: SetFn) -> &mut Self {
        self.setters.push(f);
        self
    }
    pub fn caller(&mut self, f: CallFn) -> &mut Self {
        self.callers.push(f);
        self
    }
    pub fn message<T: 'static>(&mut self, handler: &'static str, build: BuildFn) -> &mut Self {
        self.messages
            .insert(TypeId::of::<T>(), MessageConv { handler, build });
        self
    }
}

fn registry() -> &'static Registry {
    static R: OnceLock<Registry> = OnceLock::new();
    R.get_or_init(|| {
        let mut r = Registry::default();
        basic::register(&mut r);
        data::register(&mut r);
        r
    })
}

/// Register module-level functions and classes of the widget groups.
pub(crate) fn add_to_module(m: &Bound<'_, PyModule>) -> PyResult<()> {
    data::add_to_module(m)
}

/// A widget with no behaviour of its own (base of pure-Python widgets and of the app).
pub(crate) struct PlainWidget;

impl Widget for PlainWidget {}

pub(crate) fn create(
    py: Python<'_>,
    kind: &str,
    kwargs: &Bound<'_, PyDict>,
    children: &mut Vec<Bound<'_, PyAny>>,
) -> PyResult<Box<dyn Widget>> {
    if kind == "Widget" {
        return Ok(Box::new(PlainWidget));
    }
    match registry().kinds.get(kind) {
        Some(f) => f(py, kwargs, children),
        None => Err(PyKeyError::new_err(format!(
            "unknown native widget kind {kind:?}"
        ))),
    }
}

pub(crate) fn get(py: Python<'_>, w: &dyn Widget, attr: &str) -> PyResult<Py<PyAny>> {
    for g in &registry().getters {
        if let Some(r) = g(py, w, attr) {
            return r;
        }
    }
    Err(PyAttributeError::new_err(format!(
        "no native attribute {attr:?}"
    )))
}

pub(crate) fn set(
    py: Python<'_>,
    w: &mut dyn Widget,
    ctx: &mut Ctx,
    attr: &str,
    value: &Bound<'_, PyAny>,
) -> PyResult<()> {
    for s in &registry().setters {
        if let Some(r) = s(py, w, ctx, attr, value) {
            return r;
        }
    }
    Err(PyAttributeError::new_err(format!(
        "can't set native attribute {attr:?}"
    )))
}

pub(crate) fn call(
    py: Python<'_>,
    w: &mut dyn Widget,
    ctx: &mut Ctx,
    method: &str,
    args: &Bound<'_, PyTuple>,
) -> PyResult<Py<PyAny>> {
    for c in &registry().callers {
        if let Some(r) = c(py, w, ctx, method, args) {
            return r;
        }
    }
    Err(PyAttributeError::new_err(format!(
        "no native method {method:?}"
    )))
}

pub(crate) fn convert_message(msg: &Message) -> Option<MessageConv> {
    registry().messages.get(&msg.payload_type_id()).copied()
}

// ---------------------------------------------------------------------------- Python classes

static CLASSES: PyOnceLock<Py<PyDict>> = PyOnceLock::new();

fn classes(py: Python<'_>) -> &Bound<'_, PyDict> {
    CLASSES
        .get_or_init(py, || PyDict::new(py).unbind())
        .bind(py)
}

/// Register a Python class under a key such as `"Button.Pressed"` (called at import time).
#[pyfunction]
pub fn register_class(py: Python<'_>, key: &str, cls: Py<PyAny>) -> PyResult<()> {
    classes(py).set_item(key, cls)
}

/// Instantiate a registered Python class with keyword arguments.
pub(crate) fn new_py<'py>(
    py: Python<'py>,
    key: &str,
    kwargs: &Bound<'py, PyDict>,
) -> PyResult<Bound<'py, PyAny>> {
    let cls = classes(py)
        .get_item(key)?
        .ok_or_else(|| PyKeyError::new_err(format!("Python class {key:?} not registered")))?;
    cls.call((), Some(kwargs))
}

// ---------------------------------------------------------------------------- helpers

/// Keyword argument `k` (absent or `None` → `None`).
pub(crate) fn kw<'py, T: FromPyObjectOwned<'py>>(
    kwargs: &Bound<'py, PyDict>,
    k: &str,
) -> PyResult<Option<T>> {
    match kwargs.get_item(k)? {
        Some(v) if !v.is_none() => Ok(Some(v.extract::<T>().map_err(|e| {
            let e: PyErr = e.into();
            PyTypeError::new_err(format!("argument {k:?}: {e}"))
        })?)),
        _ => Ok(None),
    }
}

/// Downcast helpers for getter/setter/caller functions (they look through wrappers such as
/// [`rich::RichHost`]).
pub(crate) fn as_ref<W: Widget>(w: &dyn Widget) -> Option<&W> {
    textual::widget::downcast_ref::<W>(w)
}

pub(crate) fn as_mut<W: Widget>(w: &mut dyn Widget) -> Option<&mut W> {
    textual::widget::downcast_mut::<W>(w)
}

pub(crate) fn to_py<'py, T: IntoPyObject<'py>>(py: Python<'py>, v: T) -> PyResult<Py<PyAny>> {
    v.into_pyobject(py)
        .map(|b| b.into_any().unbind())
        .map_err(Into::into)
}

/// Positional argument `i` of a method call.
pub(crate) fn arg<'py, T: FromPyObjectOwned<'py>>(
    args: &Bound<'py, PyTuple>,
    i: usize,
) -> PyResult<T> {
    args.get_item(i)?.extract::<T>().map_err(Into::into)
}

pub(crate) fn none(py: Python<'_>) -> PyResult<Py<PyAny>> {
    Ok(py.None())
}

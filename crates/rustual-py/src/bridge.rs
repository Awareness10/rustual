//! The thread-local *frame stack* that lets Python code reach the running app.
//!
//! The Rust runtime only hands out access to the DOM through a [`Ctx`] (inside handlers) or a
//! [`RenderCtx`] (inside `render`). Python code runs *inside* those calls (an adapter method
//! calls into Python), so before calling Python the adapter pushes a [`Frame`] holding raw
//! pointers to its `Ctx`/`RenderCtx` and to its checked-out base widget. Native functions
//! called from Python use the top frame for access:
//!
//! - `Ctx` frame: full access (queries, mutation, mounting ...).
//! - `Render` frame: read-only DOM access; mutation raises `RuntimeError`.
//! - `Blocked` frame (inside `compose`, while the runtime is mid-mount): no access at all.
//! - no frame, but a [`Pilot`] is registered (tests, between pilot steps): access via the pilot.
//! - no frame on a thread other than the app thread: `WrongThreadError`, which the Python
//!   layer turns into a `call_from_thread` round trip.
//!
//! Safety: the pointers in a frame are created from `&mut` borrows that stay alive (and
//! unused) for the frame's whole lifetime, frames are strictly nested (pushed and popped
//! around synchronous Python calls, popped on unwind by [`FrameGuard`]), and only the top
//! frame's `Ctx` is ever dereferenced, so no two live `&mut` to the app state are used at once.

use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use rustual::{AppHandle, Ctx, Dom, NodeId, Pilot, RenderCtx, Widget};
use std::cell::{Cell, RefCell};
use std::sync::Mutex;
use std::thread::ThreadId;

pyo3::create_exception!(
    _native,
    WrongThreadError,
    PyRuntimeError,
    "A rustual API was called from a thread other than the app's thread."
);
pyo3::create_exception!(
    _native,
    NoActiveAppError,
    PyRuntimeError,
    "No app is running (or the widget is not mounted)."
);

#[derive(Clone, Copy)]
pub(crate) enum FrameKind {
    Ctx(*mut Ctx<'static>),
    Render(*const RenderCtx<'static>),
    /// Read-only DOM access without a render context (e.g. `check_action`).
    Read(*const Dom),
    Blocked,
}

pub(crate) struct Frame {
    pub kind: FrameKind,
    /// The node whose adapter pushed the frame.
    pub node: Option<NodeId>,
    /// That node's base widget, checked out of the DOM while the frame is active.
    pub base: Option<*mut dyn Widget>,
    /// The adapter's Python object (so lookups of a checked-out node still find it).
    pub obj: Option<Py<PyAny>>,
}

thread_local! {
    static FRAMES: RefCell<Vec<Frame>> = const { RefCell::new(Vec::new()) };
    static PILOT: Cell<Option<*mut Pilot>> = const { Cell::new(None) };
    /// First Python exception raised by a callback, re-raised when the app exits / at the
    /// next pilot step.
    static ERROR: RefCell<Option<PyErr>> = const { RefCell::new(None) };
}

/// The running app (one at a time): its thread, root and a handle to post to it.
pub(crate) struct Running {
    pub thread: ThreadId,
    pub root: NodeId,
    pub handle: AppHandle,
}

pub(crate) static RUNNING: Mutex<Option<Running>> = Mutex::new(None);

pub(crate) fn running_lock() -> std::sync::MutexGuard<'static, Option<Running>> {
    RUNNING.lock().unwrap_or_else(|e| e.into_inner())
}

/// Pops the frame pushed by [`push`] (also on unwind).
pub(crate) struct FrameGuard(());

impl Drop for FrameGuard {
    fn drop(&mut self) {
        let frame = FRAMES.with(|f| f.borrow_mut().pop());
        // Dropping a `Py` without the GIL is deferred by PyO3, so this is fine either way.
        drop(frame);
    }
}

pub(crate) fn push(frame: Frame) -> FrameGuard {
    FRAMES.with(|f| f.borrow_mut().push(frame));
    FrameGuard(())
}

#[allow(clippy::unnecessary_cast)] // the casts erase the Ctx lifetime
pub(crate) fn ctx_frame(
    ctx: &mut Ctx,
    base: Option<&mut dyn Widget>,
    obj: Option<Py<PyAny>>,
) -> Frame {
    Frame {
        kind: FrameKind::Ctx(ctx as *mut Ctx<'_> as *mut Ctx<'static>),
        node: Some(ctx.id()),
        base: base.map(|b| b as *mut dyn Widget),
        obj,
    }
}

#[allow(clippy::unnecessary_cast)]
pub(crate) fn render_frame(rctx: &RenderCtx, base: &dyn Widget, obj: Option<Py<PyAny>>) -> Frame {
    Frame {
        kind: FrameKind::Render(rctx as *const RenderCtx<'_> as *const RenderCtx<'static>),
        node: Some(rctx.node),
        base: Some(base as *const dyn Widget as *mut dyn Widget),
        obj,
    }
}

pub(crate) fn read_frame(dom: &Dom) -> Frame {
    Frame {
        kind: FrameKind::Read(dom as *const Dom),
        node: None,
        base: None,
        obj: None,
    }
}

pub(crate) fn blocked_frame() -> Frame {
    Frame {
        kind: FrameKind::Blocked,
        node: None,
        base: None,
        obj: None,
    }
}

pub(crate) fn set_pilot(p: Option<*mut Pilot>) {
    PILOT.with(|c| c.set(p));
}

fn top_kind() -> Option<FrameKind> {
    FRAMES.with(|f| f.borrow().last().map(|f| f.kind))
}

fn no_access_error() -> PyErr {
    let other_thread = running_lock()
        .as_ref()
        .map(|r| r.thread != std::thread::current().id())
        .unwrap_or(false);
    if other_thread {
        WrongThreadError::new_err(
            "this must run on the app thread; use app.call_from_thread() from workers",
        )
    } else {
        NoActiveAppError::new_err("no app is running")
    }
}

fn render_error() -> PyErr {
    PyRuntimeError::new_err(
        "this operation can't be used while rendering or in check_action (read-only access)",
    )
}

fn blocked_error() -> PyErr {
    PyRuntimeError::new_err("the DOM can't be accessed from compose(); do it in on_mount() instead")
}

/// Run `f` with mutable access to the app.
pub(crate) fn with_ctx<R>(f: impl FnOnce(&mut Ctx) -> PyResult<R>) -> PyResult<R> {
    match top_kind() {
        Some(FrameKind::Ctx(p)) => {
            // SAFETY: see the module docs; the top frame's Ctx outlives the frame.
            let ctx = unsafe { &mut *p };
            f(ctx)
        }
        Some(FrameKind::Render(_)) | Some(FrameKind::Read(_)) => Err(render_error()),
        Some(FrameKind::Blocked) => Err(blocked_error()),
        None => match PILOT.with(|c| c.get()) {
            // SAFETY: the pilot is registered only while its owner is alive and not
            // running a step (steps push frames before calling Python).
            Some(p) => unsafe { &mut *p }.with_ctx(None, f),
            None => Err(no_access_error()),
        },
    }
}

/// Run `f` with read access to the DOM (works while rendering too).
pub(crate) fn with_dom<R>(f: impl FnOnce(&Dom) -> PyResult<R>) -> PyResult<R> {
    match top_kind() {
        Some(FrameKind::Ctx(p)) => f(unsafe { &*p }.dom()),
        Some(FrameKind::Render(p)) => f(unsafe { &*p }.dom),
        // SAFETY: the DOM outlives the frame (shared borrow held by the caller).
        Some(FrameKind::Read(p)) => f(unsafe { &*p }),
        Some(FrameKind::Blocked) => Err(blocked_error()),
        None => match PILOT.with(|c| c.get()) {
            Some(p) => f(unsafe { &*p }.dom()),
            None => Err(no_access_error()),
        },
    }
}

/// The checked-out base widget of `node`, if a frame holds it.
fn frame_base(node: NodeId) -> Option<*mut dyn Widget> {
    FRAMES.with(|f| {
        f.borrow()
            .iter()
            .rev()
            .find(|fr| fr.node == Some(node) && fr.base.is_some())
            .and_then(|fr| fr.base)
    })
}

/// The Python object of a checked-out adapter node, if a frame holds it.
pub(crate) fn frame_obj(py: Python<'_>, node: NodeId) -> Option<Py<PyAny>> {
    FRAMES.with(|f| {
        f.borrow()
            .iter()
            .rev()
            .find(|fr| fr.node == Some(node) && fr.obj.is_some())
            .and_then(|fr| fr.obj.as_ref().map(|o| o.clone_ref(py)))
    })
}

/// Look through a binding adapter to the widget it wraps.
pub(crate) fn unwrap_ref(w: &dyn Widget) -> &dyn Widget {
    match w.delegate() {
        Some(inner) => inner,
        None => w,
    }
}

pub(crate) fn unwrap_mut(w: &mut dyn Widget) -> &mut dyn Widget {
    if w.delegate().is_some() {
        w.delegate_mut().expect("delegate")
    } else {
        w
    }
}

/// Run `f` with the (unwrapped) widget at `node` and a `Ctx` targeting it.
pub(crate) fn with_widget<R>(
    node: NodeId,
    f: impl FnOnce(&mut dyn Widget, &mut Ctx) -> PyResult<R>,
) -> PyResult<R> {
    if let Some(ptr) = frame_base(node) {
        if let Some(FrameKind::Render(_)) = top_kind() {
            return Err(render_error());
        }
        return with_ctx(|ctx| {
            // SAFETY: the base widget is checked out by an enclosing frame and only used
            // through this pointer while that frame's handler is suspended in Python.
            let w = unsafe { &mut *ptr };
            ctx.for_node(node, |c| f(w, c))
        });
    }
    with_ctx(
        |ctx| match ctx.with_widget(node, |w, c| f(unwrap_mut(w), c)) {
            Some(r) => r,
            None => Err(NoActiveAppError::new_err("widget is not mounted")),
        },
    )
}

/// Run `f` with read access to the (unwrapped) widget at `node`.
pub(crate) fn with_widget_ref<R>(
    node: NodeId,
    f: impl FnOnce(&dyn Widget) -> PyResult<R>,
) -> PyResult<R> {
    if let Some(ptr) = frame_base(node) {
        if !matches!(top_kind(), Some(FrameKind::Blocked)) {
            return f(unsafe { &*ptr });
        }
    }
    with_dom(|dom| {
        let w = dom
            .get(node)
            .and_then(|n| n.widget())
            .ok_or_else(|| NoActiveAppError::new_err("widget is not mounted"))?;
        f(unwrap_ref(w))
    })
}

// ---------------------------------------------------------------------------- errors

/// Record a Python exception raised by a callback and ask the app to exit
/// (Textual exits the app and re-raises when a handler fails).
pub(crate) fn report_error(err: PyErr) {
    ERROR.with(|e| {
        let mut e = e.borrow_mut();
        if e.is_none() {
            *e = Some(err);
        }
    });
    let exited = match top_kind() {
        Some(FrameKind::Ctx(p)) => {
            unsafe { &mut *p }.exit();
            true
        }
        _ => false,
    };
    if !exited {
        if let Some(r) = running_lock().as_ref() {
            r.handle.exit();
        }
    }
}

pub(crate) fn take_error() -> Option<PyErr> {
    ERROR.with(|e| e.borrow_mut().take())
}

pub(crate) fn has_error() -> bool {
    ERROR.with(|e| e.borrow().is_some())
}

/// Call into Python with the GIL; errors are reported (and `None` returned).
pub(crate) fn call_py<R>(f: impl FnOnce(Python<'_>) -> PyResult<R>) -> Option<R> {
    Python::attach(|py| match f(py) {
        Ok(r) => Some(r),
        Err(e) => {
            report_error(e);
            None
        }
    })
}

// ---------------------------------------------------------------------------- node ids

pub(crate) fn node_to_int(id: NodeId) -> u64 {
    use slotmap::Key;
    id.data().as_ffi()
}

pub(crate) fn int_to_node(v: u64) -> NodeId {
    slotmap::KeyData::from_ffi(v).into()
}

/// Wrapper asserting a value may cross into `py.detach` (it stays on this thread:
/// `detach` runs its closure synchronously on the calling thread).
pub(crate) struct AssertSend<T>(pub T);
unsafe impl<T> Send for AssertSend<T> {}

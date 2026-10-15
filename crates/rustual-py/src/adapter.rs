//! [`PyWidget`]: the adapter that mounts a Python widget object in the Rust DOM.
//!
//! Every widget created from Python is a `PyWidget` wrapping a *base* Rust widget (a built-in
//! such as `Button`, or a plain widget for pure-Python widgets) plus the Python object. Trait
//! methods call the Python object's `_rs_*` hooks when its class defines the corresponding
//! handlers and otherwise delegate to the base. Typed lookups in the core see through the
//! adapter via [`Widget::delegate`].

use crate::bridge::{self, call_py, node_to_int};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyString, PyTuple};
use rustual::events::MouseButton;
use rustual::{Binding, Child, Content, Ctx, Event, Flow, Message, NodeId, RenderCtx, Widget};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

/// A message posted from Python (payload is the Python message object).
pub(crate) struct PyMessage(pub Py<PyAny>);

/// A callable to run on the app thread (from `call_from_thread`); handled by the root.
pub(crate) struct PyCall(pub Py<PyAny>);

/// Per-Python-class information, computed once by `_rs_class_info()`.
pub(crate) struct ClassInfo {
    type_names: &'static [&'static str],
    default_css: &'static str,
    handlers: HashSet<String>,
    has_render: bool,
    has_compose: bool,
    has_content_width: bool,
    has_content_height: bool,
    has_key_methods: bool,
    has_check_action: bool,
    bindings: Vec<Binding>,
    help: Option<&'static str>,
    /// Base widget CSS + Python `DEFAULT_CSS` (the base kind is fixed per class).
    combined_css: std::sync::OnceLock<&'static str>,
}

impl ClassInfo {
    fn css_with_base(&self, base: &dyn Widget) -> &'static str {
        self.combined_css.get_or_init(|| {
            let base_css = base.default_css();
            match (base_css.is_empty(), self.default_css.is_empty()) {
                (true, _) => self.default_css,
                (false, true) => base_css,
                (false, false) => leak(format!("{base_css}\n{}", self.default_css)),
            }
        })
    }
}

/// Keyed by the type's address; the entry holds a strong reference to the type, so the
/// address can't be reused by another class while it's cached.
type ClassCache = HashMap<usize, (Py<PyAny>, Arc<ClassInfo>)>;
static CLASS_INFO: Mutex<Option<ClassCache>> = Mutex::new(None);

fn leak(s: String) -> &'static str {
    Box::leak(s.into_boxed_str())
}

pub(crate) fn class_info(py: Python<'_>, obj: &Bound<'_, PyAny>) -> PyResult<Arc<ClassInfo>> {
    let ty = obj.get_type();
    let key = ty.as_ptr() as usize;
    if let Some((_, info)) = CLASS_INFO
        .lock()
        .unwrap()
        .get_or_insert_with(HashMap::new)
        .get(&key)
    {
        return Ok(info.clone());
    }
    let d = obj.call_method0("_rs_class_info")?;
    let d = d.cast::<PyDict>()?;
    let get = |k: &str| -> PyResult<Bound<'_, PyAny>> {
        d.get_item(k)?
            .ok_or_else(|| pyo3::exceptions::PyKeyError::new_err(k.to_string()))
    };
    let names: Vec<String> = get("type_names")?.extract()?;
    let type_names: &'static [&'static str] = Box::leak(
        names
            .into_iter()
            .map(leak)
            .collect::<Vec<_>>()
            .into_boxed_slice(),
    );
    let default_css = leak(get("default_css")?.extract()?);
    let handlers: HashSet<String> = get("handlers")?
        .extract::<Vec<String>>()?
        .into_iter()
        .collect();
    let has_key_methods = handlers.iter().any(|h| h.starts_with("key_"));
    let mut bindings = Vec::new();
    for b in get("bindings")?.cast::<PyList>()?.iter() {
        bindings.push(binding_from_py(&b)?);
    }
    let help: Option<String> = get("help")?.extract()?;
    let info = Arc::new(ClassInfo {
        type_names,
        default_css,
        has_render: get("has_render")?.extract()?,
        has_compose: get("has_compose")?.extract()?,
        has_content_width: get("has_content_width")?.extract()?,
        has_content_height: get("has_content_height")?.extract()?,
        has_key_methods,
        has_check_action: get("has_check_action")?.extract()?,
        handlers,
        bindings,
        help: help.map(leak),
        combined_css: std::sync::OnceLock::new(),
    });
    let _ = py;
    CLASS_INFO
        .lock()
        .unwrap()
        .get_or_insert_with(HashMap::new)
        .insert(key, (ty.clone().into_any().unbind(), info.clone()));
    Ok(info)
}

/// `(keys, action, description, show, key_display, priority, id)` → [`Binding`].
pub(crate) fn binding_from_py(b: &Bound<'_, PyAny>) -> PyResult<Binding> {
    let (keys, action, description, show, key_display, priority, id): (
        String,
        String,
        String,
        bool,
        Option<String>,
        bool,
        Option<String>,
    ) = b.extract()?;
    let mut binding = Binding::new(&keys, &action, &description);
    if !show {
        binding = binding.hidden();
    }
    if priority {
        binding = binding.priority();
    }
    if let Some(k) = key_display {
        binding = binding.key_display(&k);
    }
    if let Some(id) = id {
        binding = binding.id(&id);
    }
    Ok(binding)
}

pub(crate) struct PyWidget {
    pub obj: Py<PyAny>,
    pub base: Box<dyn Widget>,
    info: Arc<ClassInfo>,
    can_focus: Option<bool>,
    can_focus_children: Option<bool>,
}

impl PyWidget {
    fn handles(&self, name: &str) -> bool {
        self.info.handlers.contains(name)
    }

    /// Call into Python with a `Ctx` frame for this widget.
    fn call_ctx<R>(
        &mut self,
        ctx: &mut Ctx,
        f: impl FnOnce(Python<'_>, &Bound<'_, PyAny>) -> PyResult<R>,
    ) -> Option<R> {
        call_py(|py| {
            let obj = self.obj.bind(py).clone();
            let frame =
                bridge::ctx_frame(ctx, Some(self.base.as_mut()), Some(obj.clone().unbind()));
            let _guard = bridge::push(frame);
            f(py, &obj)
        })
    }

    fn call_render<R>(
        &self,
        rctx: &RenderCtx,
        f: impl FnOnce(Python<'_>, &Bound<'_, PyAny>) -> PyResult<R>,
    ) -> Option<R> {
        call_py(|py| {
            let obj = self.obj.bind(py).clone();
            let frame = bridge::render_frame(rctx, self.base.as_ref(), Some(obj.clone().unbind()));
            let _guard = bridge::push(frame);
            f(py, &obj)
        })
    }

    /// Run a Python callable (timer callback) with a frame for this widget.
    pub(crate) fn run_callback(&mut self, ctx: &mut Ctx, callback: &Py<PyAny>) {
        self.call_ctx(ctx, |py, obj| {
            obj.call_method1("_rs_invoke", (callback.bind(py),))?;
            Ok(())
        });
    }
}

/// Build a mountable [`Child`] from a Python widget object (recursively, with its children).
pub(crate) fn build_child(py: Python<'_>, obj: &Bound<'_, PyAny>) -> PyResult<Child> {
    let spec = obj.call_method0("_rs_spec")?;
    let spec = spec.cast::<PyDict>()?;
    let item = |k: &str| -> PyResult<Bound<'_, PyAny>> {
        spec.get_item(k)?
            .ok_or_else(|| pyo3::exceptions::PyKeyError::new_err(k.to_string()))
    };
    let kind: String = item("kind")?.extract()?;
    let kwargs = item("kwargs")?;
    let kwargs = kwargs.cast::<PyDict>()?;
    let children = item("children")?;
    let children = children.cast::<PyList>()?;
    let mut child_objs: Vec<Bound<'_, PyAny>> = children.iter().collect();
    let base = crate::widgets::create(py, &kind, kwargs, &mut child_objs)?;
    let info = class_info(py, obj)?;
    let widget = PyWidget {
        obj: obj.clone().unbind(),
        base,
        info,
        can_focus: item("can_focus")?.extract()?,
        can_focus_children: item("can_focus_children")?.extract()?,
    };
    let mut child = Child::new(Box::new(widget));
    child.id = item("id")?.extract()?;
    child.classes = item("classes")?.extract()?;
    child.styles = item("styles")?.extract()?;
    child.disabled = item("disabled")?.extract()?;
    child.border_title = item("border_title")?.extract()?;
    child.border_subtitle = item("border_subtitle")?.extract()?;
    child.tooltip = item("tooltip")?.extract()?;
    for c in child_objs {
        child.children.push(build_child(py, &c)?);
    }
    Ok(child)
}

/// The adapter for the app object (root of the DOM).
pub(crate) fn build_root(py: Python<'_>, obj: &Bound<'_, PyAny>) -> PyResult<PyWidget> {
    let info = class_info(py, obj)?;
    Ok(PyWidget {
        obj: obj.clone().unbind(),
        base: Box::new(crate::widgets::PlainWidget),
        info,
        can_focus: Some(false),
        can_focus_children: None,
    })
}

fn mouse_tuple<'py>(
    py: Python<'py>,
    kind: &str,
    m: &rustual::MouseEvent,
) -> PyResult<Bound<'py, PyTuple>> {
    let button: i32 = match m.button {
        Some(MouseButton::Left) => 1,
        Some(MouseButton::Middle) => 2,
        Some(MouseButton::Right) => 3,
        None => 0,
    };
    PyTuple::new(
        py,
        [
            PyString::new(py, kind).into_any(),
            m.x.into_pyobject(py)?.into_any(),
            m.y.into_pyobject(py)?.into_any(),
            m.screen_x.into_pyobject(py)?.into_any(),
            m.screen_y.into_pyobject(py)?.into_any(),
            button.into_pyobject(py)?.into_any(),
            pyo3::types::PyBool::new(py, m.ctrl).to_owned().into_any(),
            pyo3::types::PyBool::new(py, m.alt).to_owned().into_any(),
            pyo3::types::PyBool::new(py, m.shift).to_owned().into_any(),
        ],
    )
}

/// Event → (handler name, Python tuple builder).
fn event_name(event: &Event) -> &'static str {
    match event {
        Event::Key(_) => "on_key",
        Event::MouseDown(_) => "on_mouse_down",
        Event::MouseUp(_) => "on_mouse_up",
        Event::Click(_) => "on_click",
        Event::MouseMove(_) => "on_mouse_move",
        Event::MouseScrollUp(_) => "on_mouse_scroll_up",
        Event::MouseScrollDown(_) => "on_mouse_scroll_down",
        Event::MouseScrollLeft(_) => "on_mouse_scroll_left",
        Event::MouseScrollRight(_) => "on_mouse_scroll_right",
        Event::Enter => "on_enter",
        Event::Leave => "on_leave",
        Event::Focus => "on_focus",
        Event::Blur => "on_blur",
        Event::Resize(_) => "on_resize",
        Event::Paste(_) => "on_paste",
    }
}

fn event_tuple<'py>(py: Python<'py>, event: &Event) -> PyResult<Bound<'py, PyTuple>> {
    let name = &event_name(event)[3..];
    match event {
        Event::Key(k) => PyTuple::new(
            py,
            [
                PyString::new(py, name).into_any(),
                PyString::new(py, &k.key).into_any(),
                k.character.into_pyobject(py)?.into_any(),
            ],
        ),
        Event::Resize(s) => (name, s.width, s.height).into_pyobject(py),
        Event::Paste(t) => (name, t.as_str()).into_pyobject(py),
        Event::Enter | Event::Leave | Event::Focus | Event::Blur => (name,).into_pyobject(py),
        _ => mouse_tuple(py, name, event.mouse().expect("mouse event")),
    }
}

/// Python handler results: bit 0 = stop propagation, bit 1 = prevent default.
const STOP: u32 = 1;
const PREVENT_DEFAULT: u32 = 2;

impl Widget for PyWidget {
    fn type_names(&self) -> &'static [&'static str] {
        self.info.type_names
    }

    fn default_css(&self) -> &'static str {
        self.info.css_with_base(self.base.as_ref())
    }

    fn on_attach(&mut self, id: NodeId) {
        self.base.on_attach(id);
        call_py(|py| self.obj.bind(py).setattr("_node", node_to_int(id)));
    }

    fn compose(&mut self) -> Vec<Child> {
        let mut children = self.base.compose();
        if self.info.has_compose {
            let composed = call_py(|py| {
                // The runtime is mid-mount: no DOM access from compose().
                let _guard = bridge::push(bridge::blocked_frame());
                let list = self.obj.bind(py).call_method0("_rs_compose")?;
                let mut out = Vec::new();
                for w in list.cast::<PyList>()?.iter() {
                    out.push(build_child(py, &w)?);
                }
                Ok(out)
            });
            children.extend(composed.unwrap_or_default());
        }
        children
    }

    fn on_mount(&mut self, ctx: &mut Ctx) {
        self.base.on_mount(ctx);
        self.call_ctx(ctx, |_, obj| obj.call_method0("_rs_on_mount").map(drop));
    }

    fn on_unmount(&mut self, ctx: &mut Ctx) {
        self.call_ctx(ctx, |_, obj| obj.call_method0("_rs_on_unmount").map(drop));
        self.base.on_unmount(ctx);
    }

    fn on_event(&mut self, event: &Event, ctx: &mut Ctx) -> Flow {
        let name = event_name(event);
        let wanted = self.handles(name) || (self.info.has_key_methods && event.key().is_some());
        let mut bits = 0;
        if wanted {
            bits = self
                .call_ctx(ctx, |py, obj| {
                    let t = event_tuple(py, event)?;
                    obj.call_method1("_rs_event", (t,))?.extract::<u32>()
                })
                .unwrap_or(0);
        }
        let base_flow = if bits & PREVENT_DEFAULT == 0 {
            self.base.on_event(event, ctx)
        } else {
            Flow::Continue
        };
        if bits & STOP != 0 {
            Flow::Stop
        } else {
            base_flow
        }
    }

    fn on_message(&mut self, message: &Message, ctx: &mut Ctx) -> Flow {
        if let Some(PyCall(callable)) = message.get::<PyCall>() {
            self.call_ctx(ctx, |py, _| callable.bind(py).call0().map(drop));
            return Flow::Stop;
        }
        let mut bits = 0;
        if let Some(PyMessage(m)) = message.get::<PyMessage>() {
            if !self.info.handlers.is_empty() {
                bits = self
                    .call_ctx(ctx, |py, obj| {
                        obj.call_method1("_rs_message", (m.bind(py),))?
                            .extract::<u32>()
                    })
                    .unwrap_or(0);
            }
        } else if let Some(conv) = crate::widgets::convert_message(message) {
            if self.handles(conv.handler) {
                bits = self
                    .call_ctx(ctx, |py, obj| {
                        let m = converted_message(py, message, conv)?;
                        obj.call_method1("_rs_message", (m,))?.extract::<u32>()
                    })
                    .unwrap_or(0);
            }
        }
        let base_flow = if bits & PREVENT_DEFAULT == 0 {
            self.base.on_message(message, ctx)
        } else {
            Flow::Continue
        };
        if bits & STOP != 0 {
            Flow::Stop
        } else {
            base_flow
        }
    }

    fn render(&self, ctx: &RenderCtx) -> Content {
        if !self.info.has_render {
            return self.base.render(ctx);
        }
        self.call_render(ctx, |_, obj| {
            let r = obj.call_method0("_rs_render")?;
            content_from_py(ctx, &r)
        })
        .unwrap_or_default()
    }

    fn content_width(&self, ctx: &RenderCtx) -> Option<i32> {
        if !self.info.has_content_width {
            return self.base.content_width(ctx);
        }
        self.call_render(ctx, |_, obj| {
            obj.call_method1("_rs_content_width", (ctx.size.width, ctx.size.height))?
                .extract::<Option<i32>>()
        })
        .flatten()
    }

    fn content_height(&self, ctx: &RenderCtx, width: i32) -> Option<i32> {
        if !self.info.has_content_height {
            return self.base.content_height(ctx, width);
        }
        self.call_render(ctx, |_, obj| {
            obj.call_method1(
                "_rs_content_height",
                (ctx.size.width, ctx.size.height, width),
            )?
            .extract::<Option<i32>>()
        })
        .flatten()
    }

    fn virtual_size(&self, ctx: &RenderCtx) -> Option<rustual::Size> {
        self.base.virtual_size(ctx)
    }

    fn can_focus(&self) -> bool {
        self.can_focus.unwrap_or_else(|| self.base.can_focus())
    }

    fn can_focus_children(&self) -> bool {
        self.can_focus_children
            .unwrap_or_else(|| self.base.can_focus_children())
    }

    fn bindings(&self) -> Vec<Binding> {
        let mut b = self.base.bindings();
        b.extend(self.info.bindings.iter().cloned());
        b
    }

    fn run_action(&mut self, action: &str, args: &[String], ctx: &mut Ctx) -> bool {
        let method = format!("action_{action}");
        if self.handles(&method) {
            let handled = self.call_ctx(ctx, |_, obj| {
                obj.call_method1("_rs_action", (action, args.to_vec()))?
                    .extract::<bool>()
            });
            if handled == Some(true) || bridge::has_error() {
                return true;
            }
        }
        self.base.run_action(action, args, ctx)
    }

    fn check_action(&self, action: &str, args: &[String]) -> Option<bool> {
        if !self.info.has_check_action {
            return self.base.check_action(action, args);
        }
        // Called during layout (no handler context): Python may read its own state only.
        call_py(|py| {
            let _guard = bridge::push(bridge::blocked_frame());
            let r = self
                .obj
                .bind(py)
                .call_method1("_rs_check_action", (action, args.to_vec()))?;
            if r.is_none() {
                Ok(None)
            } else {
                Ok(Some(r.is_truthy()?))
            }
        })
        .unwrap_or(Some(true))
    }

    fn commands(&self) -> Vec<rustual::Command> {
        self.base.commands()
    }

    fn allow_maximize(&self) -> Option<bool> {
        self.base.allow_maximize()
    }

    fn help(&self) -> Option<&str> {
        self.info.help.or_else(|| self.base.help())
    }

    fn delegate(&self) -> Option<&dyn Widget> {
        Some(self.base.as_ref())
    }

    fn delegate_mut(&mut self) -> Option<&mut dyn Widget> {
        Some(self.base.as_mut())
    }
}

thread_local! {
    /// The Python object for the message currently bubbling (by message id), so every handler
    /// along the chain sees the same object (attributes set by one handler persist).
    static CONVERTED: std::cell::RefCell<Option<(u64, Py<PyAny>)>> = const { std::cell::RefCell::new(None) };
}

fn converted_message<'py>(
    py: Python<'py>,
    message: &Message,
    conv: crate::widgets::MessageConv,
) -> PyResult<Bound<'py, PyAny>> {
    let cached = CONVERTED.with(|c| {
        c.borrow()
            .as_ref()
            .filter(|(id, _)| *id == message.id)
            .map(|(_, obj)| obj.clone_ref(py))
    });
    if let Some(obj) = cached {
        return Ok(obj.into_bound(py));
    }
    let m = (conv.build)(py, message)?;
    m.setattr("_sender", node_to_int(message.sender))?;
    CONVERTED.with(|c| *c.borrow_mut() = Some((message.id, m.clone().unbind())));
    Ok(m)
}

/// Convert a Python `render()` result to [`Content`].
/// `str` is Textual markup (with `$variables`); objects with `__rs_markup__` (our `Content`)
/// or `markup` (Rich `Text`) are converted through their markup; anything else via `str()`.
pub(crate) fn content_from_py(ctx: &RenderCtx, r: &Bound<'_, PyAny>) -> PyResult<Content> {
    if r.is_none() {
        return Ok(Content::Empty);
    }
    if let Ok(s) = r.cast::<PyString>() {
        return Ok(Content::Text(ctx.markup(&s.to_cow()?)));
    }
    if let Ok(m) = r.getattr("__rs_markup__") {
        let s: String = m.extract()?;
        return Ok(Content::Text(ctx.markup(&s)));
    }
    if let (Ok(m), true) = (r.getattr("markup"), r.hasattr("plain")?) {
        if let Ok(s) = m.extract::<String>() {
            return Ok(Content::Text(ctx.markup(&s)));
        }
    }
    if r.hasattr("__rich_console__")? || r.hasattr("__rich__")? {
        return crate::widgets::rich::rich_content(r.py(), ctx, r);
    }
    let s = r.str()?;
    Ok(Content::Text(rustual::Text::plain(s.to_cow()?.as_ref())))
}

/// The Python object for a node: the adapter's object, or `None` for nodes created by Rust
/// widgets (the Python layer then builds a proxy from the node's type names).
pub(crate) fn node_object(py: Python<'_>, node: NodeId) -> PyResult<Option<Py<PyAny>>> {
    if let Some(obj) = bridge::frame_obj(py, node) {
        return Ok(Some(obj));
    }
    bridge::with_dom(|dom| {
        Ok(dom.get(node).and_then(|n| n.widget()).and_then(|w| {
            (w as &dyn std::any::Any)
                .downcast_ref::<PyWidget>()
                .map(|pw| pw.obj.clone_ref(py))
        }))
    })
}

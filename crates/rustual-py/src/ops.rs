//! Native operations called by the Python layer. All take explicit node ids (ints), so they
//! work the same whichever handler (frame) is running.

use crate::adapter::{self, build_child, PyCall, PyMessage, PyWidget};
use crate::bridge::{
    self, int_to_node, node_to_int, with_ctx, with_dom, with_widget, with_widget_ref,
};
use pyo3::exceptions::{PyKeyError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyList, PyTuple};
use rustual::app::Severity;
use rustual::css::SelectorList;
use rustual::{Ctx, NodeId};
use std::time::Duration;

type Node = u64;

fn n(v: Node) -> NodeId {
    int_to_node(v)
}

fn ids(v: Vec<NodeId>) -> Vec<Node> {
    v.into_iter().map(node_to_int).collect()
}

// ---------------------------------------------------------------------------- tree & queries

/// Nodes under `root` (inclusive) matching `selector`, in document order.
#[pyfunction]
pub fn query(root: Node, selector: &str) -> PyResult<Vec<Node>> {
    let sel = SelectorList::parse(selector)
        .map_err(|e| PyValueError::new_err(format!("invalid selector {selector:?}: {e}")))?;
    with_dom(|dom| Ok(ids(dom.query(n(root), &sel))))
}

/// Whether `node` matches `selector`.
#[pyfunction]
pub fn matches(node: Node, selector: &str) -> PyResult<bool> {
    let sel = SelectorList::parse(selector)
        .map_err(|e| PyValueError::new_err(format!("invalid selector {selector:?}: {e}")))?;
    with_dom(|dom| Ok(dom.contains(n(node)) && sel.matches(dom.node_ref(n(node)))))
}

/// The Python object mounted at `node`, or `None` for nodes created by Rust widgets.
#[pyfunction]
pub fn node_object(py: Python<'_>, node: Node) -> PyResult<Option<Py<PyAny>>> {
    adapter::node_object(py, n(node))
}

/// CSS type names of a node, most specific first (ending with `"Widget"`).
#[pyfunction]
pub fn type_names(node: Node) -> PyResult<Vec<String>> {
    with_dom(|dom| {
        Ok(dom
            .get(n(node))
            .map(|nd| nd.type_names.iter().map(|s| s.to_string()).collect())
            .unwrap_or_default())
    })
}

#[pyfunction]
pub fn is_mounted(node: Node) -> PyResult<bool> {
    with_dom(|dom| Ok(dom.contains(n(node))))
}

#[pyfunction]
pub fn parent(node: Node) -> PyResult<Option<Node>> {
    with_dom(|dom| Ok(dom.parent(n(node)).map(node_to_int)))
}

#[pyfunction]
pub fn children(node: Node) -> PyResult<Vec<Node>> {
    with_dom(|dom| Ok(ids(dom.children(n(node)).to_vec())))
}

#[pyfunction]
pub fn root() -> PyResult<Node> {
    with_dom(|dom| {
        dom.root
            .map(node_to_int)
            .ok_or_else(|| bridge::NoActiveAppError::new_err("no app"))
    })
}

/// The screen containing `node`.
#[pyfunction]
pub fn screen_of(node: Node) -> PyResult<Option<Node>> {
    with_dom(|dom| Ok(dom.screen_of(n(node)).map(node_to_int)))
}

#[pyfunction]
pub fn active_screen() -> PyResult<Node> {
    with_ctx(|ctx| Ok(node_to_int(ctx.screen()))).or_else(|err| {
        // Read-only frames (render, check_action) see the screen recorded by the last layout.
        with_dom(|dom| dom.active_screen.map(node_to_int).ok_or(err))
    })
}

#[pyfunction]
pub fn css_id(node: Node) -> PyResult<Option<String>> {
    with_dom(|dom| Ok(dom.get(n(node)).and_then(|nd| nd.css_id.clone())))
}

// ---------------------------------------------------------------------------- classes & styles

#[pyfunction]
pub fn classes(node: Node) -> PyResult<Vec<String>> {
    with_dom(|dom| {
        Ok(dom
            .get(n(node))
            .map(|nd| nd.classes.clone())
            .unwrap_or_default())
    })
}

#[pyfunction]
pub fn has_class(node: Node, class: &str) -> PyResult<bool> {
    with_dom(|dom| {
        Ok(dom
            .get(n(node))
            .map(|nd| nd.has_class(class))
            .unwrap_or(false))
    })
}

/// `op`: "add", "remove", "toggle".
#[pyfunction]
pub fn update_classes(node: Node, op: &str, names: Vec<String>) -> PyResult<()> {
    with_ctx(|ctx| {
        let Some(nd) = ctx.dom_mut().get_mut(n(node)) else {
            return Ok(());
        };
        for c in &names {
            match op {
                "add" => nd.add_class(c),
                "remove" => nd.remove_class(c),
                "toggle" => nd.toggle_class(c),
                _ => return Err(PyValueError::new_err(format!("bad class op {op}"))),
            }
        }
        Ok(())
    })
}

#[pyfunction]
pub fn set_classes(node: Node, names: Vec<String>) -> PyResult<()> {
    with_ctx(|ctx| {
        if let Some(nd) = ctx.dom_mut().get_mut(n(node)) {
            nd.classes = names;
        }
        Ok(())
    })
}

/// Apply inline CSS declarations (`"width: 10; color: red"`).
#[pyfunction]
pub fn set_inline_style(node: Node, css: &str) -> PyResult<()> {
    with_ctx(|ctx| ctx.set_style(n(node), css).map_err(PyValueError::new_err))
}

#[pyfunction]
pub fn set_display(node: Node, visible: bool) -> PyResult<()> {
    with_ctx(|ctx| {
        ctx.set_display(n(node), visible);
        Ok(())
    })
}

/// A computed style property as CSS text (currently `"display"` and `"visibility"`).
#[pyfunction]
pub fn computed_style(node: Node, prop: &str) -> PyResult<Option<String>> {
    use rustual::css::{Display, Visibility};
    with_dom(|dom| {
        let Some(nd) = dom.get(n(node)) else {
            return Ok(None);
        };
        Ok(match prop {
            "display" => Some(
                match nd.styles.get_display() {
                    Display::None => "none",
                    _ => "block",
                }
                .to_string(),
            ),
            "visibility" => Some(
                match nd.styles.get_visibility() {
                    Visibility::Hidden => "hidden",
                    _ => "visible",
                }
                .to_string(),
            ),
            _ => None,
        })
    })
}

#[pyfunction]
pub fn set_disabled(node: Node, disabled: bool) -> PyResult<()> {
    with_ctx(|ctx| {
        ctx.set_disabled(n(node), disabled);
        Ok(())
    })
}

#[pyfunction]
pub fn is_disabled(node: Node) -> PyResult<bool> {
    with_dom(|dom| Ok(dom.is_disabled(n(node))))
}

/// `which`: "title", "subtitle" or "tooltip".
#[pyfunction]
pub fn set_node_text(node: Node, which: &str, value: Option<String>) -> PyResult<()> {
    with_ctx(|ctx| {
        let Some(nd) = ctx.dom_mut().get_mut(n(node)) else {
            return Ok(());
        };
        match which {
            "title" => nd.border_title = value,
            "subtitle" => nd.border_subtitle = value,
            "tooltip" => nd.tooltip = value,
            _ => return Err(PyValueError::new_err(format!("unknown node text {which}"))),
        }
        Ok(())
    })
}

#[pyfunction]
pub fn refresh() -> PyResult<()> {
    match with_ctx(|ctx| {
        ctx.refresh();
        Ok(())
    }) {
        // Refreshing while rendering (e.g. a reactive set in render) is harmless to skip.
        Err(_) => Ok(()),
        ok => ok,
    }
}

#[pyfunction]
pub fn set_loading(node: Node, loading: bool) -> PyResult<()> {
    with_ctx(|ctx| {
        ctx.set_loading(n(node), loading);
        Ok(())
    })
}

// ---------------------------------------------------------------------------- mounting

/// Mount Python widgets under `parent` (at `index`, or appended). Returns their node ids.
#[pyfunction]
#[pyo3(signature = (parent, widgets, index=None))]
pub fn mount(
    py: Python<'_>,
    parent: Node,
    widgets: &Bound<'_, PyList>,
    index: Option<usize>,
) -> PyResult<Vec<Node>> {
    let mut built = Vec::new();
    for w in widgets.iter() {
        built.push(build_child(py, &w)?);
    }
    with_ctx(|ctx| {
        let mut out = Vec::new();
        for (i, child) in built.into_iter().enumerate() {
            let id = ctx.mount_to(n(parent), child, index.map(|x| x + i));
            out.push(node_to_int(id));
        }
        Ok(out)
    })
}

#[pyfunction]
pub fn remove(node: Node) -> PyResult<()> {
    with_ctx(|ctx| {
        ctx.remove(n(node));
        Ok(())
    })
}

#[pyfunction]
pub fn remove_children(node: Node) -> PyResult<()> {
    with_ctx(|ctx| {
        ctx.remove_children(n(node));
        Ok(())
    })
}

// ---------------------------------------------------------------------------- focus & geometry

#[pyfunction]
pub fn focus(node: Node) -> PyResult<()> {
    with_ctx(|ctx| {
        ctx.focus(n(node));
        Ok(())
    })
}

#[pyfunction]
pub fn blur() -> PyResult<()> {
    with_ctx(|ctx| {
        if let Some(focused) = ctx.focused() {
            ctx.for_node(focused, |c| c.blur());
        }
        Ok(())
    })
}

#[pyfunction]
pub fn focus_next(forward: bool) -> PyResult<()> {
    with_ctx(|ctx| {
        if forward {
            ctx.focus_next()
        } else {
            ctx.focus_previous()
        }
        Ok(())
    })
}

#[pyfunction]
pub fn focused() -> PyResult<Option<Node>> {
    with_dom(|dom| Ok(dom.focused.map(node_to_int)))
}

#[pyfunction]
pub fn hovered() -> PyResult<Option<Node>> {
    with_dom(|dom| Ok(dom.hovered.map(node_to_int)))
}

/// `(x, y, width, height)` of the node's region (`"outer"`) or content region (`"content"`).
#[pyfunction]
#[pyo3(signature = (node, which="outer"))]
pub fn region(node: Node, which: &str) -> PyResult<(i32, i32, i32, i32)> {
    with_dom(|dom| {
        let Some(nd) = dom.get(n(node)) else {
            return Ok((0, 0, 0, 0));
        };
        let r = match which {
            "content" => nd.layout.content_region,
            "virtual" => {
                let v = nd.layout.virtual_size;
                rustual::Region::new(0, 0, v.width, v.height)
            }
            _ => nd.layout.region,
        };
        Ok((r.x, r.y, r.width, r.height))
    })
}

#[pyfunction]
pub fn app_size() -> PyResult<(i32, i32)> {
    with_ctx(|ctx| {
        let s = ctx.app_size();
        Ok((s.width, s.height))
    })
}

#[pyfunction]
pub fn scroll_offset(node: Node) -> PyResult<(i32, i32, i32, i32)> {
    with_dom(|dom| {
        let Some(nd) = dom.get(n(node)) else {
            return Ok((0, 0, 0, 0));
        };
        let max = nd.layout.max_scroll();
        Ok((nd.scroll_offset.x, nd.scroll_offset.y, max.x, max.y))
    })
}

#[pyfunction]
pub fn scroll_to(node: Node, x: i32, y: i32) -> PyResult<()> {
    with_ctx(|ctx| {
        ctx.scroll_node_to(n(node), x, y);
        Ok(())
    })
}

#[pyfunction]
pub fn scroll_visible(node: Node) -> PyResult<()> {
    with_ctx(|ctx| {
        ctx.for_node(n(node), |c| c.scroll_visible());
        Ok(())
    })
}

// ---------------------------------------------------------------------------- messages, actions

#[pyfunction]
#[pyo3(signature = (sender, target, message, bubble=true))]
pub fn post_message(sender: Node, target: Node, message: Py<PyAny>, bubble: bool) -> PyResult<()> {
    with_ctx(|ctx| {
        ctx.for_node(n(sender), |c| {
            if bubble {
                c.post_message_to(n(target), PyMessage(message))
            } else {
                c.post_message_no_bubble(n(target), PyMessage(message))
            }
        });
        Ok(())
    })
}

/// Run an action string (e.g. `"app.quit"`) as if from `node`'s binding.
#[pyfunction]
pub fn run_action(node: Node, action: &str) -> PyResult<()> {
    with_ctx(|ctx| {
        ctx.for_node(n(node), |c| c.run_action(action));
        Ok(())
    })
}

/// Post a callable to be run on the app thread (any thread). Returns false if no app runs.
#[pyfunction]
pub fn post_call(callable: Py<PyAny>) -> bool {
    let running = bridge::running_lock();
    match running.as_ref() {
        Some(r) => r.handle.post_message(r.root, PyCall(callable)),
        None => false,
    }
}

#[pyfunction]
pub fn on_app_thread() -> bool {
    bridge::running_lock()
        .as_ref()
        .map(|r| r.thread == std::thread::current().id())
        .unwrap_or(true)
}

#[pyfunction]
pub fn app_running() -> bool {
    bridge::running_lock().is_some()
}

// ---------------------------------------------------------------------------- timers

/// Call `callback` (through `widget._rs_invoke`) after `delay` seconds, optionally repeating.
#[pyfunction]
pub fn set_timer(node: Node, delay: f64, callback: Py<PyAny>, repeat: bool) -> PyResult<u64> {
    let d = Duration::from_secs_f64(delay.max(0.0));
    with_ctx(|ctx| {
        let f = move |w: &mut PyWidget, c: &mut Ctx| w.run_callback(c, &callback);
        let id = ctx.for_node(n(node), |c| {
            if repeat {
                c.set_interval::<PyWidget>(d, f)
            } else {
                c.set_timer::<PyWidget>(d, move |w, c| f(w, c))
            }
        });
        Ok(timer_to_int(id))
    })
}

fn timer_to_int(id: rustual::TimerId) -> u64 {
    // TimerId is opaque; keep the ids in a side table.
    TIMERS.with(|t| {
        let mut t = t.borrow_mut();
        t.push(id);
        (t.len() - 1) as u64
    })
}

thread_local! {
    static TIMERS: std::cell::RefCell<Vec<rustual::TimerId>> = const { std::cell::RefCell::new(Vec::new()) };
}

#[pyfunction]
pub fn cancel_timer(id: u64) -> PyResult<()> {
    let tid = TIMERS.with(|t| t.borrow().get(id as usize).copied());
    if let Some(tid) = tid {
        with_ctx(|ctx| {
            ctx.cancel_timer(tid);
            Ok(())
        })?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------- app-level

#[pyfunction]
pub fn exit() -> PyResult<()> {
    with_ctx(|ctx| {
        ctx.exit();
        Ok(())
    })
}

#[pyfunction]
pub fn bell() -> PyResult<()> {
    with_ctx(|ctx| {
        ctx.bell();
        Ok(())
    })
}

#[pyfunction]
#[pyo3(signature = (message, title="", severity="information", timeout=None))]
pub fn notify(message: &str, title: &str, severity: &str, timeout: Option<f64>) -> PyResult<()> {
    let severity = match severity {
        "warning" => Severity::Warning,
        "error" => Severity::Error,
        _ => Severity::Information,
    };
    with_ctx(|ctx| {
        ctx.notify_with(
            message,
            title,
            severity,
            timeout.map(|t| Duration::from_secs_f64(t.max(0.0))),
        );
        Ok(())
    })
}

/// `which`: "title" or "sub_title".
#[pyfunction]
pub fn set_app_text(which: &str, value: &str) -> PyResult<()> {
    with_ctx(|ctx| {
        match which {
            "title" => ctx.set_title(value),
            _ => ctx.set_sub_title(value),
        }
        Ok(())
    })
}

/// `Screen.title` / `Screen.sub_title` of the screen `node` (`None`: the app's).
#[pyfunction]
#[pyo3(signature = (node, which, value))]
pub fn set_screen_text(node: Node, which: &str, value: Option<&str>) -> PyResult<()> {
    with_ctx(|ctx| {
        ctx.for_node(n(node), |c| match which {
            "title" => c.set_screen_title(value),
            _ => c.set_screen_sub_title(value),
        });
        Ok(())
    })
}

#[pyfunction]
pub fn set_theme(name: &str) -> PyResult<bool> {
    with_ctx(|ctx| Ok(ctx.set_theme(name)))
}

#[pyfunction]
pub fn theme() -> PyResult<String> {
    with_ctx(|ctx| Ok(ctx.theme().to_string()))
}

#[pyfunction]
pub fn theme_names() -> PyResult<Vec<String>> {
    with_ctx(|ctx| Ok(ctx.theme_names()))
}

/// `App.native_ansi_color`.
#[pyfunction]
pub fn native_ansi_color() -> PyResult<bool> {
    with_ctx(|ctx| Ok(ctx.native_ansi_color()))
}

/// Set `App.ansi_color` (`None`: follow the theme).
#[pyfunction]
#[pyo3(signature = (ansi_color))]
pub fn set_ansi_color(ansi_color: Option<bool>) -> PyResult<()> {
    with_ctx(|ctx| {
        ctx.set_ansi_color(ansi_color);
        Ok(())
    })
}

#[pyfunction]
pub fn copy_to_clipboard(text: &str) -> PyResult<()> {
    with_ctx(|ctx| {
        ctx.copy_to_clipboard(text);
        Ok(())
    })
}

#[pyfunction]
pub fn clipboard() -> PyResult<String> {
    with_ctx(|ctx| Ok(ctx.clipboard().to_string()))
}

// ---------------------------------------------------------------------------- screens

#[pyfunction]
pub fn push_screen(
    py: Python<'_>,
    pusher: Node,
    screen: &Bound<'_, PyAny>,
    modal: bool,
) -> PyResult<Node> {
    let child = build_child(py, screen)?;
    with_ctx(|ctx| {
        let id = ctx.for_node(n(pusher), |c| {
            if modal {
                c.push_modal(child)
            } else {
                c.push_screen(child)
            }
        });
        Ok(node_to_int(id))
    })
}

#[pyfunction]
pub fn switch_screen(py: Python<'_>, screen: &Bound<'_, PyAny>) -> PyResult<Node> {
    let child = build_child(py, screen)?;
    with_ctx(|ctx| Ok(node_to_int(ctx.switch_screen(child))))
}

#[pyfunction]
pub fn pop_screen() -> PyResult<()> {
    with_ctx(|ctx| {
        ctx.pop_screen();
        Ok(())
    })
}

#[pyfunction]
pub fn screen_stack() -> PyResult<Vec<Node>> {
    with_ctx(|ctx| {
        let root = ctx.root();
        let dom = ctx.dom();
        Ok(dom
            .children(root)
            .iter()
            .filter(|c| dom.get(**c).map(|nd| nd.is_screen).unwrap_or(false))
            .map(|c| node_to_int(*c))
            .collect())
    })
}

#[pyfunction]
pub fn install_screen(py: Python<'_>, screen: &Bound<'_, PyAny>, name: &str) -> PyResult<Node> {
    let child = build_child(py, screen)?;
    with_ctx(|ctx| Ok(node_to_int(ctx.install_screen(child, name))))
}

#[pyfunction]
pub fn uninstall_screen(name: &str) -> PyResult<bool> {
    with_ctx(|ctx| Ok(ctx.uninstall_screen(name)))
}

#[pyfunction]
pub fn is_screen_installed(name: &str) -> PyResult<bool> {
    with_ctx(|ctx| Ok(ctx.is_screen_installed(name)))
}

/// `op`: "push" or "switch".
#[pyfunction]
pub fn named_screen(op: &str, name: &str) -> PyResult<Node> {
    with_ctx(|ctx| {
        let id = match op {
            "push" => ctx.push_screen_named(name),
            _ => ctx.switch_screen_named(name),
        };
        id.map(node_to_int)
            .ok_or_else(|| PyKeyError::new_err(format!("no screen named {name:?}")))
    })
}

#[pyfunction]
pub fn switch_mode(name: &str) -> PyResult<()> {
    with_ctx(|ctx| {
        if ctx.switch_mode(name) {
            Ok(())
        } else {
            Err(PyKeyError::new_err(format!("no mode named {name:?}")))
        }
    })
}

#[pyfunction]
pub fn current_mode() -> PyResult<String> {
    with_ctx(|ctx| Ok(ctx.current_mode().to_string()))
}

#[pyfunction]
pub fn maximize(node: Option<Node>) -> PyResult<bool> {
    with_ctx(|ctx| {
        Ok(match node {
            Some(id) => ctx.maximize(n(id)),
            None => {
                ctx.minimize();
                true
            }
        })
    })
}

// ---------------------------------------------------------------------------- built-in widget data

/// Read an attribute of a built-in (Rust) widget.
#[pyfunction]
pub fn widget_get(py: Python<'_>, node: Node, attr: &str) -> PyResult<Py<PyAny>> {
    with_widget_ref(n(node), |w| crate::widgets::get(py, w, attr))
}

/// Set an attribute of a built-in (Rust) widget.
#[pyfunction]
pub fn widget_set(
    py: Python<'_>,
    node: Node,
    attr: &str,
    value: &Bound<'_, PyAny>,
) -> PyResult<()> {
    with_widget(n(node), |w, ctx| {
        crate::widgets::set(py, w, ctx, attr, value)
    })
}

/// Call a method of a built-in (Rust) widget.
#[pyfunction]
pub fn widget_call(
    py: Python<'_>,
    node: Node,
    method: &str,
    args: &Bound<'_, PyTuple>,
) -> PyResult<Py<PyAny>> {
    with_widget(n(node), |w, ctx| {
        crate::widgets::call(py, w, ctx, method, args)
    })
}

/// Add app-level CSS at runtime (e.g. a screen's `CSS`). Returns parse errors.
#[pyfunction]
pub fn add_css(key: &str, css: &str) -> PyResult<Vec<String>> {
    with_ctx(|ctx| Ok(ctx.add_css(key, css)))
}

#[pyfunction]
pub fn has_css(key: &str) -> PyResult<bool> {
    with_ctx(|ctx| Ok(ctx.has_css(key)))
}

/// Record an exception from Python code run outside a handler (async task, worker) so the app
/// exits and re-raises it.
#[pyfunction]
pub fn report_error(error: &Bound<'_, PyAny>) {
    bridge::report_error(PyErr::from_value(error.clone()));
}

/// Take the recorded handler exception, if any.
#[pyfunction]
pub fn take_error(py: Python<'_>) -> Option<Py<PyAny>> {
    bridge::take_error().map(|e| e.into_value(py).into_any())
}

/// Register a theme at runtime.
#[pyfunction]
pub fn register_theme(theme: crate::runner::ThemeSpec) -> PyResult<()> {
    with_ctx(|ctx| {
        ctx.register_theme(theme.into_theme());
        Ok(())
    })
}

/// Suppress native messages handled by `handlers` (e.g. `"on_switch_changed"`) until
/// `prevent_pop` (Textual's `with self.prevent(...)`).
#[pyfunction]
pub fn prevent_push(handlers: Vec<String>) -> PyResult<()> {
    let types = crate::widgets::message_types_for(&handlers);
    with_ctx(|ctx| {
        ctx.prevent_messages(types);
        Ok(())
    })
}

#[pyfunction]
pub fn prevent_pop() -> PyResult<()> {
    with_ctx(|ctx| {
        ctx.allow_messages();
        Ok(())
    })
}

/// The fields of a built-in theme (no app needed), or `None`.
#[pyfunction]
pub fn builtin_theme(py: Python<'_>, name: &str) -> PyResult<Option<Py<pyo3::types::PyDict>>> {
    let Some(t) = rustual::css::Theme::builtin()
        .into_iter()
        .find(|t| t.name == name)
    else {
        return Ok(None);
    };
    let d = pyo3::types::PyDict::new(py);
    d.set_item("name", &t.name)?;
    d.set_item("primary", &t.primary)?;
    for (k, v) in [
        ("secondary", &t.secondary),
        ("warning", &t.warning),
        ("error", &t.error),
        ("success", &t.success),
        ("accent", &t.accent),
        ("foreground", &t.foreground),
        ("background", &t.background),
        ("surface", &t.surface),
        ("panel", &t.panel),
        ("boost", &t.boost),
    ] {
        d.set_item(k, v)?;
    }
    d.set_item("dark", t.dark)?;
    d.set_item("luminosity_spread", t.luminosity_spread)?;
    d.set_item("text_alpha", t.text_alpha)?;
    d.set_item("variables", t.variables.clone())?;
    d.set_item("ansi", t.ansi)?;
    Ok(Some(d.unbind()))
}

/// Names of the built-in themes (`BUILTIN_THEMES`).
#[pyfunction]
pub fn builtin_theme_names() -> Vec<String> {
    rustual::css::Theme::builtin()
        .into_iter()
        .map(|t| t.name)
        .collect()
}

/// The CSS variables a theme generates (`ColorSystem.generate()`).
#[pyfunction]
pub fn theme_variables(
    spec: crate::runner::ThemeSpec,
) -> std::collections::HashMap<String, String> {
    spec.into_theme().to_variables()
}

/// `(color, bgcolor, {attribute: on})` of a style, colors as Rich color strings.
type RichStyleParts = (
    Option<String>,
    Option<String>,
    std::collections::HashMap<&'static str, bool>,
);

/// Colors and attributes of a widget, or one of its component classes (for `rich_style` /
/// `get_component_rich_style`): `(color, bgcolor, {attr: bool})`, colors as Rich color strings.
#[pyfunction]
#[pyo3(signature = (node, component=None, partial=false))]
pub fn widget_style(
    node: Node,
    component: Option<&str>,
    partial: bool,
) -> PyResult<RichStyleParts> {
    with_ctx(|ctx| {
        let s = ctx.widget_style(n(node), component, partial);
        let color = |c: Option<rustual::Color>| {
            c.filter(|c| !c.is_transparent())
                .map(|c| match c.ansi_name() {
                    Some(name) => name.trim_start_matches("ansi_").to_string(),
                    None => c.with_alpha(1.0).hex(),
                })
        };
        let mut attrs = std::collections::HashMap::new();
        for (name, value) in [
            ("bold", s.bold),
            ("dim", s.dim),
            ("italic", s.italic),
            ("underline", s.underline),
            ("strike", s.strike),
            ("reverse", s.reverse),
            ("blink", s.blink),
        ] {
            if let Some(v) = value {
                attrs.insert(name, v);
            }
        }
        Ok((color(s.fg), color(s.bg), attrs))
    })
}

/// `DOMNode.bind`: add a binding to `node` at runtime.
#[pyfunction]
pub fn bind(node: Node, binding: &Bound<'_, PyAny>) -> PyResult<()> {
    let binding = crate::adapter::binding_from_py(binding)?;
    with_ctx(|ctx| {
        ctx.for_node(n(node), |c| c.bind(binding));
        Ok(())
    })
}

/// `App.clear_notifications`.
#[pyfunction]
pub fn clear_notifications() -> PyResult<()> {
    with_ctx(|ctx| {
        ctx.clear_notifications();
        Ok(())
    })
}

/// `App.set_keymap` (`replace`) / `App.update_keymap`: binding id → keys.
#[pyfunction]
pub fn set_keymap(
    keymap: std::collections::HashMap<String, String>,
    replace: bool,
) -> PyResult<()> {
    with_ctx(|ctx| {
        if replace {
            ctx.set_keymap(keymap);
        } else {
            ctx.update_keymap(keymap);
        }
        Ok(())
    })
}

/// Footer-style display of a key (`ctrl+s` → `^s`).
#[pyfunction]
pub fn format_key(key: &str) -> String {
    rustual::binding::format_key(key)
}

/// Show / hide the help panel (`App.action_show_help_panel` / `action_hide_help_panel`).
#[pyfunction]
pub fn help_panel(show: bool) -> PyResult<()> {
    with_ctx(|ctx| {
        if show {
            ctx.show_help_panel();
        } else {
            ctx.hide_help_panel();
        }
        Ok(())
    })
}

/// `Matcher.match` / `highlight`: fuzzy score (the Textual scale) and matched char offsets.
#[pyfunction]
pub fn fuzzy_match(query: &str, candidate: &str, case_sensitive: bool) -> (f64, Vec<usize>) {
    rustual::command::FuzzySearch::new(case_sensitive).match_offsets(query, candidate)
}

/// The built-in system commands for the active screen: `(title, help, action)`.
#[pyfunction]
pub fn system_commands() -> PyResult<Vec<(String, String, String)>> {
    with_ctx(|ctx| {
        Ok(ctx
            .system_commands()
            .into_iter()
            .filter_map(|c| match c.action {
                rustual::command::CommandAction::Action(a) => Some((c.name, c.help, a)),
                rustual::command::CommandAction::Callback(_) => None,
            })
            .collect())
    })
}

/// Deliver provider hits to an open palette:
/// `(score or None for discovery, display markup, text, help, callback id)`.
#[pyfunction]
pub fn palette_add_hits(
    palette: Node,
    generation: u64,
    hits: Vec<(Option<f64>, String, String, String, u64)>,
) -> PyResult<()> {
    let commands = hits
        .into_iter()
        .map(|(score, display, text, help, id)| {
            let mut c =
                rustual::Command::new(&text, &help, &format!("app._rs_palette_callback({id})"))
                    .display(rustual::Text::from_markup(&display));
            if let Some(s) = score {
                c = c.score(s);
            }
            c
        })
        .collect();
    with_ctx(|ctx| {
        ctx.add_command_hits(n(palette), generation, commands);
        Ok(())
    })
}

/// Open a command palette searching the app's providers (`builtin`: also the Rust-side
/// commands, i.e. widgets' `commands()`).
#[pyfunction]
pub fn open_command_palette(placeholder: &str, builtin: bool) -> PyResult<()> {
    with_ctx(|ctx| {
        ctx.open_command_palette_searching(Vec::new(), placeholder, builtin);
        Ok(())
    })
}

/// Keep the terminal in application mode between app runs (the launcher), or restore it.
#[pyfunction]
pub fn keep_terminal(keep: bool) {
    rustual::driver::keep_terminal(keep);
}

/// The plain text of Textual markup.
#[pyfunction]
pub fn markup_plain(markup: &str) -> String {
    rustual::Text::from_markup(markup).plain_text()
}

/// Named colors (`COLOR_NAME_TO_RGB`): name → (r, g, b) or (r, g, b, a) when translucent.
#[pyfunction]
pub fn color_names() -> Vec<(&'static str, Vec<u8>)> {
    rustual::Color::named_colors()
        .map(|(name, (r, g, b, a))| {
            (
                name,
                if a == 255 {
                    vec![r, g, b]
                } else {
                    vec![r, g, b, a]
                },
            )
        })
        .collect()
}

/// `Screen.HORIZONTAL_BREAKPOINTS` / `VERTICAL_BREAKPOINTS` of the screen `node` (`None`: the
/// app's).
#[pyfunction]
#[pyo3(signature = (node, horizontal, vertical))]
pub fn set_screen_breakpoints(
    node: Node,
    horizontal: Option<Vec<(i32, String)>>,
    vertical: Option<Vec<(i32, String)>>,
) -> PyResult<()> {
    with_ctx(|ctx| {
        ctx.for_node(n(node), |c| c.set_screen_breakpoints(horizontal, vertical));
        Ok(())
    })
}

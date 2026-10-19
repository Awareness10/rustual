//! Running an app (`App.run`) and driving it headlessly (`App.run_test` → [`PilotHandle`]).
//!
//! GIL discipline: the event loop runs inside `py.detach` (GIL released) so Python threads
//! (workers) keep running; every callback into Python re-attaches with `Python::attach`.
//! Nothing ever blocks on another thread while holding the GIL.

use crate::adapter::{build_child, build_root};
use crate::bridge::{self, node_to_int, AssertSend, Running};
use pyo3::exceptions::{PyOSError, PyRuntimeError};
use pyo3::prelude::*;
use rustual::{App, Child, Pilot, Size};
use std::time::Duration;

/// App configuration collected by the Python `App` class.
#[derive(FromPyObject)]
#[pyo3(from_item_all)]
pub struct AppConfig {
    css: Vec<(String, String)>,
    title: String,
    sub_title: String,
    theme: Option<String>,
    /// `(name, factory, modal)` from `SCREENS`.
    screens: Vec<(String, Py<PyAny>, bool)>,
    /// `(name, factory)` from `MODES`.
    modes: Vec<(String, Py<PyAny>)>,
    default_mode: Option<String>,
    themes: Vec<ThemeSpec>,
    command_palette: bool,
    /// `App.AUTO_FOCUS`.
    auto_focus: Option<String>,
    /// `App(ansi_color=...)`: `None` follows the theme.
    ansi_color: Option<bool>,
    /// `HORIZONTAL_BREAKPOINTS` / `VERTICAL_BREAKPOINTS`.
    horizontal_breakpoints: Vec<(i32, String)>,
    vertical_breakpoints: Vec<(i32, String)>,
}

/// A theme from Python (`rustual.theme.Theme`, passed as a dict).
#[derive(FromPyObject)]
#[pyo3(from_item_all)]
pub struct ThemeSpec {
    name: String,
    primary: String,
    secondary: Option<String>,
    warning: Option<String>,
    error: Option<String>,
    success: Option<String>,
    accent: Option<String>,
    foreground: Option<String>,
    background: Option<String>,
    surface: Option<String>,
    panel: Option<String>,
    boost: Option<String>,
    dark: bool,
    luminosity_spread: f32,
    text_alpha: f32,
    variables: std::collections::HashMap<String, String>,
    ansi: bool,
}

impl ThemeSpec {
    pub fn into_theme(self) -> rustual::css::Theme {
        let mut t = rustual::css::Theme::new(&self.name, &self.primary);
        t.secondary = self.secondary;
        t.warning = self.warning;
        t.error = self.error;
        t.success = self.success;
        t.accent = self.accent;
        t.foreground = self.foreground;
        t.background = self.background;
        t.surface = self.surface;
        t.panel = self.panel;
        t.boost = self.boost;
        t.dark = self.dark;
        t.luminosity_spread = self.luminosity_spread;
        t.text_alpha = self.text_alpha;
        t.variables = self.variables;
        t.ansi = self.ansi;
        t
    }
}

/// A screen factory: calls the Python factory (a class or callable) and builds the screen.
fn factory(f: Py<PyAny>) -> impl Fn() -> Child + 'static {
    move || {
        bridge::call_py(|py| {
            // Factories run while the runtime is mid-operation: no DOM access from __init__.
            let _guard = bridge::push(bridge::blocked_frame());
            let obj = f.bind(py).call0()?;
            build_child(py, &obj)
        })
        .unwrap_or_else(|| Child::new(Box::new(crate::widgets::PlainWidget)))
    }
}

fn breakpoint_refs(v: &[(i32, String)]) -> Vec<(i32, &str)> {
    v.iter().map(|(n, c)| (*n, c.as_str())).collect()
}

fn build_app(py: Python<'_>, app: &Bound<'_, PyAny>, cfg: AppConfig) -> PyResult<App> {
    let root = build_root(py, app)?;
    let mut a = App::new(root).title(&cfg.title).sub_title(&cfg.sub_title);
    a = a.enable_command_palette(cfg.command_palette);
    a = a.auto_focus(cfg.auto_focus.as_deref());
    // Python apps search with `App.COMMANDS` providers (system commands included, via
    // `App.get_system_commands`), driven by `App._rs_command_search`.
    let app_obj = app.clone().unbind();
    a = a
        .system_commands(false)
        .command_search(move |palette, event, ctx| {
            use rustual::command::SearchEvent;
            let (kind, generation, query) = match event {
                SearchEvent::Opened => ("opened", 0, ""),
                SearchEvent::Query { query, generation } => ("query", generation, query),
                SearchEvent::Closed => ("closed", 0, ""),
            };
            let _frame = bridge::push(bridge::ctx_frame(ctx, None, None));
            bridge::call_py(|py| {
                app_obj.bind(py).call_method1(
                    "_rs_command_search",
                    (kind, crate::bridge::node_to_int(palette), generation, query),
                )?;
                Ok(())
            });
        });
    for theme in cfg.themes {
        a = a.register_theme(theme.into_theme());
    }
    for (name, css) in &cfg.css {
        a = a.css(css);
        let _ = name;
    }
    if let Some(t) = &cfg.theme {
        a = a.theme(t);
    }
    if let Some(ansi) = cfg.ansi_color {
        a = a.ansi_color(ansi);
    }
    a = a.horizontal_breakpoints(&breakpoint_refs(&cfg.horizontal_breakpoints));
    a = a.vertical_breakpoints(&breakpoint_refs(&cfg.vertical_breakpoints));
    for (name, f, modal) in cfg.screens {
        a = if modal {
            a.modal_screen(&name, factory(f))
        } else {
            a.screen(&name, factory(f))
        };
    }
    for (name, f) in cfg.modes {
        a = a.mode(&name, factory(f));
    }
    if let Some(m) = &cfg.default_mode {
        a = a.default_mode(m);
    }
    Ok(a)
}

fn register_running(app: &App) -> PyResult<()> {
    let mut running = bridge::running_lock();
    if running.is_some() {
        return Err(PyRuntimeError::new_err("another app is already running"));
    }
    *running = Some(Running {
        thread: std::thread::current().id(),
        root: app.root(),
        handle: app.handle(),
    });
    Ok(())
}

fn unregister_running() {
    *bridge::running_lock() = None;
}

/// Run the app in the terminal until it exits. Re-raises the first exception raised by a
/// Python handler.
#[pyfunction]
pub fn run_app(py: Python<'_>, app: &Bound<'_, PyAny>, config: AppConfig) -> PyResult<()> {
    let a = build_app(py, app, config)?;
    register_running(&a)?;
    let _ = bridge::take_error();
    let a = AssertSend(a);
    let result = py.detach(move || {
        let a = a;
        a.0.run_blocking()
    });
    unregister_running();
    if let Some(e) = bridge::take_error() {
        return Err(e);
    }
    result.map_err(|e| PyOSError::new_err(e.to_string()))
}

/// A headless app driven from Python tests (Textual's `Pilot`). Unsendable: it must be used
/// on the thread that created it.
#[pyclass(unsendable)]
pub struct PilotHandle {
    pilot: *mut Pilot,
    /// `None` after `close`. Shut down without blocking: the handle may be garbage-collected
    /// while another app's runtime is running on this thread.
    rt: Option<tokio::runtime::Runtime>,
}

impl PilotHandle {
    #[allow(clippy::mut_from_ref)] // single-threaded handle; see SAFETY below
    fn pilot(&self) -> PyResult<&mut Pilot> {
        if self.pilot.is_null() {
            return Err(PyRuntimeError::new_err("the test app has been closed"));
        }
        // SAFETY: owned by this handle (unsendable, so single-threaded), freed in `close`.
        Ok(unsafe { &mut *self.pilot })
    }

    /// Run a pilot step on the runtime with the GIL released, then re-raise handler errors.
    fn step<R>(&self, py: Python<'_>, f: impl AsyncFnOnce(&mut Pilot) -> R) -> PyResult<R> {
        let rt = self
            .rt
            .as_ref()
            .ok_or_else(|| PyRuntimeError::new_err("the test app has been closed"))?;
        let job = AssertSend((self.pilot()?, rt, f));
        let r = py.detach(move || {
            let job = job;
            let (pilot, rt, f) = job.0;
            AssertSend(rt.block_on(f(pilot)))
        });
        if let Some(e) = bridge::take_error() {
            return Err(e);
        }
        Ok(r.0)
    }
}

#[pyfunction]
pub fn start_test(
    py: Python<'_>,
    app: &Bound<'_, PyAny>,
    config: AppConfig,
    width: i32,
    height: i32,
) -> PyResult<PilotHandle> {
    let a = build_app(py, app, config)?;
    register_running(&a)?;
    let _ = bridge::take_error();
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| PyOSError::new_err(e.to_string()))?;
    let a = AssertSend(a);
    let rtr = AssertSend(&rt);
    let pilot = py.detach(move || {
        let (a, rtr) = (a, rtr);
        AssertSend(rtr.0.block_on(a.0.test(Size::new(width, height))))
    });
    let handle = PilotHandle {
        pilot: Box::into_raw(Box::new(pilot.0)),
        rt: Some(rt),
    };
    bridge::set_pilot(Some(handle.pilot));
    if let Some(e) = bridge::take_error() {
        handle.close_inner();
        return Err(e);
    }
    Ok(handle)
}

impl PilotHandle {
    fn close_inner(&self) {
        // (called at most once: `close` nulls the pointer through `&mut self`)
        bridge::set_pilot(None);
        unregister_running();
    }
}

#[pymethods]
impl PilotHandle {
    fn close(&mut self) {
        if self.pilot.is_null() {
            return;
        }
        self.close_inner();
        // SAFETY: created by Box::into_raw in start_test; nulled below so it's freed once.
        let pilot = unsafe { Box::from_raw(self.pilot) };
        self.pilot = std::ptr::null_mut();
        // Dropping the DOM drops Python objects; we hold the GIL here.
        drop(pilot);
        if let Some(rt) = self.rt.take() {
            rt.shutdown_background();
        }
    }

    fn press(&self, py: Python<'_>, keys: Vec<String>) -> PyResult<()> {
        let keys: Vec<&str> = keys.iter().map(String::as_str).collect();
        self.step(py, async |p| p.press(&keys).await)
    }

    fn type_text(&self, py: Python<'_>, text: &str) -> PyResult<()> {
        self.step(py, async |p| p.type_text(text).await)
    }

    fn paste(&self, py: Python<'_>, text: &str) -> PyResult<()> {
        self.step(py, async |p| p.paste(text).await)
    }

    fn pause(&self, py: Python<'_>) -> PyResult<()> {
        self.step(py, async |p| p.pause().await)
    }

    /// Process pending work without waiting; returns whether anything happened.
    fn pump(&self, py: Python<'_>) -> PyResult<bool> {
        self.step(py, async |p| p.pump())
    }

    /// Whether animations or CSS transitions are running.
    fn animating(&self) -> PyResult<bool> {
        let p = self.pilot()?;
        Ok(p.animations_running() + p.transitions_running() > 0)
    }

    fn wait(&self, py: Python<'_>, seconds: f64) -> PyResult<()> {
        let d = Duration::from_secs_f64(seconds.max(0.0));
        self.step(py, async |p| p.wait(d).await)
    }

    /// Click the first widget matching `selector` (at `offset` within it, default its centre...
    /// the core clicks its top-left content cell). Returns whether a widget matched.
    #[pyo3(signature = (selector=None, offset=(0, 0)))]
    fn click(
        &self,
        py: Python<'_>,
        selector: Option<String>,
        offset: (i32, i32),
    ) -> PyResult<bool> {
        match selector {
            Some(sel) if offset == (0, 0) => self.step(py, async |p| p.click(&sel).await),
            Some(sel) => {
                let target = {
                    let p = self.pilot()?;
                    p.query(&sel)
                        .first()
                        .copied()
                        .map(|id| p.dom().get(id).map(|nd| nd.layout.region))
                };
                match target.flatten() {
                    Some(r) => {
                        self.step(py, async |p| {
                            p.click_at(r.x + offset.0, r.y + offset.1).await
                        })?;
                        Ok(true)
                    }
                    None => Ok(false),
                }
            }
            None => {
                self.step(py, async |p| p.click_at(offset.0, offset.1).await)?;
                Ok(true)
            }
        }
    }

    fn hover(&self, py: Python<'_>, selector: &str) -> PyResult<bool> {
        self.step(py, async |p| p.hover(selector).await)
    }

    fn hover_at(&self, py: Python<'_>, x: i32, y: i32) -> PyResult<()> {
        self.step(py, async |p| p.hover_at(x, y).await)
    }

    fn mouse_down(&self, py: Python<'_>, x: i32, y: i32) -> PyResult<()> {
        self.step(py, async |p| p.mouse_down(x, y).await)
    }

    fn mouse_up(&self, py: Python<'_>, x: i32, y: i32) -> PyResult<()> {
        self.step(py, async |p| p.mouse_up(x, y).await)
    }

    fn scroll_down_at(&self, py: Python<'_>, x: i32, y: i32) -> PyResult<()> {
        self.step(py, async |p| p.scroll_down_at(x, y).await)
    }

    fn scroll_up_at(&self, py: Python<'_>, x: i32, y: i32) -> PyResult<()> {
        self.step(py, async |p| p.scroll_up_at(x, y).await)
    }

    fn resize(&self, py: Python<'_>, width: i32, height: i32) -> PyResult<()> {
        self.step(py, async |p| p.resize(Size::new(width, height)).await)
    }

    fn has_exited(&self) -> PyResult<bool> {
        Ok(self.pilot()?.has_exited())
    }

    fn export_svg(&self) -> PyResult<String> {
        Ok(self.pilot()?.export_svg())
    }

    fn screen_text(&self) -> PyResult<String> {
        Ok(self.pilot()?.screen_text())
    }

    fn notifications(&self) -> PyResult<Vec<String>> {
        Ok(self.pilot()?.notifications())
    }

    fn bell_count(&self) -> PyResult<usize> {
        Ok(self.pilot()?.bell_count())
    }

    fn root(&self) -> PyResult<u64> {
        Ok(node_to_int(self.pilot()?.dom().root.expect("root")))
    }

    fn css_errors(&self) -> PyResult<Vec<String>> {
        Ok(self.pilot()?.css_errors().to_vec())
    }
}

impl Drop for PilotHandle {
    fn drop(&mut self) {
        if !self.pilot.is_null() {
            self.close_inner();
            // SAFETY: see `close`.
            drop(unsafe { Box::from_raw(self.pilot) });
            self.pilot = std::ptr::null_mut();
        }
        if let Some(rt) = self.rt.take() {
            rt.shutdown_background();
        }
    }
}

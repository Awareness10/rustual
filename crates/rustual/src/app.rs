//! The application runtime: owns the DOM, dispatches events and messages, runs timers and
//! workers, and produces frames.

use crate::binding::{Action, Binding};
use crate::compositor;
use crate::context::Ctx;
use crate::dom::{Dom, Node, NodeId};
use crate::driver::{CrosstermDriver, DriverEvent};
use crate::events::{Event, KeyEvent, MouseButton, MouseEvent};
use crate::layout::{self, Arrangement};
use crate::message::{Flow, Message};
use crate::pilot::Pilot;
use crate::screen::Screen;
use crate::widget::{Child, RenderEnv, Widget};
use rustual_css::{Display, Origin, Overflow, Styles, Theme};
use rustual_render::{Canvas, Offset, Region, Size};
use std::any::Any;
use std::collections::{HashMap, VecDeque};
use std::hash::{Hash, Hasher};
use std::io;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::Instant;

const APP_DEFAULT_CSS: &str = "
App {
    background: $background;
    color: $foreground;
}
*:disabled:can-focus {
    opacity: 0.7;
}
Widget {
    scrollbar-background: $scrollbar-background;
    scrollbar-background-hover: $scrollbar-background-hover;
    scrollbar-background-active: $scrollbar-background-active;
    scrollbar-color: $scrollbar;
    scrollbar-color-active: $scrollbar-active;
    scrollbar-color-hover: $scrollbar-hover;
    scrollbar-corner-color: $scrollbar-corner-color;
}
Screen.-maximized-view {
    hatch: right $panel;
}

/* ANSI themes (Textual `:ansi`). */
App {
    &:ansi {
        background: ansi_default;
        color: ansi_default;

        .-ansi-scrollbar {
            scrollbar-background: ansi_default;
            scrollbar-background-hover: ansi_default;
            scrollbar-background-active: ansi_default;
            scrollbar-color: ansi_blue;
            scrollbar-color-active: ansi_bright_blue;
            scrollbar-color-hover: ansi_bright_blue;
            scrollbar-corner-color: ansi_default;
        }

        .bindings-table--key {
            color: ansi_magenta;
        }
        .bindings-table--description {
            color: ansi_default;
        }

        .bindings-table--header {
            color: ansi_default;
        }

        .bindings-table--divider {
            color: transparent;
            text-style: dim;
        }
    }
}
";

/// Lines scrolled per mouse-wheel tick.
const WHEEL_LINES: i32 = 3;

/// Identifies a timer created with [`Ctx::set_timer`] / [`Ctx::set_interval`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TimerId(pub(crate) u64);

impl TimerId {
    /// The id as a number (unique within an app), e.g. to hand it to another language.
    pub fn to_raw(self) -> u64 {
        self.0
    }

    /// The inverse of [`TimerId::to_raw`]. Cancelling an id that matches no timer does nothing.
    pub fn from_raw(raw: u64) -> TimerId {
        TimerId(raw)
    }
}

pub(crate) type TimerCallback = Box<dyn FnMut(&mut dyn Widget, &mut Ctx)>;

pub(crate) struct Timer {
    pub id: TimerId,
    pub node: NodeId,
    pub deadline: Instant,
    pub interval: Option<Duration>,
    /// `None` while the callback is running.
    pub callback: Option<TimerCallback>,
}

/// Severity of a notification (`Ctx::notify`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Severity {
    #[default]
    Information,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Notification {
    pub message: String,
    pub title: String,
    pub severity: Severity,
    pub(crate) expires: Instant,
}

/// Posted to the app root whenever a notification is added, so a toast widget can react.
#[derive(Debug, Clone, PartialEq)]
pub struct Notified(pub Notification);

/// A closure sent to the app thread by [`AppHandle::call_later`] and friends.
pub(crate) type AppCall = Box<dyn FnOnce(&mut AppState) + Send>;

/// Signals sent to the app from other tasks or threads (see [`AppHandle`]).
pub(crate) enum Signal {
    Message {
        target: NodeId,
        payload: Box<dyn Any + Send>,
        bubble: bool,
    },
    Call(AppCall),
    Exit,
}

/// Most signals handled in one go before the app renders again (so a flood of messages from
/// a fast producer is batched into one frame without starving the terminal's input).
pub(crate) const SIGNAL_BATCH: usize = 1024;

/// A cloneable, `Send` handle for talking to the app from workers and other threads
/// (Textual's thread-safe `post_message` and `App.call_from_thread`).
///
/// Everything sent through a handle is handled on the app thread, in the order it was sent.
///
/// ```
/// # use rustual::prelude::*;
/// struct Reading(f64);
///
/// fn start(ctx: &mut Ctx) {
///     let handle = ctx.app_handle();
///     let me = ctx.id();
///     std::thread::spawn(move || {
///         // Fire and forget: delivered to `me`'s `on_message` (and bubbles).
///         handle.post_message(me, Reading(21.5));
///         // Run code on the app thread against a widget, without defining a message.
///         handle.call_on::<Input>(me, |input, ctx| input.set_value("21.5", ctx));
///         // Or wait for a result (Textual's `call_from_thread`).
///         let title = handle.call_from_thread(|ctx| ctx.title().to_string());
///     });
/// }
/// ```
#[derive(Clone)]
pub struct AppHandle {
    tx: mpsc::UnboundedSender<Signal>,
}

impl AppHandle {
    /// Post a message to `target` (it bubbles like any message). Returns false if the app has
    /// exited. Never blocks: the message is queued for the app thread.
    pub fn post_message<T: Any + Send>(&self, target: NodeId, payload: T) -> bool {
        self.send(Signal::Message {
            target,
            payload: Box::new(payload),
            bubble: true,
        })
    }

    /// Like [`AppHandle::post_message`] for a message that does not bubble.
    pub fn post_message_no_bubble<T: Any + Send>(&self, target: NodeId, payload: T) -> bool {
        self.send(Signal::Message {
            target,
            payload: Box::new(payload),
            bubble: false,
        })
    }

    /// Run `f` on the app thread, with a [`Ctx`] for the app root, after everything sent
    /// before it (Textual's `call_later` from a thread). Returns false if the app has exited.
    pub fn call_later(&self, f: impl FnOnce(&mut Ctx) + Send + 'static) -> bool {
        self.send(Signal::Call(Box::new(move |app: &mut AppState| {
            let mut ctx = Ctx {
                node: app.root,
                app,
            };
            f(&mut ctx);
        })))
    }

    /// Run `f` on the app thread with the widget at `node` (which must be a `W`). `f` is
    /// dropped without running if the node has been removed by then or is not a `W`.
    /// Returns false if the app has exited.
    pub fn call_on<W: Widget>(
        &self,
        node: NodeId,
        f: impl FnOnce(&mut W, &mut Ctx) + Send + 'static,
    ) -> bool {
        self.send(Signal::Call(Box::new(move |app: &mut AppState| {
            app.call_widget(node, |w, ctx| {
                if let Some(w) = crate::widget::downcast_mut::<W>(w) {
                    f(w, ctx);
                }
            });
        })))
    }

    /// Run `f` on the app thread and wait for its result, for async workers. `None` if the
    /// app exited before running it.
    pub async fn call<R: Send + 'static>(
        &self,
        f: impl FnOnce(&mut Ctx) -> R + Send + 'static,
    ) -> Option<R> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        if !self.call_later(move |ctx| {
            let _ = tx.send(f(ctx));
        }) {
            return None;
        }
        rx.await.ok()
    }

    /// Run `f` on the app thread and block until it returns (Textual's
    /// `App.call_from_thread`), for plain threads and [`Ctx::run_blocking`] workers. `None` if
    /// the app exited before running it.
    ///
    /// # Panics
    ///
    /// When called from async code (the app thread or an async worker), which would
    /// deadlock or stall the runtime; use [`AppHandle::call`] there.
    pub fn call_from_thread<R: Send + 'static>(
        &self,
        f: impl FnOnce(&mut Ctx) -> R + Send + 'static,
    ) -> Option<R> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        if !self.call_later(move |ctx| {
            let _ = tx.send(f(ctx));
        }) {
            return None;
        }
        rx.blocking_recv().ok()
    }

    /// Whether the app is still running (it accepts messages).
    pub fn is_running(&self) -> bool {
        !self.tx.is_closed()
    }

    pub fn exit(&self) {
        let _ = self.tx.send(Signal::Exit);
    }

    fn send(&self, signal: Signal) -> bool {
        self.tx.send(signal).is_ok()
    }
}

pub use crate::workers::WorkerHandle;

/// Posted (to the pusher) when a screen is dismissed without a result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScreenDismissed;

pub(crate) enum Pending {
    Mount(NodeId),
    Message {
        target: NodeId,
        msg: Message,
    },
    Event {
        target: NodeId,
        event: Event,
    },
    Action {
        target: NodeId,
        action: String,
    },
    AutoFocus(NodeId),
    PopScreen(Option<Box<dyn Any + Send>>),
    /// A closure from [`AppHandle::call_later`] (queued, to keep the order of signals).
    Call(AppCall),
}

pub(crate) struct AppState {
    pub dom: Dom,
    pub env: RenderEnv,
    pub root: NodeId,
    pub screens: Vec<NodeId>,
    pub size: Size,
    pub queue: VecDeque<Pending>,
    pub pending_removals: Vec<NodeId>,
    pub timers: Vec<Timer>,
    pub next_timer_id: u64,
    pub cancelled_timers: Vec<TimerId>,
    pub signal_tx: mpsc::UnboundedSender<Signal>,
    pub signal_rx: Option<mpsc::UnboundedReceiver<Signal>>,
    pub dirty: bool,
    pub exit_requested: bool,
    pub arrangements: HashMap<NodeId, Arrangement>,
    pub themes: Vec<Theme>,
    pub theme: String,
    /// `App.ansi_color`: native ANSI colors if `Some(true)`, truecolor if `Some(false)`,
    /// the theme's `ansi` flag if `None`.
    pub ansi_color: Option<bool>,
    /// `NO_COLOR` was set (or [`App::no_color`]).
    pub no_color: bool,
    /// The active screen at the last layout (for `ScreenSuspend` / `ScreenResume`).
    pub last_active_screen: Option<NodeId>,
    /// `HORIZONTAL_BREAKPOINTS` / `VERTICAL_BREAKPOINTS` of the app: `(min size, class)`.
    pub breakpoints: Breakpoints,
    /// Classes set on each screen by breakpoints (removed when no longer wanted).
    pub breakpoint_classes: HashMap<NodeId, Vec<String>>,
    pub app_css: Vec<(String, String)>,
    pub css_errors: Vec<String>,
    pub mouse_down_on: Option<NodeId>,
    pub mouse_pos: Option<Offset>,
    /// screen → node that pushed it.
    pub screen_pushers: HashMap<NodeId, NodeId>,
    /// screen → node focused when another screen was pushed on top.
    pub screen_focus: HashMap<NodeId, NodeId>,
    pub scroll_visible_request: Option<NodeId>,
    pub bell_count: usize,
    pub notifications: Vec<Notification>,
    pub last_frame: Option<Canvas>,
    pub started: bool,
    pub animator: crate::animation::Animator,
    pub mouse: crate::scrollbar::MouseState,
    /// Command palette, tooltip and key panel state (see `crate::ux`).
    pub ux: crate::ux::UxState,
    /// Named screens, modes, maximize and loading state (see `crate::screens`).
    pub screens_ext: crate::screens::ScreenState,
    /// Worker registry (see `crate::workers`).
    pub workers: crate::workers::Workers,
    /// Clipboard / suspend / pending terminal writes (see `crate::terminal`).
    pub terminal: crate::terminal::TerminalState,
    /// Binding id → keys (see `crate::binding::apply_keymap`).
    pub keymap: crate::binding::Keymap,
    /// Message types currently suppressed (Textual's `prevent`), innermost scope last.
    pub prevented: Vec<Vec<std::any::TypeId>>,
    /// Most frames drawn per second in a terminal ([`App::max_fps`]).
    pub max_fps: u32,
    /// Selector of the widget focused when a screen activates ([`App::auto_focus`]).
    pub auto_focus: Option<String>,
}

/// The application: build with [`App::new`], configure, then [`App::run`] (or [`App::test`]).
///
/// ```no_run
/// # use rustual::prelude::*;
/// struct Hello;
/// impl Widget for Hello {
///     fn compose(&mut self) -> Vec<Child> { compose![Label::new("Hello, World!")] }
/// }
/// #[tokio::main]
/// async fn main() -> std::io::Result<()> { App::new(Hello).run().await }
/// ```
pub struct App {
    pub(crate) state: Box<AppState>,
}

impl App {
    /// Create an app whose root widget is `root`. The root's `compose()` populates the
    /// default screen; its `bindings()`/`run_action()`/`on_message()` act at app level.
    pub fn new<W: Widget>(root: W) -> App {
        let title = std::any::type_name::<W>()
            .rsplit("::")
            .next()
            .unwrap_or("App")
            .to_string();
        let mut dom = Dom::new();
        let mut node = Node::new(Box::new(root));
        node.type_names.insert(node.type_names.len() - 1, "App");
        let root_id = dom.insert(node);
        dom.root = Some(root_id);
        if let Some(w) = dom.get_mut(root_id).and_then(|n| n.widget.as_mut()) {
            w.on_attach(root_id);
        }
        let (tx, rx) = mpsc::unbounded_channel();
        let env = RenderEnv {
            title,
            ..Default::default()
        };
        let state = AppState {
            dom,
            env,
            root: root_id,
            screens: Vec::new(),
            size: Size::new(80, 24),
            queue: VecDeque::new(),
            pending_removals: Vec::new(),
            timers: Vec::new(),
            next_timer_id: 1,
            cancelled_timers: Vec::new(),
            signal_tx: tx,
            signal_rx: Some(rx),
            dirty: true,
            exit_requested: false,
            arrangements: HashMap::new(),
            themes: Theme::builtin(),
            theme: default_theme(),
            ansi_color: None,
            no_color: std::env::var_os("NO_COLOR").is_some(),
            last_active_screen: None,
            breakpoints: Breakpoints::default(),
            breakpoint_classes: HashMap::new(),
            app_css: Vec::new(),
            css_errors: Vec::new(),
            mouse_down_on: None,
            mouse_pos: None,
            screen_pushers: HashMap::new(),
            screen_focus: HashMap::new(),
            scroll_visible_request: None,
            bell_count: 0,
            notifications: Vec::new(),
            last_frame: None,
            started: false,
            animator: Default::default(),
            mouse: Default::default(),
            ux: Default::default(),
            screens_ext: Default::default(),
            workers: Default::default(),
            terminal: Default::default(),
            keymap: Default::default(),
            prevented: Vec::new(),
            max_fps: default_max_fps(),
            auto_focus: Some("*".to_string()),
        };
        App {
            state: Box::new(state),
        }
    }

    /// Add app-level CSS (Textual's `App.CSS`). May be called multiple times.
    pub fn css(mut self, css: &str) -> Self {
        let key = format!("app-css-{}", self.state.app_css.len());
        self.state.app_css.push((key, css.to_string()));
        self
    }

    /// Load app CSS from a file (Textual's `CSS_PATH`).
    pub fn css_path(mut self, path: impl AsRef<std::path::Path>) -> io::Result<Self> {
        let path = path.as_ref();
        let css = std::fs::read_to_string(path)?;
        self.state.app_css.push((path.display().to_string(), css));
        Ok(self)
    }

    pub fn title(mut self, title: &str) -> Self {
        self.state.env.title = title.to_string();
        self
    }

    pub fn sub_title(mut self, sub_title: &str) -> Self {
        self.state.env.sub_title = sub_title.to_string();
        self
    }

    /// Select a theme by name (see [`Theme::builtin`]). The default is `TEXTUAL_THEME`, else
    /// `textual-dark`; an unknown name falls back to `textual-dark`.
    pub fn theme(mut self, name: &str) -> Self {
        self.state.theme = name.to_string();
        self
    }

    /// Native ANSI colors (`true`), ANSI colors converted to truecolor (`false`); without this,
    /// the theme decides (ANSI themes such as `ansi-dark` use native colors). Textual's
    /// `App(ansi_color=...)`.
    pub fn ansi_color(mut self, ansi_color: bool) -> Self {
        self.state.ansi_color = Some(ansi_color);
        self
    }

    /// Remove colors, as when `NO_COLOR` is set (which this overrides).
    pub fn no_color(mut self, no_color: bool) -> Self {
        self.state.no_color = no_color;
        self
    }

    /// Classes set on screens by terminal width: `(min width, class)` pairs; the class of the
    /// largest threshold not above the width applies (Textual `HORIZONTAL_BREAKPOINTS`).
    pub fn horizontal_breakpoints(mut self, breakpoints: &[(i32, &str)]) -> Self {
        self.state.breakpoints.horizontal = Some(to_breakpoints(breakpoints));
        self
    }

    /// Like [`App::horizontal_breakpoints`] for the terminal height (Textual
    /// `VERTICAL_BREAKPOINTS`).
    pub fn vertical_breakpoints(mut self, breakpoints: &[(i32, &str)]) -> Self {
        self.state.breakpoints.vertical = Some(to_breakpoints(breakpoints));
        self
    }

    pub fn register_theme(mut self, theme: Theme) -> Self {
        self.state.themes.retain(|t| t.name != theme.name);
        self.state.themes.push(theme);
        self
    }

    /// Draw at most `fps` frames a second in the terminal (default: `TEXTUAL_FPS`, else 60,
    /// as in Textual). Changes made faster than that are merged into the next frame.
    pub fn max_fps(mut self, fps: u32) -> Self {
        self.state.max_fps = fps.max(1);
        self
    }

    /// Which widget gets focus when a screen becomes active with nothing on it focused: the
    /// first focusable widget matching `selector` (default `"*"`), or none for `None` (Textual's
    /// `App.AUTO_FOCUS`). Screens can override it with [`Widget::auto_focus`].
    pub fn auto_focus(mut self, selector: Option<&str>) -> Self {
        self.state.auto_focus = selector.map(String::from);
        self
    }

    /// A `Send` handle for posting messages to the app (valid once it runs).
    pub fn handle(&self) -> AppHandle {
        self.state.handle()
    }

    /// The root node (the widget passed to [`App::new`]).
    pub fn root(&self) -> NodeId {
        self.state.root
    }

    /// CSS parse errors encountered so far (as display strings).
    pub fn css_errors(&self) -> &[String] {
        &self.state.css_errors
    }

    /// Run in the terminal until the app exits.
    ///
    /// Screenshot mode (like `textual run --screenshot`): if `TEXTUAL_SCREENSHOT=<file.svg>` is set,
    /// the app runs headless at `TEXTUAL_SIZE` (`COLSxROWS`, default `100x30`), presses the
    /// comma-separated keys in `TEXTUAL_PRESS`, waits `TEXTUAL_SCREENSHOT_DELAY` ms (default 500),
    /// writes an SVG screenshot and returns.
    pub async fn run(self) -> io::Result<()> {
        if let Ok(path) = std::env::var("TEXTUAL_SCREENSHOT") {
            return self.screenshot_to(&path).await;
        }
        let mut state = self.state;
        let mut driver = CrosstermDriver::start()?;
        // Start listening for events (including resizes) before reading the size: a resize
        // that lands in between would otherwise be lost and the app would keep the old size
        // (e.g. a tmux pane resized while the app starts).
        let events = driver.event_stream();
        state.size = driver.size();
        state.startup();
        let mut rx = state.signal_rx.take().expect("signal receiver");
        let result = run_loop(&mut state, &mut driver, &mut rx, events).await;
        state.shutdown();
        driver.stop()?;
        result
    }

    async fn screenshot_to(self, path: &str) -> io::Result<()> {
        let size = std::env::var("TEXTUAL_SIZE")
            .ok()
            .and_then(|s| {
                let (w, h) = s.split_once('x')?;
                Some(Size::new(w.trim().parse().ok()?, h.trim().parse().ok()?))
            })
            .unwrap_or(Size::new(100, 30));
        let delay = std::env::var("TEXTUAL_SCREENSHOT_DELAY")
            .ok()
            .and_then(|d| d.parse().ok())
            .unwrap_or(500);
        let mut pilot = self.test(size).await;
        if let Ok(keys) = std::env::var("TEXTUAL_PRESS") {
            let keys: Vec<&str> = keys
                .split(',')
                .map(str::trim)
                .filter(|k| !k.is_empty())
                .collect();
            pilot.press(&keys).await;
        }
        pilot.wait(Duration::from_millis(delay)).await;
        std::fs::write(path, pilot.export_svg())
    }

    /// Run on a new multi-threaded tokio runtime (for non-async `main`).
    pub fn run_blocking(self) -> io::Result<()> {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()?
            .block_on(self.run())
    }

    /// Start the app headless for testing, at the given terminal size.
    pub async fn test(self, size: Size) -> Pilot {
        let mut state = self.state;
        state.size = size;
        state.startup();
        let rx = state.signal_rx.take().expect("signal receiver");
        let mut pilot = Pilot::new(state, rx);
        pilot.pause().await;
        pilot
    }
}

async fn run_loop(
    state: &mut AppState,
    driver: &mut CrosstermDriver,
    rx: &mut mpsc::UnboundedReceiver<Signal>,
    mut events: crossterm::event::EventStream,
) -> io::Result<()> {
    use futures::StreamExt;
    // Like Textual (`TEXTUAL_FPS`), frames are drawn at most `max_fps` times a second: changes
    // arriving faster (e.g. a 1 kHz stream of messages) are merged into the next frame.
    let frame_interval = state.frame_interval();
    let mut last_draw: Option<Instant> = None;
    loop {
        state.process_queue();
        if state.exit_requested {
            break;
        }
        if crate::terminal::service_suspend(state, driver)? {
            events = driver.event_stream();
            state.resize(state.size);
            state.process_queue();
            last_draw = None;
        }
        let mut draw_at = None;
        if state.dirty {
            let now = Instant::now();
            match last_draw {
                Some(last) if now < last + frame_interval => draw_at = Some(last + frame_interval),
                _ => {
                    let frame = state.render_frame();
                    driver.draw(&frame, state.last_frame.as_ref())?;
                    state.last_frame = Some(frame);
                    last_draw = Some(now);
                }
            }
        }
        crate::terminal::flush(state, driver)?;
        let deadline = [state.next_deadline(), draw_at]
            .into_iter()
            .flatten()
            .min()
            .unwrap_or_else(|| Instant::now() + Duration::from_secs(3600));
        tokio::select! {
            ev = events.next() => {
                match ev {
                    Some(Ok(ev)) => {
                        for ev in crate::driver::convert_event(ev) {
                            if let DriverEvent::Resize(size) = ev {
                                state.last_frame = None;
                                driver.clear()?;
                                state.resize(size);
                                // Redraw the cleared screen at once.
                                last_draw = None;
                            } else {
                                state.handle_driver_event(ev);
                            }
                        }
                    }
                    Some(Err(e)) => return Err(e),
                    None => break,
                }
            }
            sig = rx.recv() => {
                if let Some(sig) = sig {
                    state.handle_signal(sig);
                    // Take everything else that is already waiting, then handle it all before
                    // the next frame.
                    for _ in 1..SIGNAL_BATCH {
                        match rx.try_recv() {
                            Ok(sig) => state.handle_signal(sig),
                            Err(_) => break,
                        }
                    }
                }
            }
            _ = tokio::time::sleep_until(deadline) => {}
        }
        // Timers fire on time even while events or signals keep the loop busy.
        state.fire_due_timers();
    }
    Ok(())
}

const DEFAULT_THEME: &str = "textual-dark";

/// `TEXTUAL_THEME`, else `textual-dark` (Textual's `constants.DEFAULT_THEME`).
fn default_theme() -> String {
    std::env::var("TEXTUAL_THEME")
        .ok()
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| DEFAULT_THEME.to_string())
}

/// `TEXTUAL_FPS` (at least 1), else 60.
fn default_max_fps() -> u32 {
    std::env::var("TEXTUAL_FPS")
        .ok()
        .and_then(|v| v.trim().parse::<u32>().ok())
        .map_or(60, |fps| fps.max(1))
}

impl AppState {
    /// Shortest time between two frames drawn in the terminal.
    pub(crate) fn frame_interval(&self) -> Duration {
        Duration::from_secs_f64(1.0 / f64::from(self.max_fps.max(1)))
    }

    // ------------------------------------------------------------------ startup

    pub(crate) fn startup(&mut self) {
        if self.started {
            return;
        }
        self.started = true;
        self.env
            .stylesheet
            .add_source("App", APP_DEFAULT_CSS, Origin::Default);
        let theme_name = self.theme.clone();
        if !self.apply_theme(&theme_name) {
            // Unknown name (e.g. a typo in `TEXTUAL_THEME`): use the default theme.
            self.apply_theme(DEFAULT_THEME);
        }
        for (key, css) in self.app_css.clone() {
            for e in self.env.stylesheet.add_source(&key, &css, Origin::App) {
                self.css_errors.push(format!("{key}:{e}"));
            }
        }
        let root = self.root;
        self.register_css(root);
        // The root's compose() goes onto the default screen.
        let children = self
            .dom
            .get_mut(root)
            .and_then(|n| n.widget.as_mut())
            .map(|w| w.compose())
            .unwrap_or_default();
        let screen = self.mount_screen(Child::new(Box::new(Screen::new())).with(children), false);
        self.screens.push(screen);
        self.queue.push_back(Pending::Mount(root));
        self.process_queue();
        self.auto_focus(screen);
        crate::screens::startup(self);
        self.dirty = true;
    }

    fn shutdown(&mut self) {
        let root = self.root;
        let ids = self.dom.descendants(root);
        for id in ids.into_iter().rev() {
            self.call_widget(id, |w, ctx| w.on_unmount(ctx));
        }
    }

    pub(crate) fn apply_theme(&mut self, name: &str) -> bool {
        let Some(theme) = self.themes.iter().find(|t| t.name == name).cloned() else {
            return false;
        };
        self.theme = name.to_string();
        self.dom.dark = theme.dark;
        self.env.dark = theme.dark;
        self.dom.ansi = self.ansi_color.unwrap_or(theme.ansi);
        self.dom.no_color = self.no_color;
        for e in self.env.stylesheet.set_variables(theme.to_variables()) {
            self.css_errors.push(e.to_string());
        }
        self.dirty = true;
        true
    }

    fn register_css(&mut self, id: NodeId) {
        let Some(node) = self.dom.get(id) else { return };
        let Some(widget) = node.widget() else { return };
        let css = widget.default_css();
        if css.is_empty() {
            return;
        }
        let key = match node.type_names.first() {
            Some(t) if *t != "Widget" && *t != "App" && *t != "Screen" => format!("default:{t}"),
            _ => {
                let mut h = std::collections::hash_map::DefaultHasher::new();
                css.hash(&mut h);
                format!("default:{:x}", h.finish())
            }
        };
        if !self.env.stylesheet.has_source(&key) {
            for e in self.env.stylesheet.add_source(&key, css, Origin::Default) {
                self.css_errors.push(format!("{key}:{e}"));
            }
        }
    }

    // ------------------------------------------------------------------ mounting

    /// Mount `child` (and its subtree) under `parent` at `index` (None = append).
    pub(crate) fn mount(&mut self, parent: NodeId, child: Child, index: Option<usize>) -> NodeId {
        let id = self.build_node(child);
        match index {
            Some(i) => self.dom.insert_child(parent, id, i),
            None => self.dom.append_child(parent, id),
        }
        self.dirty = true;
        id
    }

    pub(crate) fn build_node(&mut self, child: Child) -> NodeId {
        let Child {
            widget,
            id,
            classes,
            styles,
            disabled,
            border_title,
            border_subtitle,
            children,
            tooltip,
        } = child;
        let mut node = Node::new(widget);
        node.css_id = id;
        node.classes = classes;
        node.disabled = disabled;
        node.border_title = border_title;
        node.border_subtitle = border_subtitle;
        node.tooltip = tooltip;
        if let Some(css) = styles {
            match Styles::parse_inline(&css) {
                Ok(s) => node.inline_styles = s,
                Err(e) => self.css_errors.push(format!("inline style: {e}")),
            }
        }
        let nid = self.dom.insert(node);
        self.register_css(nid);
        let composed = self
            .dom
            .get_mut(nid)
            .and_then(|n| n.widget.as_mut())
            .map(|w| {
                w.on_attach(nid);
                w.compose()
            })
            .unwrap_or_default();
        for c in children.into_iter().chain(composed) {
            let cid = self.build_node(c);
            self.dom.append_child(nid, cid);
        }
        // Children were queued first, so they mount before their parent.
        self.queue.push_back(Pending::Mount(nid));
        nid
    }

    pub(crate) fn mount_screen(&mut self, child: Child, modal: bool) -> NodeId {
        let id = self.build_node(child);
        if let Some(n) = self.dom.get_mut(id) {
            n.is_screen = true;
            n.is_modal = modal;
            let pos = n.type_names.len() - 1;
            if !n.type_names.contains(&"Screen") {
                n.type_names.insert(pos, "Screen");
            }
            if modal && !n.type_names.contains(&"ModalScreen") {
                let pos = n
                    .type_names
                    .iter()
                    .position(|t| *t == "Screen")
                    .unwrap_or(pos);
                n.type_names.insert(pos, "ModalScreen");
            }
        }
        // (Also when the widget names itself a `ModalScreen` in its `type_names`.)
        if modal && !self.env.stylesheet.has_source("default:ModalScreen") {
            self.env.stylesheet.add_source(
                "default:ModalScreen",
                crate::screen::MODAL_SCREEN_CSS,
                Origin::Default,
            );
        }
        if !self.env.stylesheet.has_source("default:Screen") {
            self.env.stylesheet.add_source(
                "default:Screen",
                crate::screen::SCREEN_CSS,
                Origin::Default,
            );
        }
        // Every screen gets a toast rack (last child) to show notifications.
        let rack = self.build_node(Child::new(
            Box::new(crate::widgets::toast::ToastRack::new()),
        ));
        self.dom.append_child(id, rack);
        let root = self.root;
        self.dom.append_child(root, id);
        self.dirty = true;
        id
    }

    pub(crate) fn push_screen(&mut self, pusher: NodeId, child: Child, modal: bool) -> NodeId {
        if let (Some(top), Some(f)) = (self.active_screen(), self.dom.focused) {
            self.screen_focus.insert(top, f);
        }
        let id = self.mount_screen(child, modal);
        self.screens.push(id);
        self.screen_pushers.insert(id, pusher);
        self.set_focus(None);
        // Mount events are already queued; auto-focus once they have been processed.
        self.queue.push_back(Pending::AutoFocus(id));
        id
    }

    /// Pop the top screen (never the last one). Posts `result` (or [`ScreenDismissed`]) to the pusher.
    pub(crate) fn pop_screen(&mut self, result: Option<Box<dyn Any + Send>>) -> bool {
        if self.screens.len() <= 1 {
            return false;
        }
        let screen = self.screens.pop().expect("screen");
        let pusher = self.screen_pushers.remove(&screen);
        if self.last_active_screen == Some(screen) {
            // Textual sends `ScreenSuspend` before the popped screen goes away.
            self.last_active_screen = None;
            let mut msg = Message::new(screen, None, crate::events::ScreenSuspend);
            msg.bubble = false;
            self.call_widget(screen, |w, ctx| {
                w.on_message(&msg, ctx);
            });
        }
        if crate::screens::is_installed(self, screen) {
            crate::screens::suspend_screen(self, screen);
        } else {
            self.remove_now(screen);
        }
        if let Some(top) = self.active_screen() {
            let f = self
                .screen_focus
                .remove(&top)
                .filter(|f| self.dom.contains(*f));
            match f {
                Some(f) => self.set_focus(Some(f)),
                None => self.auto_focus(top),
            }
        }
        if let Some(pusher) = pusher.filter(|p| self.dom.contains(*p)) {
            let payload = result.unwrap_or_else(|| Box::new(ScreenDismissed));
            let msg = Message::from_boxed(screen, None, payload);
            self.queue.push_back(Pending::Message {
                target: pusher,
                msg,
            });
        }
        self.dirty = true;
        true
    }

    pub(crate) fn active_screen(&self) -> Option<NodeId> {
        self.screens.last().copied()
    }

    /// Screens to paint, bottom → top: the topmost non-modal screen and every modal above it.
    fn visible_screens(&self) -> Vec<NodeId> {
        let mut out = Vec::new();
        for s in self.screens.iter().rev() {
            out.push(*s);
            if !self.dom.get(*s).map(|n| n.is_modal).unwrap_or(false) {
                break;
            }
        }
        out.reverse();
        out
    }

    fn auto_focus(&mut self, screen: NodeId) {
        // Only the active screen auto-focuses (the app's `on_mount` may already have pushed
        // another screen on top of the default one), and never over a focus set on it.
        if self.active_screen() != Some(screen)
            || self
                .dom
                .focused
                .map(|f| self.dom.screen_of(f) == Some(screen))
                .unwrap_or(false)
        {
            return;
        }
        let selector = self
            .dom
            .get(screen)
            .and_then(|n| n.widget())
            .and_then(|w| w.auto_focus().map(String::from))
            .or_else(|| self.auto_focus.clone());
        let Some(selector) = selector.filter(|s| !s.trim().is_empty()) else {
            return;
        };
        let chain = self.focus_chain();
        let target = if selector.trim() == "*" {
            chain.first().copied()
        } else {
            self.dom
                .query_str(screen, &selector)
                .into_iter()
                .find(|id| chain.contains(id))
        };
        if let Some(target) = target {
            self.set_focus(Some(target));
        }
    }

    // ------------------------------------------------------------------ removal

    pub(crate) fn request_remove(&mut self, id: NodeId) {
        if !self.pending_removals.contains(&id) {
            self.pending_removals.push(id);
        }
    }

    fn apply_removals(&mut self) {
        while let Some(id) = self.pending_removals.pop() {
            if self.dom.contains(id) {
                if self.screens.contains(&id) {
                    // Removing a screen means popping it.
                    if self.active_screen() == Some(id) {
                        self.queue.push_back(Pending::PopScreen(None));
                    }
                    continue;
                }
                self.remove_now(id);
            }
        }
    }

    fn remove_now(&mut self, id: NodeId) {
        let ids = self.dom.descendants(id);
        let focused_removed = self.dom.focused.map(|f| ids.contains(&f)).unwrap_or(false);
        for n in ids.iter().rev() {
            self.call_widget(*n, |w, ctx| w.on_unmount(ctx));
        }
        self.dom.remove_subtree(id);
        let dom = &self.dom;
        let live: Vec<NodeId> = dom
            .screen_titles
            .keys()
            .copied()
            .filter(|k| dom.contains(*k))
            .collect();
        self.dom.screen_titles.retain(|k, _| live.contains(k));
        let dom = &self.dom;
        let live: Vec<NodeId> = dom
            .screen_breakpoints
            .keys()
            .copied()
            .filter(|k| dom.contains(*k))
            .collect();
        self.dom.screen_breakpoints.retain(|k, _| live.contains(k));
        self.timers.retain(|t| !ids.contains(&t.node));
        // Like Textual, a removed widget's workers are cancelled.
        self.workers.cancel_nodes(&ids);
        self.screens.retain(|s| !ids.contains(s));
        for a in self.arrangements.values_mut() {
            a.order.retain(|n| !ids.contains(n));
        }
        if self
            .mouse_down_on
            .map(|m| ids.contains(&m))
            .unwrap_or(false)
        {
            self.mouse_down_on = None;
        }
        if focused_removed {
            self.dom.focused = None;
        }
        self.dirty = true;
    }

    // ------------------------------------------------------------------ handler plumbing

    /// Check out the widget at `id`, run `f` with a [`Ctx`] for it, check it back in.
    /// Returns `None` if the node doesn't exist or its widget is already checked out.
    pub(crate) fn call_widget<R>(
        &mut self,
        id: NodeId,
        f: impl FnOnce(&mut dyn Widget, &mut Ctx) -> R,
    ) -> Option<R> {
        let mut widget = self.dom.get_mut(id)?.widget.take()?;
        let result = {
            let mut ctx = Ctx {
                app: self,
                node: id,
            };
            f(widget.as_mut(), &mut ctx)
        };
        match self.dom.get_mut(id) {
            Some(node) => {
                node.can_focus = widget.can_focus();
                node.can_focus_children = widget.can_focus_children();
                node.widget = Some(widget);
            }
            None => drop(widget),
        }
        self.dirty = true;
        Some(result)
    }

    /// Process queued mounts, messages, events and removals until the queue is empty.
    pub(crate) fn process_queue(&mut self) -> bool {
        let mut did = false;
        loop {
            self.apply_removals();
            let Some(p) = self.queue.pop_front() else {
                break;
            };
            did = true;
            match p {
                Pending::Mount(id) => {
                    if self.dom.get(id).map(|n| !n.mounted).unwrap_or(false) {
                        if let Some(n) = self.dom.get_mut(id) {
                            n.mounted = true;
                        }
                        self.call_widget(id, |w, ctx| w.on_mount(ctx));
                    }
                }
                Pending::Message { target, msg } => self.deliver_message(target, msg),
                Pending::Event { target, event } => {
                    self.dispatch_event(target, event);
                }
                Pending::Action { target, action } => {
                    self.run_action(target, &action);
                }
                Pending::AutoFocus(screen) => {
                    // Like Textual (`AUTO_FOCUS` on resume): only when nothing on the screen has
                    // focus yet, so a focus set in the screen's `on_mount` wins.
                    let has_focus = self
                        .dom
                        .focused
                        .is_some_and(|f| self.dom.screen_of(f) == Some(screen));
                    if self.active_screen() == Some(screen) && !has_focus {
                        self.auto_focus(screen);
                    }
                }
                Pending::PopScreen(result) => {
                    self.pop_screen(result);
                }
                Pending::Call(f) => {
                    f(self);
                    self.dirty = true;
                }
            }
        }
        self.apply_removals();
        did
    }

    pub(crate) fn post(&mut self, target: NodeId, msg: Message) {
        let ty = msg.payload_type_id();
        if self.prevented.iter().any(|scope| scope.contains(&ty)) {
            return;
        }
        self.queue.push_back(Pending::Message { target, msg });
    }

    fn deliver_message(&mut self, target: NodeId, msg: Message) {
        let mut cur = Some(target);
        while let Some(id) = cur {
            if !self.dom.contains(id) {
                break;
            }
            let parent = self.dom.parent(id);
            let flow = self
                .call_widget(id, |w, ctx| w.on_message(&msg, ctx))
                .unwrap_or(Flow::Continue);
            if flow == Flow::Stop || !msg.bubble {
                break;
            }
            cur = self.dom.parent(id).or(parent);
        }
    }

    /// Deliver an event to `target`, bubbling to ancestors if the event bubbles and wasn't stopped.
    /// Mouse coordinates are rebased to each receiver. Returns the final flow.
    pub(crate) fn dispatch_event(&mut self, target: NodeId, mut event: Event) -> Flow {
        let mut cur = Some(target);
        while let Some(id) = cur {
            if !self.dom.contains(id) {
                return Flow::Continue;
            }
            if let Some(region) = self.dom.get(id).map(|n| n.layout.region) {
                if let Some(m) = event.mouse_mut() {
                    m.x = m.screen_x - region.x;
                    m.y = m.screen_y - region.y;
                }
            }
            let disabled = self.dom.is_disabled(id) || self.dom.is_loading(id);
            let flow = if disabled && (event.mouse().is_some() || event.key().is_some()) {
                Flow::Continue
            } else {
                self.call_widget(id, |w, ctx| w.on_event(&event, ctx))
                    .unwrap_or(Flow::Continue)
            };
            if flow == Flow::Stop {
                return Flow::Stop;
            }
            if !event.bubbles() {
                return Flow::Continue;
            }
            cur = self.dom.parent(id);
        }
        Flow::Continue
    }

    // ------------------------------------------------------------------ signals & timers

    pub(crate) fn handle_signal(&mut self, sig: Signal) {
        match sig {
            Signal::Message {
                target,
                payload,
                bubble,
            } => {
                if self.dom.contains(target) {
                    let css = self.dom.get(target).and_then(|n| n.css_id.clone());
                    let mut msg = Message::from_boxed(target, css, payload);
                    msg.bubble = bubble;
                    self.post(target, msg);
                }
            }
            Signal::Call(f) => self.queue.push_back(Pending::Call(f)),
            Signal::Exit => self.exit_requested = true,
        }
    }

    pub(crate) fn handle(&self) -> AppHandle {
        AppHandle {
            tx: self.signal_tx.clone(),
        }
    }

    pub(crate) fn add_timer(
        &mut self,
        node: NodeId,
        delay: Duration,
        repeat: bool,
        cb: TimerCallback,
    ) -> TimerId {
        let id = TimerId(self.next_timer_id);
        self.next_timer_id += 1;
        self.timers.push(Timer {
            id,
            node,
            deadline: Instant::now() + delay,
            interval: if repeat {
                Some(delay.max(Duration::from_millis(1)))
            } else {
                None
            },
            callback: Some(cb),
        });
        id
    }

    pub(crate) fn cancel_timer(&mut self, id: TimerId) {
        let before = self.timers.len();
        self.timers.retain(|t| t.id != id || t.callback.is_none());
        if self.timers.len() == before {
            // Possibly running right now; mark so it isn't rescheduled.
            self.cancelled_timers.push(id);
        }
    }

    pub(crate) fn next_deadline(&self) -> Option<Instant> {
        let t = self
            .timers
            .iter()
            .filter(|t| t.callback.is_some())
            .map(|t| t.deadline)
            .min();
        let n = self.notifications.iter().map(|n| n.expires).min();
        [t, n, self.animator.next_frame].into_iter().flatten().min()
    }

    /// Fire all timers whose deadline has passed. Returns whether any fired.
    pub(crate) fn fire_due_timers(&mut self) -> bool {
        let now = Instant::now();
        let before = self.notifications.len();
        self.notifications.retain(|n| n.expires > now);
        let mut fired = self.notifications.len() != before;
        if fired {
            self.dirty = true;
        }
        let due: Vec<TimerId> = self
            .timers
            .iter()
            .filter(|t| t.callback.is_some() && t.deadline <= now)
            .map(|t| t.id)
            .collect();
        for tid in due {
            let Some(idx) = self.timers.iter().position(|t| t.id == tid) else {
                continue;
            };
            let node = self.timers[idx].node;
            let mut cb = self.timers[idx].callback.take().expect("timer callback");
            self.call_widget(node, |w, ctx| cb(w, ctx));
            fired = true;
            let cancelled = self.cancelled_timers.contains(&tid);
            self.cancelled_timers.retain(|c| *c != tid);
            if let Some(idx) = self.timers.iter().position(|t| t.id == tid) {
                match self.timers[idx].interval {
                    Some(iv) if !cancelled => {
                        let t = &mut self.timers[idx];
                        // Skip missed ticks rather than firing a burst.
                        t.deadline += iv;
                        if t.deadline <= now {
                            t.deadline = now + iv;
                        }
                        t.callback = Some(cb);
                    }
                    _ => {
                        self.timers.remove(idx);
                    }
                }
            }
        }
        fired |= self.tick_animations(now, false);
        fired
    }

    // ------------------------------------------------------------------ focus

    /// Focusable nodes of the active screen in document order.
    pub(crate) fn focus_chain(&self) -> Vec<NodeId> {
        let Some(screen) = self.active_screen() else {
            return Vec::new();
        };
        let mut out = Vec::new();
        let mut stack = vec![crate::screens::focus_root(self, screen)];
        while let Some(id) = stack.pop() {
            let Some(node) = self.dom.get(id) else {
                continue;
            };
            if node.styles.get_display() == Display::None || node.disabled || node.loading {
                continue;
            }
            if node.can_focus && id != screen {
                out.push(id);
            }
            if node.can_focus_children {
                stack.extend(node.children.iter().rev());
            }
        }
        out
    }

    pub(crate) fn set_focus(&mut self, id: Option<NodeId>) {
        let id = id.filter(|i| self.dom.contains(*i));
        if self.dom.focused == id {
            return;
        }
        let old = self.dom.focused;
        self.dom.focused = id;
        if let Some(old) = old {
            self.queue.push_back(Pending::Event {
                target: old,
                event: Event::Blur,
            });
            if let Some(parent) = self.dom.parent(old) {
                self.post(
                    parent,
                    Message::new(old, None, crate::events::DescendantBlur { widget: old }),
                );
            }
        }
        if let Some(new) = id {
            self.queue.push_back(Pending::Event {
                target: new,
                event: Event::Focus,
            });
            if let Some(parent) = self.dom.parent(new) {
                self.post(
                    parent,
                    Message::new(new, None, crate::events::DescendantFocus { widget: new }),
                );
            }
            self.scroll_visible_request = Some(new);
        }
        self.dirty = true;
    }

    pub(crate) fn focus_next(&mut self, forward: bool) {
        let chain = self.focus_chain();
        if chain.is_empty() {
            return;
        }
        let pos = self.dom.focused.and_then(|f| {
            chain.iter().position(|c| *c == f).or_else(|| {
                // Focused node inside a non-focusable-children container: find nearest.
                chain.iter().position(|c| self.dom.is_ancestor(*c, f))
            })
        });
        let next = match (pos, forward) {
            (None, true) => 0,
            (None, false) => chain.len() - 1,
            (Some(p), true) => (p + 1) % chain.len(),
            (Some(p), false) => (p + chain.len() - 1) % chain.len(),
        };
        self.set_focus(Some(chain[next]));
    }

    // ------------------------------------------------------------------ input

    pub(crate) fn handle_driver_event(&mut self, ev: DriverEvent) {
        match ev {
            DriverEvent::Key(k) => self.handle_key(k),
            DriverEvent::Mouse(kind, m) => self.handle_mouse(kind, m),
            DriverEvent::Paste(s) => {
                let target = self.dom.focused.or(self.active_screen());
                if let Some(t) = target {
                    self.dispatch_event(t, Event::Paste(s));
                }
            }
            DriverEvent::Resize(size) => self.resize(size),
        }
        self.process_queue();
    }

    pub(crate) fn resize(&mut self, size: Size) {
        self.size = size;
        self.dirty = true;
        let mut targets = vec![self.root];
        targets.extend(self.screens.iter().copied());
        for t in targets {
            self.queue.push_back(Pending::Event {
                target: t,
                event: Event::Resize(size),
            });
        }
    }

    /// Nodes whose bindings apply, innermost first: focused → ancestors → app root.
    fn binding_chain(&self) -> Vec<NodeId> {
        self.binding_chain_for(self.active_screen())
    }

    /// The binding chain of `screen`, from the widget focused on it (its saved focus when it
    /// isn't the active screen).
    fn binding_chain_for(&self, screen: Option<NodeId>) -> Vec<NodeId> {
        let focus = if screen == self.active_screen() {
            self.dom.focused
        } else {
            screen.and_then(|s| self.screen_focus.get(&s).copied())
        };
        let start = focus.or(screen).unwrap_or(self.root);
        let mut chain = self.dom.ancestors_with_self(start);
        if !chain.contains(&self.root) {
            chain.push(self.root);
        }
        chain
    }

    /// The chain up to (and including) a modal screen (Textual's `_modal_binding_chain`): only
    /// priority bindings reach past a modal to the app.
    fn modal_chain(&self, mut chain: Vec<NodeId>) -> Vec<NodeId> {
        if let Some(i) = chain
            .iter()
            .position(|id| self.dom.get(*id).is_some_and(|n| n.is_modal))
        {
            chain.truncate(i + 1);
        }
        chain
    }

    /// The bindings of each node in `chain` (innermost first), without the keys that a node
    /// nearer the focus would consume itself (Textual's `Screen._binding_chain` filtering with
    /// `check_consume_key`): with an input focused, a printable-key binding on the app neither
    /// shows in the footer nor runs.
    fn chain_bindings(&self, chain: &[NodeId]) -> Vec<(NodeId, Vec<Binding>)> {
        let consumes = |node: NodeId, key: &str| {
            self.dom
                .get(node)
                .and_then(|n| n.widget())
                .is_some_and(|w| w.check_consume_key(key, crate::binding::key_to_character(key)))
        };
        let mut result: Vec<(NodeId, Vec<Binding>)> = Vec::with_capacity(chain.len());
        for &id in chain {
            let mut bindings = self.node_bindings(id);
            if !result.is_empty() {
                bindings.retain_mut(|b| {
                    b.keys
                        .retain(|k| !result.iter().any(|(inner, _)| consumes(*inner, k)));
                    !b.keys.is_empty()
                });
            }
            result.push((id, bindings));
        }
        result
    }

    fn node_bindings(&self, id: NodeId) -> Vec<Binding> {
        let mut b = self
            .dom
            .get(id)
            .and_then(|n| n.widget())
            .map(|w| w.bindings())
            .unwrap_or_default();
        if let Some(n) = self.dom.get(id) {
            b.extend(n.extra_bindings.iter().cloned());
        }
        // Built-in bindings are defaults, listed first: a key the widget binds itself replaces
        // the default in its place (as a subclass's `BINDINGS` replace the base class's in
        // Textual, which is why a custom `ctrl+q` binding comes first in the footer).
        let mut defaults = Vec::new();
        if id == self.root {
            defaults.push(Binding::new("ctrl+q", "quit", "Quit").hidden().priority());
            defaults.push(Binding::new("ctrl+c", "quit", "Quit").hidden());
            if self.ux.palette_enabled {
                defaults.push(crate::command::palette_binding());
            }
        }
        if self.dom.get(id).map(|n| n.is_screen).unwrap_or(false) {
            defaults.push(Binding::new("tab", "app.focus_next", "Focus next").hidden());
            defaults
                .push(Binding::new("shift+tab", "app.focus_previous", "Focus previous").hidden());
        }
        // (binding, is a default)
        let mut merged: Vec<Option<(Binding, bool)>> =
            defaults.into_iter().map(|d| Some((d, true))).collect();
        for own in b {
            let mut slot = None;
            for (i, entry) in merged.iter_mut().enumerate() {
                if entry.as_ref().is_some_and(|(d, is_default)| {
                    *is_default && d.keys.iter().any(|k| own.keys.contains(k))
                }) {
                    *entry = None;
                    slot.get_or_insert(i);
                }
            }
            match slot {
                Some(i) => merged[i] = Some((own, false)),
                None => merged.push(Some((own, false))),
            }
        }
        let mut b: Vec<Binding> = merged.into_iter().flatten().map(|(b, _)| b).collect();
        crate::binding::apply_keymap(&mut b, &self.keymap);
        b
    }

    pub(crate) fn handle_key(&mut self, key: KeyEvent) {
        self.dom.active_screen = self.active_screen();
        crate::tooltip::clear(self);
        if crate::screens::handle_key(self, &key) {
            return;
        }
        let chain = self.binding_chain();
        // 1. priority bindings (app first, like Textual)
        for (id, bindings) in self.chain_bindings(&chain).into_iter().rev() {
            for b in bindings {
                if b.priority
                    && b.matches(&key.key)
                    && self.action_state(id, &b.action) == Some(true)
                    && self.run_action(id, &b.action)
                {
                    return;
                }
            }
        }
        // 2. the focused widget (bubbling)
        let target = self
            .dom
            .focused
            .or(self.active_screen())
            .unwrap_or(self.root);
        if self.dispatch_event(target, Event::Key(key.clone())) == Flow::Stop {
            return;
        }
        // 3. regular bindings along the chain (stopping at a modal screen)
        let chain = self.modal_chain(self.binding_chain());
        for (id, bindings) in self.chain_bindings(&chain) {
            for b in bindings {
                if !b.priority
                    && b.matches(&key.key)
                    && self.action_state(id, &b.action) == Some(true)
                    && self.run_action(id, &b.action)
                {
                    return;
                }
            }
        }
    }

    pub(crate) fn handle_mouse(&mut self, kind: MouseKind, m: MouseEvent) {
        self.mouse_pos = Some(Offset::new(m.screen_x, m.screen_y));
        if self.scrollbar_mouse(kind, &m) {
            return;
        }
        let under = self.node_at(m.screen_x, m.screen_y);
        crate::tooltip::on_mouse(self, kind, under);
        // Mouse capture / implicit drag capture (see `scrollbar.rs`).
        let target = self.mouse_route(kind, &m, under);
        match kind {
            MouseKind::Move => {
                self.update_hover(self.mouse.captured.or(under));
                if let Some(t) = target {
                    self.dispatch_event(t, Event::MouseMove(m));
                }
            }
            MouseKind::Down => {
                self.update_hover(self.mouse.captured.or(under));
                let Some(t) = target else { return };
                if m.button == Some(MouseButton::Left) {
                    let focus_target = self.dom.ancestors_with_self(t).into_iter().find(|n| {
                        self.dom.get(*n).map(|n| n.can_focus).unwrap_or(false)
                            && !self.dom.is_disabled(*n)
                    });
                    if let Some(f) = focus_target {
                        if self.focus_chain().contains(&f) {
                            self.set_focus(Some(f));
                        }
                    }
                }
                self.mouse_down_on = Some(t);
                self.dispatch_event(t, Event::MouseDown(m));
            }
            MouseKind::Up => {
                let down = self.mouse_down_on.take();
                let Some(t) = target else { return };
                self.dispatch_event(t, Event::MouseUp(m));
                let Some(t) = under else { return };
                if let Some(d) = down {
                    // Click goes to the nearest common node of press and release.
                    let click_target = self
                        .dom
                        .ancestors_with_self(t)
                        .into_iter()
                        .find(|a| *a == d || self.dom.is_ancestor(*a, d));
                    if let Some(ct) = click_target {
                        self.register_click(Offset::new(m.screen_x, m.screen_y));
                        // `[@click=action]` markup under the pointer runs its action instead of bubbling.
                        if ct != t || !crate::ux::click_link(self, t, m) {
                            self.dispatch_event(ct, Event::Click(m));
                        }
                    }
                }
            }
            MouseKind::ScrollUp
            | MouseKind::ScrollDown
            | MouseKind::ScrollLeft
            | MouseKind::ScrollRight => {
                let Some(t) = target else { return };
                let (event, dx, dy) = match kind {
                    MouseKind::ScrollUp => (Event::MouseScrollUp(m), 0, -WHEEL_LINES),
                    MouseKind::ScrollDown => (Event::MouseScrollDown(m), 0, WHEEL_LINES),
                    MouseKind::ScrollLeft => (Event::MouseScrollLeft(m), -WHEEL_LINES, 0),
                    _ => (Event::MouseScrollRight(m), WHEEL_LINES, 0),
                };
                if self.dispatch_event(t, event) == Flow::Continue {
                    self.scroll_nearest(t, dx, dy);
                }
            }
        }
    }

    fn update_hover(&mut self, target: Option<NodeId>) {
        if self.dom.hovered == target {
            return;
        }
        if let Some(old) = self.dom.hovered {
            self.queue.push_back(Pending::Event {
                target: old,
                event: Event::Leave,
            });
        }
        if let Some(new) = target {
            self.queue.push_back(Pending::Event {
                target: new,
                event: Event::Enter,
            });
        }
        self.dom.hovered = target;
        self.dirty = true;
    }

    pub(crate) fn node_at(&self, x: i32, y: i32) -> Option<NodeId> {
        let screen = self.active_screen()?;
        self.arrangements.get(&screen)?.node_at(&self.dom, x, y)
    }

    /// Scroll the innermost scrollable node at or above `from` that can move in the given direction.
    fn scroll_nearest(&mut self, from: NodeId, dx: i32, dy: i32) -> bool {
        for id in self.dom.ancestors_with_self(from) {
            let Some(node) = self.dom.get(id) else {
                continue;
            };
            let max = scrollable_max(node);
            let cur = node.scroll_offset;
            let next = Offset::new(cur.x + dx, cur.y + dy).clamp(max.x, max.y);
            if next != cur {
                self.animator.cancel_scroll(id);
                if let Some(n) = self.dom.get_mut(id) {
                    n.scroll_offset = next;
                }
                self.dirty = true;
                return true;
            }
        }
        false
    }

    // ------------------------------------------------------------------ actions

    /// Run an action string on behalf of `owner` (the node whose binding triggered it).
    /// `Widget::check_action` of the widget an action string resolves to (from `owner`).
    pub(crate) fn action_state(&self, owner: NodeId, action: &str) -> Option<bool> {
        let parsed = Action::parse(action);
        let target = match parsed.namespace.as_deref() {
            Some("app") => self.root,
            Some("screen") => self.active_screen().unwrap_or(self.root),
            Some("focused") => match self.dom.focused {
                Some(f) => f,
                None => return Some(true),
            },
            _ => owner,
        };
        self.dom
            .get(target)
            .and_then(|n| n.widget())
            .map(|w| w.check_action(&parsed.name, &parsed.args, &self.dom))
            .unwrap_or(Some(true))
    }

    pub(crate) fn run_action(&mut self, owner: NodeId, action: &str) -> bool {
        let parsed = Action::parse(action);
        let target = match parsed.namespace.as_deref() {
            Some("app") => self.root,
            Some("screen") => self.active_screen().unwrap_or(self.root),
            Some("focused") => match self.dom.focused {
                Some(f) => f,
                None => return false,
            },
            _ => owner,
        };
        let handled = self
            .call_widget(target, |w, ctx| {
                w.run_action(&parsed.name, &parsed.args, ctx)
            })
            .unwrap_or(false);
        if handled {
            return true;
        }
        self.builtin_action(target, &parsed)
    }

    fn builtin_action(&mut self, target: NodeId, action: &Action) -> bool {
        let arg = |i: usize| action.args.get(i).map(String::as_str).unwrap_or("");
        match action.name.as_str() {
            "quit" => self.exit_requested = true,
            "bell" => self.bell_count += 1,
            "toggle_dark" => {
                let dark = !self.dom.dark;
                let current = self.theme.clone();
                let alt = if dark {
                    "textual-dark"
                } else {
                    "textual-light"
                };
                if current != alt {
                    self.apply_theme(alt);
                }
            }
            "set_theme" => return self.apply_theme(arg(0)),
            "focus_next" => self.focus_next(true),
            "focus_previous" => self.focus_next(false),
            "pop_screen" | "dismiss" | "back" => {
                if self.screens.len() <= 1 {
                    return false;
                }
                self.queue.push_back(Pending::PopScreen(None));
            }
            "focus" => {
                let found = self.dom.find_by_css_id(self.root, arg(0));
                if found.is_none() {
                    return false;
                }
                self.set_focus(found);
            }
            "add_class" | "remove_class" | "toggle_class" => {
                let nodes = self.dom.query_str(self.root, arg(0));
                for n in nodes {
                    if let Some(node) = self.dom.get_mut(n) {
                        match action.name.as_str() {
                            "add_class" => node.add_class(arg(1)),
                            "remove_class" => node.remove_class(arg(1)),
                            _ => node.toggle_class(arg(1)),
                        }
                    }
                }
            }
            "notify" => self.notify(
                arg(0).to_string(),
                arg(1).to_string(),
                Severity::Information,
                None,
            ),
            "scroll_up" | "scroll_down" | "scroll_left" | "scroll_right" | "scroll_home"
            | "scroll_end" | "page_up" | "page_down" => {
                let Some(node) = self.dom.get(target) else {
                    return false;
                };
                let max = node.layout.max_scroll();
                let page = node.layout.content_region.height.max(1);
                // Relative to where an animated scroll is heading (Textual's `scroll_target_y`).
                let cur = self.scroll_target(target);
                let next = match action.name.as_str() {
                    "scroll_up" => Offset::new(cur.x, cur.y - 1),
                    "scroll_down" => Offset::new(cur.x, cur.y + 1),
                    "scroll_left" => Offset::new(cur.x - 1, cur.y),
                    "scroll_right" => Offset::new(cur.x + 1, cur.y),
                    "scroll_home" => Offset::new(cur.x, 0),
                    "scroll_end" => Offset::new(cur.x, max.y),
                    "page_up" => Offset::new(cur.x, cur.y - page),
                    _ => Offset::new(cur.x, cur.y + page),
                };
                // Animated like Textual's `scroll_*(animate=True)`.
                self.scroll_animated(
                    target,
                    next.clamp(max.x, max.y),
                    None,
                    crate::animation::Easing::DEFAULT_SCROLL,
                );
            }
            _ => {
                return crate::ux::builtin_action(self, action)
                    || crate::screens::builtin_action(self, target, action)
                    || crate::terminal::builtin_action(self, action)
            }
        }
        self.dirty = true;
        true
    }

    pub(crate) fn notify(
        &mut self,
        message: String,
        title: String,
        severity: Severity,
        timeout: Option<Duration>,
    ) {
        let n = Notification {
            message,
            title,
            severity,
            expires: Instant::now() + timeout.unwrap_or(Duration::from_secs(5)),
        };
        self.notifications.push(n.clone());
        let root = self.root;
        // Let every screen's toast rack show the new notification.
        for rack in self.dom.query_str(root, "ToastRack") {
            let mut msg = Message::new(root, None, Notified(n.clone()));
            msg.bubble = false;
            self.post(rack, msg);
        }
        self.post(root, Message::new(root, None, Notified(n)));
        self.dirty = true;
    }

    /// Forget a notification (e.g. a dismissed toast). Removes the first equal one.
    pub(crate) fn unnotify(&mut self, n: &Notification) {
        if let Some(i) = self.notifications.iter().position(|x| x == n) {
            self.notifications.remove(i);
            self.dirty = true;
        }
    }

    // ------------------------------------------------------------------ frames

    /// Recompute styles for the whole tree (cascade + inline + inheritance).
    pub(crate) fn update_styles(&mut self) {
        let now = Instant::now();
        let mut stack = vec![(self.root, None::<NodeId>)];
        while let Some((id, parent)) = stack.pop() {
            let mut styles = self.env.stylesheet.compute(self.dom.node_ref(id));
            let Some(node) = self.dom.get(id) else {
                continue;
            };
            styles.merge(&node.inline_styles);
            // CSS transitions: overlay interpolated values (see `crate::transition`).
            crate::transition::apply(
                &mut self.animator,
                &self.dom,
                id,
                &mut styles,
                self.size,
                now,
            );
            if let Some(p) = parent.and_then(|p| self.dom.get(p)) {
                styles.inherit_from(&p.styles);
            }
            let children = node.children.clone();
            if let Some(node) = self.dom.get_mut(id) {
                node.styles = styles;
            }
            for c in children.into_iter().rev() {
                stack.push((c, Some(id)));
            }
        }
        crate::transition::end_frame(&mut self.animator, &self.dom, now);
    }

    /// Compute the bindings shown in footers and the key panel, for every visible screen.
    fn update_active_bindings(&mut self) {
        let active_screen = self.active_screen();
        let mut per_screen = HashMap::new();
        for screen in self.visible_screens() {
            let bindings = self.screen_bindings(Some(screen));
            if Some(screen) == active_screen {
                self.env.active_bindings = bindings.active.clone();
                self.env.all_bindings = bindings.all.clone();
            }
            per_screen.insert(screen, bindings);
        }
        if active_screen.is_none() {
            let bindings = self.screen_bindings(None);
            self.env.active_bindings = bindings.active;
            self.env.all_bindings = bindings.all;
        }
        self.env.screen_bindings = per_screen;
    }

    /// Textual's `Screen.active_bindings` (8.x): one binding per key, from the widget focused
    /// on `screen` up to the app (or the modal screen); the first one wins, unless a later one
    /// is a priority binding (it then takes the earlier one's place). Hidden bindings take part
    /// too: a hidden binding near the focus hides a shown one for the same key further up.
    fn screen_bindings(&self, screen: Option<NodeId>) -> crate::widget::ScreenBindings {
        let mut seen_all: Vec<String> = Vec::new();
        let mut active: Vec<(String, Binding)> = Vec::new();
        let mut all_groups: Vec<Vec<Binding>> = Vec::new();
        let chain = self.modal_chain(self.binding_chain_for(screen));
        for (id, bindings) in self.chain_bindings(&chain) {
            let mut all_group = Vec::new();
            for mut b in bindings {
                match self.action_state(id, &b.action) {
                    Some(false) => continue,
                    None => b.disabled = true,
                    Some(true) => {}
                }
                let k = b.keys.first().cloned().unwrap_or_default();
                if !seen_all.contains(&k) {
                    seen_all.push(k.clone());
                    all_group.push(b.clone());
                }
                match active.iter_mut().find(|(key, _)| *key == k) {
                    Some((_, existing)) => {
                        if b.priority && !existing.priority {
                            *existing = b;
                        }
                    }
                    None => active.push((k, b)),
                }
            }
            all_groups.push(all_group);
        }
        // The footer shows them in that order (focused widget first); the key panel lists the
        // app first.
        all_groups.reverse();
        crate::widget::ScreenBindings {
            active: active
                .into_iter()
                .map(|(_, b)| b)
                .filter(|b| b.show)
                .collect(),
            all: all_groups,
        }
    }

    pub(crate) fn layout(&mut self) {
        self.dom.active_screen = self.active_screen();
        self.post_screen_activation();
        self.apply_breakpoints();
        self.update_styles();
        crate::screens::adjust_styles(self);
        self.update_active_bindings();
        self.env.notifications = self.notifications.clone();
        let size = self.size;
        self.arrangements.clear();
        for s in self.visible_screens() {
            let arr = layout::layout_screen(&mut self.dom, &self.env, s, size);
            self.arrangements.insert(s, arr);
        }
        if let Some(target) = self.scroll_visible_request.take() {
            if self.scroll_into_view(target) {
                for s in self.visible_screens() {
                    let arr = layout::layout_screen(&mut self.dom, &self.env, s, size);
                    self.arrangements.insert(s, arr);
                }
            }
        }
    }

    /// Adjust scrollable ancestors so `target`'s region is visible. Returns whether anything moved.
    pub(crate) fn scroll_into_view(&mut self, target: NodeId) -> bool {
        let Some(mut region) = self.dom.get(target).map(|n| n.layout.region) else {
            return false;
        };
        let mut moved = false;
        for anc in self.dom.ancestors(target) {
            let Some(node) = self.dom.get(anc) else {
                continue;
            };
            let max = scrollable_max(node);
            if max == Offset::ZERO {
                continue;
            }
            let viewport = node.layout.content_region;
            let delta = viewport.get_scroll_to_visible(&region);
            if delta != Offset::ZERO {
                let cur = node.scroll_offset;
                let next = (cur + delta).clamp(max.x, max.y);
                let applied = next - cur;
                if applied != Offset::ZERO {
                    if let Some(n) = self.dom.get_mut(anc) {
                        n.scroll_offset = next;
                    }
                    region = region.translate(-applied);
                    moved = true;
                }
            }
        }
        moved
    }

    /// Restyle, lay out and paint a full frame.
    pub(crate) fn render_frame(&mut self) -> Canvas {
        self.layout();
        let screens = self.visible_screens();
        let layers: Vec<(NodeId, &Arrangement)> = screens
            .iter()
            .filter_map(|s| self.arrangements.get(s).map(|a| (*s, a)))
            .collect();
        let mut canvas = compositor::render(&self.dom, &self.env, &layers, self.size);
        crate::filter::apply(&mut canvas, self.output_mode());
        self.dirty = false;
        canvas
    }

    /// How frames are written: native ANSI colors, dark theme, `NO_COLOR`.
    pub(crate) fn output_mode(&self) -> crate::filter::OutputMode {
        crate::filter::OutputMode {
            native_ansi: self.dom.ansi,
            dark: self.dom.dark,
            no_color: self.no_color,
        }
    }

    /// The current frame for a screenshot: ANSI colors shown with the terminal theme.
    pub(crate) fn screenshot_frame(&mut self) -> Canvas {
        let mut canvas = self.render_frame();
        crate::filter::to_truecolor(&mut canvas);
        canvas
    }

    /// Change `ansi_color` at runtime (`None`: follow the theme).
    pub(crate) fn set_ansi_color(&mut self, ansi_color: Option<bool>) {
        self.ansi_color = ansi_color;
        let theme_ansi = self
            .themes
            .iter()
            .find(|t| t.name == self.theme)
            .is_some_and(|t| t.ansi);
        self.dom.ansi = ansi_color.unwrap_or(theme_ansi);
        self.dirty = true;
    }

    /// Breakpoint classes on every screen for the terminal size (Textual `Screen._on_resize`):
    /// per list, the class of the largest threshold not above the size is set and the list's
    /// other classes are removed. A screen's own lists replace the app's.
    fn apply_breakpoints(&mut self) {
        let size = self.size;
        for screen in self.screens.clone() {
            let own = self.dom.screen_breakpoints.get(&screen);
            let horizontal = own
                .and_then(|b| b.horizontal.clone())
                .or_else(|| self.breakpoints.horizontal.clone())
                .unwrap_or_default();
            let vertical = own
                .and_then(|b| b.vertical.clone())
                .or_else(|| self.breakpoints.vertical.clone())
                .unwrap_or_default();
            let Some(node) = self.dom.get_mut(screen) else {
                continue;
            };
            let mut wanted: Vec<String> = Vec::new();
            let mut listed: Vec<&String> = Vec::new();
            for (list, dimension) in [(&horizontal, size.width), (&vertical, size.height)] {
                let active = list
                    .iter()
                    .filter(|(min, _)| dimension >= *min)
                    .max_by_key(|(min, _)| *min);
                wanted.extend(active.map(|(_, class)| class.clone()));
                listed.extend(list.iter().map(|(_, class)| class));
            }
            let previous = self.breakpoint_classes.remove(&screen).unwrap_or_default();
            for class in listed.into_iter().chain(previous.iter()) {
                if !wanted.contains(class) && node.has_class(class) {
                    node.remove_class(class);
                    self.dirty = true;
                }
            }
            for class in &wanted {
                if !node.has_class(class) {
                    node.add_class(class);
                    self.dirty = true;
                }
            }
            if !wanted.is_empty() {
                self.breakpoint_classes.insert(screen, wanted);
            }
        }
    }

    /// `ScreenSuspend` / `ScreenResume` when the active screen changed since the last layout.
    fn post_screen_activation(&mut self) {
        let active = self.dom.active_screen;
        if active == self.last_active_screen {
            return;
        }
        let previous = std::mem::replace(&mut self.last_active_screen, active);
        for (target, resume) in [(previous, false), (active, true)] {
            let Some(target) = target.filter(|t| self.dom.contains(*t)) else {
                continue;
            };
            let mut msg = if resume {
                Message::new(target, None, crate::events::ScreenResume)
            } else {
                Message::new(target, None, crate::events::ScreenSuspend)
            };
            msg.bubble = false;
            self.post(target, msg);
        }
    }

    /// Region of a node from the last layout.
    pub(crate) fn region_of(&self, id: NodeId) -> Region {
        self.dom
            .get(id)
            .map(|n| n.layout.region)
            .unwrap_or_default()
    }
}

/// Max scroll offset on the axes a user/focus may scroll: only axes with `overflow: auto | scroll`
/// (like Textual's `allow_horizontal_scroll` / `allow_vertical_scroll`).
fn scrollable_max(node: &Node) -> Offset {
    let max = node.layout.max_scroll();
    let x = if node.styles.get_overflow_x() == Overflow::Hidden {
        0
    } else {
        max.x
    };
    let y = if node.styles.get_overflow_y() == Overflow::Hidden {
        0
    } else {
        max.y
    };
    Offset::new(x, y)
}

/// Kinds of mouse input from the driver.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseKind {
    Down,
    Up,
    Move,
    ScrollUp,
    ScrollDown,
    ScrollLeft,
    ScrollRight,
}

/// Breakpoint lists: `(min size, class)`; `None` means "not set" (a screen inherits the app's).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Breakpoints {
    pub horizontal: Option<Vec<(i32, String)>>,
    pub vertical: Option<Vec<(i32, String)>>,
}

fn to_breakpoints(breakpoints: &[(i32, &str)]) -> Vec<(i32, String)> {
    breakpoints
        .iter()
        .map(|(min, class)| (*min, class.to_string()))
        .collect()
}

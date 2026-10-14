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
use std::any::Any;
use std::collections::{HashMap, VecDeque};
use std::hash::{Hash, Hasher};
use std::io;
use std::time::Duration;
use textual_css::{Display, Origin, Styles, Theme};
use textual_render::{Canvas, Offset, Region, Size};
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
";

/// Lines scrolled per mouse-wheel tick.
const WHEEL_LINES: i32 = 3;

/// Identifies a timer created with [`Ctx::set_timer`] / [`Ctx::set_interval`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TimerId(pub(crate) u64);

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

/// Signals sent to the app from other tasks or threads (see [`AppHandle`]).
pub(crate) enum Signal {
    Message {
        target: NodeId,
        payload: Box<dyn Any + Send>,
    },
    Exit,
}

/// A cloneable, `Send` handle for talking to the app from workers and other threads.
#[derive(Clone)]
pub struct AppHandle {
    tx: mpsc::UnboundedSender<Signal>,
}

impl AppHandle {
    /// Post a message to `target` (it bubbles like any message). Returns false if the app has exited.
    pub fn post_message<T: Any + Send>(&self, target: NodeId, payload: T) -> bool {
        self.tx
            .send(Signal::Message {
                target,
                payload: Box::new(payload),
            })
            .is_ok()
    }

    pub fn exit(&self) {
        let _ = self.tx.send(Signal::Exit);
    }
}

/// Handle to a running worker; dropping it does not cancel the worker.
pub struct WorkerHandle {
    pub(crate) join: tokio::task::JoinHandle<()>,
}

impl WorkerHandle {
    pub fn cancel(&self) {
        self.join.abort();
    }

    pub fn is_finished(&self) -> bool {
        self.join.is_finished()
    }
}

/// Posted (to the pusher) when a screen is dismissed without a result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScreenDismissed;

pub(crate) enum Pending {
    Mount(NodeId),
    Message { target: NodeId, msg: Message },
    Event { target: NodeId, event: Event },
    Action { target: NodeId, action: String },
    AutoFocus(NodeId),
    PopScreen(Option<Box<dyn Any + Send>>),
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
}

/// The application: build with [`App::new`], configure, then [`App::run`] (or [`App::test`]).
///
/// ```ignore
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
            theme: "textual-dark".to_string(),
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

    /// Select a theme by name (see [`Theme::builtin`]).
    pub fn theme(mut self, name: &str) -> Self {
        self.state.theme = name.to_string();
        self
    }

    pub fn register_theme(mut self, theme: Theme) -> Self {
        self.state.themes.retain(|t| t.name != theme.name);
        self.state.themes.push(theme);
        self
    }

    /// CSS parse errors encountered so far (as display strings).
    pub fn css_errors(&self) -> &[String] {
        &self.state.css_errors
    }

    /// Run in the terminal until the app exits.
    pub async fn run(self) -> io::Result<()> {
        let mut state = self.state;
        let mut driver = CrosstermDriver::start()?;
        state.size = driver.size();
        state.startup();
        let mut rx = state.signal_rx.take().expect("signal receiver");
        let result = run_loop(&mut state, &mut driver, &mut rx).await;
        state.shutdown();
        driver.stop()?;
        result
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
) -> io::Result<()> {
    use futures::StreamExt;
    let mut events = driver.event_stream();
    loop {
        state.process_queue();
        if state.exit_requested {
            break;
        }
        if state.dirty {
            let frame = state.render_frame();
            driver.draw(&frame, state.last_frame.as_ref())?;
            state.last_frame = Some(frame);
        }
        let deadline = state
            .next_deadline()
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
                if let Some(sig) = sig { state.handle_signal(sig); }
            }
            _ = tokio::time::sleep_until(deadline) => {
                state.fire_due_timers();
            }
        }
    }
    Ok(())
}

impl AppState {
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
        self.apply_theme(&theme_name);
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

    fn build_node(&mut self, child: Child) -> NodeId {
        let Child {
            widget,
            id,
            classes,
            styles,
            disabled,
            border_title,
            border_subtitle,
            children,
        } = child;
        let mut node = Node::new(widget);
        node.css_id = id;
        node.classes = classes;
        node.disabled = disabled;
        node.border_title = border_title;
        node.border_subtitle = border_subtitle;
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
            .map(|w| w.compose())
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
                self.env.stylesheet.add_source(
                    "default:ModalScreen",
                    crate::screen::MODAL_SCREEN_CSS,
                    Origin::Default,
                );
            }
        }
        if !self.env.stylesheet.has_source("default:Screen") {
            self.env.stylesheet.add_source(
                "default:Screen",
                crate::screen::SCREEN_CSS,
                Origin::Default,
            );
        }
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
        self.remove_now(screen);
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
        if self
            .dom
            .focused
            .map(|f| self.dom.screen_of(f) == Some(screen))
            .unwrap_or(false)
        {
            return;
        }
        let chain = self.focus_chain();
        if let Some(first) = chain.first() {
            self.set_focus(Some(*first));
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
        self.timers.retain(|t| !ids.contains(&t.node));
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
                    if self.active_screen() == Some(screen) {
                        self.auto_focus(screen);
                    }
                }
                Pending::PopScreen(result) => {
                    self.pop_screen(result);
                }
            }
        }
        self.apply_removals();
        did
    }

    pub(crate) fn post(&mut self, target: NodeId, msg: Message) {
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
            let disabled = self.dom.is_disabled(id);
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
            Signal::Message { target, payload } => {
                if self.dom.contains(target) {
                    let css = self.dom.get(target).and_then(|n| n.css_id.clone());
                    let msg = Message::from_boxed(target, css, payload);
                    self.post(target, msg);
                }
            }
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
        match (t, n) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        }
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
        fired
    }

    // ------------------------------------------------------------------ focus

    /// Focusable nodes of the active screen in document order.
    pub(crate) fn focus_chain(&self) -> Vec<NodeId> {
        let Some(screen) = self.active_screen() else {
            return Vec::new();
        };
        let mut out = Vec::new();
        let mut stack = vec![screen];
        while let Some(id) = stack.pop() {
            let Some(node) = self.dom.get(id) else {
                continue;
            };
            if node.styles.get_display() == Display::None || node.disabled {
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
        }
        if let Some(new) = id {
            self.queue.push_back(Pending::Event {
                target: new,
                event: Event::Focus,
            });
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
        let start = self
            .dom
            .focused
            .or(self.active_screen())
            .unwrap_or(self.root);
        let mut chain = self.dom.ancestors_with_self(start);
        if !chain.contains(&self.root) {
            chain.push(self.root);
        }
        chain
    }

    fn node_bindings(&self, id: NodeId) -> Vec<Binding> {
        let mut b = self
            .dom
            .get(id)
            .and_then(|n| n.widget())
            .map(|w| w.bindings())
            .unwrap_or_default();
        if id == self.root {
            b.push(Binding::new("ctrl+q", "quit", "Quit").hidden().priority());
            b.push(Binding::new("ctrl+c", "quit", "Quit").hidden().priority());
        }
        if self.dom.get(id).map(|n| n.is_screen).unwrap_or(false) {
            b.push(Binding::new("tab", "app.focus_next", "Focus next").hidden());
            b.push(Binding::new("shift+tab", "app.focus_previous", "Focus previous").hidden());
        }
        b
    }

    pub(crate) fn handle_key(&mut self, key: KeyEvent) {
        let chain = self.binding_chain();
        // 1. priority bindings (app first, like Textual)
        for id in chain.iter().rev() {
            for b in self.node_bindings(*id) {
                if b.priority && b.matches(&key.key) && self.run_action(*id, &b.action) {
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
        // 3. regular bindings along the chain
        let chain = self.binding_chain();
        for id in chain {
            for b in self.node_bindings(id) {
                if !b.priority && b.matches(&key.key) && self.run_action(id, &b.action) {
                    return;
                }
            }
        }
    }

    pub(crate) fn handle_mouse(&mut self, kind: MouseKind, m: MouseEvent) {
        self.mouse_pos = Some(Offset::new(m.screen_x, m.screen_y));
        let target = self.node_at(m.screen_x, m.screen_y);
        match kind {
            MouseKind::Move => {
                self.update_hover(target);
                if let Some(t) = target {
                    self.dispatch_event(t, Event::MouseMove(m));
                }
            }
            MouseKind::Down => {
                self.update_hover(target);
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
                if let Some(d) = down {
                    // Click goes to the nearest common node of press and release.
                    let click_target = self
                        .dom
                        .ancestors_with_self(t)
                        .into_iter()
                        .find(|a| *a == d || self.dom.is_ancestor(*a, d));
                    if let Some(ct) = click_target {
                        self.dispatch_event(ct, Event::Click(m));
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
            let max = node.layout.max_scroll();
            let cur = node.scroll_offset;
            let next = Offset::new(cur.x + dx, cur.y + dy).clamp(max.x, max.y);
            if next != cur {
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
                let Some(node) = self.dom.get_mut(target) else {
                    return false;
                };
                let max = node.layout.max_scroll();
                let page = node.layout.content_region.height.max(1);
                let cur = node.scroll_offset;
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
                node.scroll_offset = next.clamp(max.x, max.y);
            }
            _ => return false,
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
        self.post(root, Message::new(root, None, Notified(n)));
        self.dirty = true;
    }

    // ------------------------------------------------------------------ frames

    /// Recompute styles for the whole tree (cascade + inline + inheritance).
    pub(crate) fn update_styles(&mut self) {
        let mut stack = vec![(self.root, None::<NodeId>)];
        while let Some((id, parent)) = stack.pop() {
            let mut styles = self.env.stylesheet.compute(self.dom.node_ref(id));
            let Some(node) = self.dom.get(id) else {
                continue;
            };
            styles.merge(&node.inline_styles);
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
    }

    /// Compute bindings shown in the footer: focus chain inner → outer, first binding per key wins.
    fn update_active_bindings(&mut self) {
        let mut seen: Vec<String> = Vec::new();
        let mut out = Vec::new();
        let chain = self.binding_chain();
        for id in chain {
            for b in self.node_bindings(id) {
                if !b.show {
                    continue;
                }
                let k = b.keys.first().cloned().unwrap_or_default();
                if seen.contains(&k) {
                    continue;
                }
                seen.push(k);
                out.push(b);
            }
        }
        // App bindings last, focused first is Textual's grouping; keep chain order reversed (app first)
        out.reverse();
        self.env.active_bindings = out;
    }

    pub(crate) fn layout(&mut self) {
        self.update_styles();
        self.update_active_bindings();
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
            let max = node.layout.max_scroll();
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
        let canvas = compositor::render(&self.dom, &self.env, &layers, self.size);
        self.dirty = false;
        canvas
    }

    /// Region of a node from the last layout.
    pub(crate) fn region_of(&self, id: NodeId) -> Region {
        self.dom
            .get(id)
            .map(|n| n.layout.region)
            .unwrap_or_default()
    }
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

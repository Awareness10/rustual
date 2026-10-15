//! Screen management beyond push / pop: `switch_screen`, named (installed) screens, modes,
//! maximize / minimize and the `loading` overlay.
//!
//! Ports of `App.switch_screen`, `App.SCREENS` / `install_screen`, `App.MODES` / `switch_mode`
//! (`textual/app.py`), `Screen.maximize` / `minimize` (`textual/screen.py`) and
//! `Widget.loading` / `set_loading` (`textual/widget.py`).
//!
//! ```ignore
//! App::new(MyApp)
//!     .mode("dashboard", || DashboardScreen::new().into())
//!     .mode("settings", || SettingsScreen::new().into())
//!     .default_mode("dashboard")
//!     .screen("help", || HelpScreen.into())
//! // in a handler:
//! ctx.switch_mode("settings");
//! ctx.push_screen_named("help");
//! ctx.maximize(ctx.focused().unwrap());
//! ctx.set_loading(table_id, true);
//! ```
//!
//! The runtime calls the hooks here from `app.rs` (startup, pop, layout, focus chain, keys,
//! builtin actions) and from the layout (`maximized_view_children`).

use crate::app::{App, AppState, Pending};
use crate::binding::Action;
use crate::context::Ctx;
use crate::dom::{Dom, NodeId};
use crate::events::{Event, KeyEvent};
use crate::widget::Child;
use std::collections::HashMap;
use std::rc::Rc;
use textual_css::{AlignH, AlignV, LayoutKind, Overflow};

/// Name of the mode holding the screen created from the root widget's `compose()`
/// (Textual's `App.DEFAULT_MODE` default).
pub const DEFAULT_MODE: &str = "_default";

/// Type names of screen children kept visible while a widget is maximized
/// (Textual's `App.ALLOW_IN_MAXIMIZED_VIEW = "Footer"`; toasts and tooltips are overlays here).
const ALLOW_IN_MAXIMIZED_VIEW: &[&str] = &["Footer", "ToastRack", "Tooltip"];

pub(crate) type ScreenFactory = Rc<dyn Fn() -> Child>;

/// Per-app state for this module.
pub(crate) struct ScreenState {
    /// `App::screen` factories (Textual: `SCREENS`), with the modal flag.
    pub named: HashMap<String, (ScreenFactory, bool)>,
    /// Installed screens: kept (not removed) when popped (Textual: `_installed_screens`).
    pub installed: HashMap<String, NodeId>,
    /// `App::mode` factories (Textual: `MODES`).
    pub modes: HashMap<String, ScreenFactory>,
    /// Screen stacks of the inactive modes (the active one is `AppState::screens`).
    pub stacks: HashMap<String, Vec<NodeId>>,
    pub current_mode: String,
    pub default_mode: Option<String>,
    /// Nodes that cover a loading widget.
    pub covers: Vec<NodeId>,
    /// `escape` minimizes a maximized widget (Textual: `App.ESCAPE_TO_MINIMIZE`).
    pub escape_to_minimize: bool,
}

impl Default for ScreenState {
    fn default() -> Self {
        ScreenState {
            named: HashMap::new(),
            installed: HashMap::new(),
            modes: HashMap::new(),
            stacks: HashMap::new(),
            current_mode: DEFAULT_MODE.to_string(),
            default_mode: None,
            covers: Vec::new(),
            escape_to_minimize: true,
        }
    }
}

// ---------------------------------------------------------------------------------- builders

impl App {
    /// Register a named screen (Textual: `SCREENS`). The factory runs the first time the
    /// screen is pushed; the screen is then installed and reused (popping it keeps it).
    pub fn screen<C: Into<Child>>(mut self, name: &str, factory: impl Fn() -> C + 'static) -> Self {
        let f: ScreenFactory = Rc::new(move || factory().into());
        self.state
            .screens_ext
            .named
            .insert(name.to_string(), (f, false));
        self
    }

    /// Like [`App::screen`] for a modal screen.
    pub fn modal_screen<C: Into<Child>>(
        mut self,
        name: &str,
        factory: impl Fn() -> C + 'static,
    ) -> Self {
        let f: ScreenFactory = Rc::new(move || factory().into());
        self.state
            .screens_ext
            .named
            .insert(name.to_string(), (f, true));
        self
    }

    /// Register a mode (Textual: `MODES`): a separate screen stack whose base screen is made
    /// by `factory` when the mode is first entered.
    pub fn mode<C: Into<Child>>(mut self, name: &str, factory: impl Fn() -> C + 'static) -> Self {
        let f: ScreenFactory = Rc::new(move || factory().into());
        self.state.screens_ext.modes.insert(name.to_string(), f);
        self
    }

    /// The mode to start in (Textual: `DEFAULT_MODE`). The root's `compose()` screen stays
    /// available as mode [`DEFAULT_MODE`] (`"_default"`).
    pub fn default_mode(mut self, name: &str) -> Self {
        self.state.screens_ext.default_mode = Some(name.to_string());
        self
    }

    /// Whether `escape` minimizes a maximized widget (default `true`, Textual: `ESCAPE_TO_MINIMIZE`).
    pub fn escape_to_minimize(mut self, on: bool) -> Self {
        self.state.screens_ext.escape_to_minimize = on;
        self
    }
}

// ---------------------------------------------------------------------------------- screens

/// Hook: after the default screen is mounted, enter the default mode.
pub(crate) fn startup(state: &mut AppState) {
    if let Some(mode) = state.screens_ext.default_mode.clone() {
        if state.screens_ext.modes.contains_key(&mode) {
            switch_mode(state, &mode);
            state.process_queue();
        }
    }
}

/// Whether `screen` is installed (kept when popped).
pub(crate) fn is_installed(state: &AppState, screen: NodeId) -> bool {
    state.screens_ext.installed.values().any(|s| *s == screen)
}

/// Hook: an installed screen leaves the stack — remember and drop its focus instead of removing it.
pub(crate) fn suspend_screen(state: &mut AppState, screen: NodeId) {
    if let Some(f) = state.dom.focused {
        if state.dom.screen_of(f) == Some(screen) {
            state.screen_focus.insert(screen, f);
            state.set_focus(None);
        }
    }
    crate::tooltip::clear(state);
    state.dirty = true;
}

/// Make `screen` (already in the DOM) the top of the stack, restoring its focus.
fn resume_screen(state: &mut AppState, screen: NodeId) {
    let f = state
        .screen_focus
        .remove(&screen)
        .filter(|f| state.dom.contains(*f) && state.dom.screen_of(*f) == Some(screen));
    match f {
        Some(f) => state.set_focus(Some(f)),
        None => {
            state.set_focus(None);
            state.queue.push_back(Pending::AutoFocus(screen));
        }
    }
    let size = state.size;
    state.queue.push_back(Pending::Event {
        target: screen,
        event: Event::Resize(size),
    });
    state.dirty = true;
}

/// Push an existing (installed) screen node.
fn push_existing(state: &mut AppState, pusher: NodeId, screen: NodeId) -> NodeId {
    if state.screens.contains(&screen) {
        return screen;
    }
    if let (Some(top), Some(f)) = (state.active_screen(), state.dom.focused) {
        state.screen_focus.insert(top, f);
    }
    crate::tooltip::clear(state);
    state.screens.push(screen);
    state.screen_pushers.insert(screen, pusher);
    resume_screen(state, screen);
    screen
}

/// The installed screen called `name`, installing it from its factory if needed.
fn named_screen(state: &mut AppState, name: &str) -> Option<NodeId> {
    if let Some(id) = state.screens_ext.installed.get(name).copied() {
        if state.dom.contains(id) {
            return Some(id);
        }
        state.screens_ext.installed.remove(name);
    }
    let (factory, modal) = state.screens_ext.named.get(name).cloned()?;
    Some(install(state, factory(), name, modal))
}

/// Mount a screen without showing it, and remember it by name (Textual: `install_screen`).
fn install(state: &mut AppState, child: Child, name: &str, modal: bool) -> NodeId {
    let id = state.mount_screen(child, modal);
    state.screens_ext.installed.insert(name.to_string(), id);
    id
}

pub(crate) fn push_named(state: &mut AppState, pusher: NodeId, name: &str) -> Option<NodeId> {
    let id = named_screen(state, name)?;
    Some(push_existing(state, pusher, id))
}

/// Replace the top screen (Textual: `switch_screen`). The replaced screen's pusher gets no
/// result; it is removed unless installed.
fn replace_top(state: &mut AppState, next: Option<NodeId>, child: Option<(Child, bool)>) -> NodeId {
    if let Some(top) = state.active_screen() {
        if Some(top) == next {
            return top;
        }
        state.screens.pop();
        state.screen_pushers.remove(&top);
        if is_installed(state, top) {
            suspend_screen(state, top);
        } else {
            state.request_remove(top);
            if let Some(f) = state.dom.focused {
                if state.dom.screen_of(f) == Some(top) {
                    state.set_focus(None);
                }
            }
        }
    }
    crate::tooltip::clear(state);
    let id = match (next, child) {
        (Some(id), _) => id,
        (None, Some((child, modal))) => state.mount_screen(child, modal),
        (None, None) => unreachable!("replace_top needs a screen"),
    };
    state.screens.push(id);
    resume_screen(state, id);
    id
}

pub(crate) fn switch_screen(state: &mut AppState, child: Child) -> NodeId {
    replace_top(state, None, Some((child, false)))
}

pub(crate) fn switch_named(state: &mut AppState, name: &str) -> Option<NodeId> {
    let id = named_screen(state, name)?;
    if state.screens.contains(&id) && state.active_screen() != Some(id) {
        // Already lower in the stack (Textual raises); leave the stack alone.
        return None;
    }
    Some(replace_top(state, Some(id), None))
}

/// Switch to another mode's screen stack (Textual: `switch_mode`). Returns false for an
/// unknown mode.
pub(crate) fn switch_mode(state: &mut AppState, mode: &str) -> bool {
    if state.screens_ext.current_mode == mode {
        return true;
    }
    let known = state.screens_ext.modes.contains_key(mode)
        || state.screens_ext.stacks.contains_key(mode)
        || mode == DEFAULT_MODE;
    if !known {
        return false;
    }
    if let Some(top) = state.active_screen() {
        suspend_screen(state, top);
    }
    let old = std::mem::take(&mut state.screens);
    let current = std::mem::replace(&mut state.screens_ext.current_mode, mode.to_string());
    state.screens_ext.stacks.insert(current, old);
    let mut stack = state
        .screens_ext
        .stacks
        .remove(mode)
        .unwrap_or_default()
        .into_iter()
        .filter(|s| state.dom.contains(*s))
        .collect::<Vec<_>>();
    if stack.is_empty() {
        let child = match state.screens_ext.modes.get(mode).cloned() {
            Some(f) => f(),
            None => Child::new(Box::new(crate::screen::Screen::new())),
        };
        stack.push(state.mount_screen(child, false));
    }
    state.screens = stack;
    state.dom.hovered = None;
    let size = state.size;
    for s in state.screens.clone() {
        state.queue.push_back(Pending::Event {
            target: s,
            event: Event::Resize(size),
        });
    }
    if let Some(top) = state.active_screen() {
        resume_screen(state, top);
    }
    true
}

// ---------------------------------------------------------------------------------- maximize

pub(crate) fn allow_maximize(dom: &Dom, id: NodeId) -> bool {
    let Some(node) = dom.get(id) else {
        return false;
    };
    if node.is_screen {
        return false;
    }
    node.widget()
        .and_then(|w| w.allow_maximize())
        .unwrap_or(node.can_focus)
}

/// Maximize `id` or, with `container`, its nearest ancestor that allows maximizing
/// (Textual: `Screen.maximize(widget, container=True)`).
pub(crate) fn maximize(state: &mut AppState, id: NodeId, container: bool) -> bool {
    if !allow_maximize(&state.dom, id) {
        return false;
    }
    let Some(screen) = state.dom.screen_of(id) else {
        return false;
    };
    let mut target = id;
    if container {
        if let Some(a) = state
            .dom
            .ancestors(id)
            .into_iter()
            .take_while(|a| *a != screen)
            .find(|a| allow_maximize(&state.dom, *a))
        {
            target = a;
        }
    }
    let previous = state.dom.get(screen).and_then(|n| n.maximized);
    if let Some(p) = previous.and_then(|p| state.dom.get_mut(p)) {
        p.remove_class("-maximized");
    }
    if let Some(n) = state.dom.get_mut(target) {
        n.add_class("-maximized");
    }
    if let Some(s) = state.dom.get_mut(screen) {
        s.maximized = Some(target);
        s.add_class("-maximized-view");
    }
    // Keep focus inside the maximized view.
    let focus_inside = state
        .dom
        .focused
        .is_some_and(|f| f == target || state.dom.is_ancestor(target, f));
    if !focus_inside {
        state.set_focus(None);
        state.queue.push_back(Pending::AutoFocus(screen));
    }
    crate::tooltip::clear(state);
    state.dirty = true;
    true
}

/// Restore the maximized widget of the active screen (Textual: `Screen.minimize`).
pub(crate) fn minimize(state: &mut AppState) -> bool {
    let Some(screen) = state.active_screen() else {
        return false;
    };
    let Some(m) = state.dom.get_mut(screen).and_then(|s| s.maximized.take()) else {
        return false;
    };
    if let Some(s) = state.dom.get_mut(screen) {
        s.remove_class("-maximized-view");
    }
    if let Some(n) = state.dom.get_mut(m) {
        n.remove_class("-maximized");
    }
    state.scroll_visible_request = state.dom.focused;
    state.dirty = true;
    true
}

/// The maximized widget of `screen`, if it still exists there.
pub(crate) fn maximized_of(dom: &Dom, screen: NodeId) -> Option<NodeId> {
    let m = dom.get(screen)?.maximized?;
    (dom.contains(m) && dom.screen_of(m) == Some(screen) && m != screen).then_some(m)
}

/// Layout hook: the children a maximized screen arranges (the maximized widget first, then
/// allowed direct children), or `None` if `id` is not a screen with a maximized widget.
pub(crate) fn maximized_view_children(dom: &Dom, id: NodeId) -> Option<Vec<NodeId>> {
    if !dom.get(id)?.is_screen {
        return None;
    }
    let m = maximized_of(dom, id)?;
    let mut out = vec![m];
    for c in dom.children(id) {
        let allowed = dom
            .get(*c)
            .is_some_and(|n| ALLOW_IN_MAXIMIZED_VIEW.iter().any(|t| n.is_type(t)));
        if allowed && *c != m {
            out.push(*c);
        }
    }
    Some(out)
}

/// Focus-chain hook: the node the active screen's focus chain starts from.
pub(crate) fn focus_root(state: &AppState, screen: NodeId) -> NodeId {
    maximized_of(&state.dom, screen).unwrap_or(screen)
}

/// Key hook (before bindings): `escape` minimizes (Textual: `App._on_key` special case).
pub(crate) fn handle_key(state: &mut AppState, key: &KeyEvent) -> bool {
    if key.key != "escape" || !state.screens_ext.escape_to_minimize {
        return false;
    }
    let Some(screen) = state.active_screen() else {
        return false;
    };
    if maximized_of(&state.dom, screen).is_none() {
        return false;
    }
    minimize(state)
}

/// Style hook (after the cascade): give covers their styles and apply the maximized-view
/// overrides (Textual's `Screen.-maximized-view { layout: vertical !important; overflow-y:
/// auto !important; align: center middle; .-maximized { dock: initial !important } }`).
pub(crate) fn adjust_styles(state: &mut AppState) {
    state.screens_ext.covers.retain(|c| state.dom.contains(*c));
    for cover in state.screens_ext.covers.clone() {
        for id in state.dom.descendants(cover) {
            let mut styles = state.env.stylesheet.compute(state.dom.node_ref(id));
            let Some(node) = state.dom.get(id) else {
                continue;
            };
            styles.merge(&node.inline_styles);
            if let Some(p) = node.parent.and_then(|p| state.dom.get(p)) {
                styles.inherit_from(&p.styles);
            }
            if let Some(node) = state.dom.get_mut(id) {
                node.styles = styles;
            }
        }
    }
    for screen in state.screens.clone() {
        let Some(m) = maximized_of(&state.dom, screen) else {
            continue;
        };
        if let Some(s) = state.dom.get_mut(screen) {
            s.styles.layout = Some(LayoutKind::Vertical);
            s.styles.overflow_y = Some(Overflow::Auto);
            s.styles.align_horizontal = Some(AlignH::Center);
            s.styles.align_vertical = Some(AlignV::Middle);
        }
        if let Some(n) = state.dom.get_mut(m) {
            n.styles.dock = None;
        }
    }
}

// ---------------------------------------------------------------------------------- loading

/// Put `id` into (or out of) the loading state (Textual: `Widget.set_loading`).
pub(crate) fn set_loading(state: &mut AppState, id: NodeId, loading: bool) {
    let Some(node) = state.dom.get(id) else {
        return;
    };
    if node.is_screen || node.loading == loading {
        return;
    }
    if loading {
        let indicator = Child::new(Box::new(crate::widgets::LoadingIndicator::new()))
            .classes("-textual-loading-indicator");
        let cover = state.build_node(indicator);
        if let Some(c) = state.dom.get_mut(cover) {
            c.is_cover = true;
            c.parent = Some(id);
        }
        if let Some(n) = state.dom.get_mut(id) {
            n.loading = true;
            n.cover = Some(cover);
        }
        state.screens_ext.covers.push(cover);
        let focused_inside = state
            .dom
            .focused
            .is_some_and(|f| f == id || state.dom.is_ancestor(id, f));
        if focused_inside {
            state.set_focus(None);
        }
    } else {
        let cover = state.dom.get_mut(id).and_then(|n| {
            n.loading = false;
            n.cover
        });
        if let Some(c) = cover {
            state.request_remove(c);
        }
    }
    state.dirty = true;
}

// ---------------------------------------------------------------------------------- actions

/// Builtin actions provided by this module. Returns false for unknown actions.
pub(crate) fn builtin_action(state: &mut AppState, target: NodeId, action: &Action) -> bool {
    let arg = action.args.first().map(String::as_str).unwrap_or("");
    match action.name.as_str() {
        "push_screen" => push_named(state, target, arg).is_some(),
        "switch_screen" => switch_named(state, arg).is_some(),
        "switch_mode" => switch_mode(state, arg),
        "maximize" => match state.dom.focused {
            Some(f) => maximize(state, f, true),
            None => false,
        },
        "minimize" => {
            minimize(state);
            true
        }
        _ => false,
    }
}

// ---------------------------------------------------------------------------------- Ctx API

impl Ctx<'_> {
    /// Replace the top screen with `screen` (Textual: `switch_screen`). The replaced screen is
    /// removed (unless it is a named / installed screen) and its pusher gets no result.
    pub fn switch_screen(&mut self, screen: impl Into<Child>) -> NodeId {
        switch_screen(self.app, screen.into())
    }

    /// Push the screen registered with [`App::screen`] (or installed) as `name`; this widget
    /// receives the dismissal result. Returns `None` for an unknown name.
    pub fn push_screen_named(&mut self, name: &str) -> Option<NodeId> {
        let pusher = self.node;
        push_named(self.app, pusher, name)
    }

    /// Replace the top screen with the named screen. Returns `None` for an unknown name (or if
    /// the screen is already lower in the stack).
    pub fn switch_screen_named(&mut self, name: &str) -> Option<NodeId> {
        switch_named(self.app, name)
    }

    /// Install a screen under `name` without showing it (Textual: `install_screen`); it can then
    /// be pushed / switched to by name and is kept when popped. Replaces an earlier one.
    pub fn install_screen(&mut self, screen: impl Into<Child>, name: &str) -> NodeId {
        if let Some(old) = self.app.screens_ext.installed.remove(name) {
            if !self.app.screens.contains(&old) {
                self.app.request_remove(old);
            }
        }
        install(self.app, screen.into(), name, false)
    }

    /// Forget an installed screen (removed now unless it is on a screen stack).
    pub fn uninstall_screen(&mut self, name: &str) -> bool {
        let Some(id) = self.app.screens_ext.installed.remove(name) else {
            return false;
        };
        let on_stack = self.app.screens.contains(&id)
            || self
                .app
                .screens_ext
                .stacks
                .values()
                .any(|s| s.contains(&id));
        if !on_stack {
            self.app.request_remove(id);
        }
        true
    }

    /// Whether a screen is installed under `name`.
    pub fn is_screen_installed(&self, name: &str) -> bool {
        self.app.screens_ext.installed.contains_key(name)
    }

    /// Switch to mode `name` (Textual: `switch_mode`); each mode keeps its own screen stack.
    /// Returns false for an unknown mode.
    pub fn switch_mode(&mut self, name: &str) -> bool {
        switch_mode(self.app, name)
    }

    /// The current mode ([`DEFAULT_MODE`] unless modes are used).
    pub fn current_mode(&self) -> &str {
        &self.app.screens_ext.current_mode
    }

    /// Maximize `id` (or its nearest ancestor that allows maximizing, like Textual's
    /// `container=True`) so it fills the screen. Returns false if not allowed.
    pub fn maximize(&mut self, id: NodeId) -> bool {
        maximize(self.app, id, true)
    }

    /// Maximize exactly `id` (Textual: `maximize(widget, container=False)`).
    pub fn maximize_exact(&mut self, id: NodeId) -> bool {
        maximize(self.app, id, false)
    }

    /// Restore the maximized widget of the active screen.
    pub fn minimize(&mut self) {
        minimize(self.app);
    }

    /// The maximized widget of the active screen.
    pub fn maximized(&self) -> Option<NodeId> {
        let screen = self.app.active_screen()?;
        maximized_of(&self.app.dom, screen)
    }

    /// Show a [`LoadingIndicator`](crate::widgets::LoadingIndicator) over `id` and block its
    /// input (`true`), or restore it (`false`) (Textual: `widget.loading = ...`).
    pub fn set_loading(&mut self, id: NodeId, loading: bool) {
        set_loading(self.app, id, loading);
    }

    /// Whether `id` is in the loading state.
    pub fn is_loading(&self, id: NodeId) -> bool {
        self.app.dom.get(id).is_some_and(|n| n.loading)
    }
}

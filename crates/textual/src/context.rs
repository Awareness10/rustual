//! [`Ctx`]: the handle widgets use to talk to the app from handlers.

use crate::app::{AppHandle, AppState, Pending, Severity, TimerId, WorkerHandle};
use crate::dom::{Dom, NodeId};
use crate::message::Message;
use crate::widget::{Child, Widget};
use std::any::Any;
use std::future::Future;
use std::time::Duration;
use textual_css::{Display, Styles};
use textual_render::{Offset, Region, Size};

/// Passed to every handler. The widget whose handler is running is "checked out" of the DOM,
/// so methods that look up widgets (`query_one`, `get`, `with`) can reach every *other* widget.
pub struct Ctx<'a> {
    pub(crate) app: &'a mut AppState,
    pub(crate) node: NodeId,
}

impl<'a> Ctx<'a> {
    // ------------------------------------------------------------------ identity & tree

    /// The node of the widget whose handler is running.
    pub fn id(&self) -> NodeId {
        self.node
    }

    /// The app root node (the widget passed to `App::new`).
    pub fn root(&self) -> NodeId {
        self.app.root
    }

    /// The active (top) screen.
    pub fn screen(&self) -> NodeId {
        self.app.active_screen().unwrap_or(self.app.root)
    }

    pub fn parent(&self) -> Option<NodeId> {
        self.app.dom.parent(self.node)
    }

    pub fn children(&self) -> Vec<NodeId> {
        self.app.dom.children(self.node).to_vec()
    }

    pub fn dom(&self) -> &Dom {
        &self.app.dom
    }

    /// Direct DOM access (marks the app dirty).
    pub fn dom_mut(&mut self) -> &mut Dom {
        self.app.dirty = true;
        &mut self.app.dom
    }

    /// CSS id of a node.
    pub fn css_id(&self, id: NodeId) -> Option<&str> {
        self.app.dom.get(id).and_then(|n| n.css_id.as_deref())
    }

    // ------------------------------------------------------------------ queries

    /// All nodes in the DOM (every screen) matching `selector`, in document order.
    pub fn query(&self, selector: &str) -> Vec<NodeId> {
        self.app.dom.query_str(self.app.root, selector)
    }

    /// Nodes under `root` (inclusive) matching `selector`.
    pub fn query_in(&self, root: NodeId, selector: &str) -> Vec<NodeId> {
        self.app.dom.query_str(root, selector)
    }

    /// First node matching `selector`, searching the active screen first, then everything.
    pub fn query_id(&self, selector: &str) -> Option<NodeId> {
        let screen = self.screen();
        self.app
            .dom
            .query_str(screen, selector)
            .into_iter()
            .next()
            .or_else(|| self.query(selector).into_iter().next())
    }

    /// First widget of type `W` matching `selector` (active screen first). Marks the app dirty.
    pub fn query_one<W: Widget>(&mut self, selector: &str) -> Option<&mut W> {
        let screen = self.screen();
        let mut ids = self.app.dom.query_str(screen, selector);
        ids.extend(self.app.dom.query_str(self.app.root, selector));
        let id = ids
            .into_iter()
            .find(|id| self.app.dom.widget::<W>(*id).is_some())?;
        self.app.dirty = true;
        self.app.dom.widget_mut::<W>(id)
    }

    /// Widget of type `W` at `id` (not the running widget itself).
    pub fn get<W: Widget>(&mut self, id: NodeId) -> Option<&mut W> {
        self.app.dirty = true;
        self.app.dom.widget_mut::<W>(id)
    }

    /// Run `f` on another widget with a `Ctx` of its own (so it can post messages, etc.).
    pub fn with_id<W: Widget, R>(
        &mut self,
        id: NodeId,
        f: impl FnOnce(&mut W, &mut Ctx) -> R,
    ) -> Option<R> {
        self.app.dom.widget::<W>(id)?;
        self.app
            .call_widget(id, |w, ctx| {
                crate::widget::downcast_mut::<W>(w).map(|w| f(w, ctx))
            })
            .flatten()
    }

    /// Like [`Ctx::with_id`] for the first widget of type `W` matching `selector`.
    pub fn with<W: Widget, R>(
        &mut self,
        selector: &str,
        f: impl FnOnce(&mut W, &mut Ctx) -> R,
    ) -> Option<R> {
        let screen = self.screen();
        let mut ids = self.app.dom.query_str(screen, selector);
        ids.extend(self.app.dom.query_str(self.app.root, selector));
        let id = ids
            .into_iter()
            .find(|id| self.app.dom.widget::<W>(*id).is_some())?;
        self.with_id(id, f)
    }

    /// Run `f` on another widget (untyped) with a `Ctx` of its own. Returns `None` if the node
    /// doesn't exist or its widget is checked out (a handler of it is running).
    pub fn with_widget<R>(
        &mut self,
        id: NodeId,
        f: impl FnOnce(&mut dyn Widget, &mut Ctx) -> R,
    ) -> Option<R> {
        self.app.call_widget(id, f)
    }

    /// Run `f` with a `Ctx` targeting another node, without checking its widget out
    /// (for node-level operations such as timers, scrolling or classes on that node).
    pub fn for_node<R>(&mut self, id: NodeId, f: impl FnOnce(&mut Ctx) -> R) -> R {
        let mut ctx = Ctx {
            app: &mut *self.app,
            node: id,
        };
        f(&mut ctx)
    }

    // ------------------------------------------------------------------ messages & actions

    /// Post a message from this widget. It is handled by this widget first, then bubbles up.
    pub fn post_message<T: Any + Send>(&mut self, payload: T) {
        let target = self.node;
        self.post_message_to(target, payload);
    }

    /// Post a message (from this widget) to `target`; it bubbles up from there.
    pub fn post_message_to<T: Any + Send>(&mut self, target: NodeId, payload: T) {
        let css = self.app.dom.get(self.node).and_then(|n| n.css_id.clone());
        let msg = Message::new(self.node, css, payload);
        self.app.post(target, msg);
    }

    /// Post a message that does not bubble.
    pub fn post_message_no_bubble<T: Any + Send>(&mut self, target: NodeId, payload: T) {
        let css = self.app.dom.get(self.node).and_then(|n| n.css_id.clone());
        let mut msg = Message::new(self.node, css, payload);
        msg.bubble = false;
        self.app.post(target, msg);
    }

    /// Queue an action string (e.g. `"app.quit"`) as if triggered by this widget's binding.
    pub fn run_action(&mut self, action: &str) {
        let target = self.node;
        self.app.queue.push_back(Pending::Action {
            target,
            action: action.to_string(),
        });
    }

    /// A `Send` handle for posting messages from workers / threads.
    pub fn app_handle(&self) -> AppHandle {
        self.app.handle()
    }

    // ------------------------------------------------------------------ classes, styles, attributes

    fn node_mut(&mut self, id: NodeId) -> Option<&mut crate::dom::Node> {
        self.app.dirty = true;
        self.app.dom.get_mut(id)
    }

    pub fn has_class(&self, class: &str) -> bool {
        self.app
            .dom
            .get(self.node)
            .map(|n| n.has_class(class))
            .unwrap_or(false)
    }

    pub fn add_class(&mut self, class: &str) {
        let id = self.node;
        self.add_class_to(id, class);
    }

    pub fn remove_class(&mut self, class: &str) {
        let id = self.node;
        self.remove_class_from(id, class);
    }

    pub fn toggle_class(&mut self, class: &str) {
        if let Some(n) = self.node_mut(self.node) {
            n.toggle_class(class);
        }
    }

    pub fn set_class(&mut self, on: bool, class: &str) {
        if let Some(n) = self.node_mut(self.node) {
            n.set_class(on, class);
        }
    }

    pub fn add_class_to(&mut self, id: NodeId, class: &str) {
        if let Some(n) = self.node_mut(id) {
            n.add_class(class);
        }
    }

    pub fn remove_class_from(&mut self, id: NodeId, class: &str) {
        if let Some(n) = self.node_mut(id) {
            n.remove_class(class);
        }
    }

    pub fn set_class_on(&mut self, id: NodeId, on: bool, class: &str) {
        if let Some(n) = self.node_mut(id) {
            n.set_class(on, class);
        }
    }

    /// Modify this widget's inline styles.
    pub fn set_styles(&mut self, f: impl FnOnce(&mut Styles)) {
        let id = self.node;
        self.set_styles_of(id, f);
    }

    pub fn set_styles_of(&mut self, id: NodeId, f: impl FnOnce(&mut Styles)) {
        if let Some(n) = self.node_mut(id) {
            f(&mut n.inline_styles);
        }
    }

    /// Merge inline CSS declarations (`"width: 10; color: red"`) into a node's inline styles.
    pub fn set_style(&mut self, id: NodeId, css: &str) -> Result<(), String> {
        let parsed = Styles::parse_inline(css)?;
        self.set_styles_of(id, |s| s.merge(&parsed));
        Ok(())
    }

    /// Show / hide a node (`display: block | none`).
    pub fn set_display(&mut self, id: NodeId, visible: bool) {
        self.set_styles_of(id, |s| {
            s.display = Some(if visible {
                Display::Block
            } else {
                Display::None
            })
        });
    }

    /// Computed styles of this widget (from the last frame).
    pub fn styles(&self) -> Styles {
        self.app
            .dom
            .get(self.node)
            .map(|n| n.styles.clone())
            .unwrap_or_default()
    }

    pub fn set_disabled(&mut self, id: NodeId, disabled: bool) {
        if let Some(n) = self.node_mut(id) {
            n.disabled = disabled;
        }
        if disabled
            && self
                .app
                .dom
                .focused
                .map(|f| f == id || self.app.dom.is_ancestor(id, f))
                .unwrap_or(false)
        {
            self.app.set_focus(None);
        }
    }

    pub fn is_disabled(&self) -> bool {
        self.app.dom.is_disabled(self.node)
    }

    pub fn set_border_title(&mut self, title: Option<&str>) {
        if let Some(n) = self.node_mut(self.node) {
            n.border_title = title.map(String::from);
        }
    }

    pub fn set_border_subtitle(&mut self, subtitle: Option<&str>) {
        if let Some(n) = self.node_mut(self.node) {
            n.border_subtitle = subtitle.map(String::from);
        }
    }

    /// Request a repaint (handlers already do this implicitly).
    pub fn refresh(&mut self) {
        self.app.dirty = true;
    }

    // ------------------------------------------------------------------ mounting

    /// Mount a child at the end of this widget's children.
    pub fn mount(&mut self, child: impl Into<Child>) -> NodeId {
        let parent = self.node;
        self.app.mount(parent, child.into(), None)
    }

    /// Mount under `parent`, optionally at `index`.
    pub fn mount_to(
        &mut self,
        parent: NodeId,
        child: impl Into<Child>,
        index: Option<usize>,
    ) -> NodeId {
        self.app.mount(parent, child.into(), index)
    }

    /// Remove a node and its subtree (deferred until the current handler returns).
    pub fn remove(&mut self, id: NodeId) {
        self.app.request_remove(id);
    }

    pub fn remove_self(&mut self) {
        let id = self.node;
        self.app.request_remove(id);
    }

    pub fn remove_children(&mut self, id: NodeId) {
        for c in self.app.dom.children(id).to_vec() {
            self.app.request_remove(c);
        }
    }

    // ------------------------------------------------------------------ focus

    pub fn focus(&mut self, id: NodeId) {
        self.app.set_focus(Some(id));
    }

    pub fn focus_self(&mut self) {
        let id = self.node;
        self.app.set_focus(Some(id));
    }

    pub fn blur(&mut self) {
        if self.app.dom.focused == Some(self.node) {
            self.app.set_focus(None);
        }
    }

    pub fn focus_next(&mut self) {
        self.app.focus_next(true);
    }

    pub fn focus_previous(&mut self) {
        self.app.focus_next(false);
    }

    pub fn focused(&self) -> Option<NodeId> {
        self.app.dom.focused
    }

    pub fn has_focus(&self) -> bool {
        self.app.dom.focused == Some(self.node)
    }

    // ------------------------------------------------------------------ timers & workers

    /// Call `f` once after `delay` (with this widget, downcast to `W`).
    pub fn set_timer<W: Widget>(
        &mut self,
        delay: Duration,
        mut f: impl FnMut(&mut W, &mut Ctx) + 'static,
    ) -> TimerId {
        let node = self.node;
        self.app.add_timer(
            node,
            delay,
            false,
            Box::new(move |w, ctx| {
                if let Some(w) = crate::widget::downcast_mut::<W>(w) {
                    f(w, ctx)
                }
            }),
        )
    }

    /// Call `f` every `interval` until cancelled or the widget is removed.
    pub fn set_interval<W: Widget>(
        &mut self,
        interval: Duration,
        mut f: impl FnMut(&mut W, &mut Ctx) + 'static,
    ) -> TimerId {
        let node = self.node;
        self.app.add_timer(
            node,
            interval,
            true,
            Box::new(move |w, ctx| {
                if let Some(w) = crate::widget::downcast_mut::<W>(w) {
                    f(w, ctx)
                }
            }),
        )
    }

    pub fn cancel_timer(&mut self, id: TimerId) {
        self.app.cancel_timer(id);
    }

    /// Run a future on the tokio runtime; its output is posted to this widget as a message.
    pub fn run_worker<F, T>(&mut self, fut: F) -> WorkerHandle
    where
        F: Future<Output = T> + Send + 'static,
        T: Any + Send,
    {
        crate::workers::run_plain(self, fut)
    }

    /// Run a blocking closure on tokio's blocking pool; its output is posted to this widget.
    pub fn run_blocking<F, T>(&mut self, f: F) -> WorkerHandle
    where
        F: FnOnce() -> T + Send + 'static,
        T: Any + Send,
    {
        crate::workers::run_plain_blocking(self, f)
    }

    // ------------------------------------------------------------------ app-level

    pub fn exit(&mut self) {
        self.app.exit_requested = true;
    }

    pub fn bell(&mut self) {
        self.app.bell_count += 1;
    }

    pub fn notify(&mut self, message: &str) {
        self.app.notify(
            message.to_string(),
            String::new(),
            Severity::Information,
            None,
        );
    }

    pub fn notify_with(
        &mut self,
        message: &str,
        title: &str,
        severity: Severity,
        timeout: Option<Duration>,
    ) {
        self.app
            .notify(message.to_string(), title.to_string(), severity, timeout);
    }

    pub fn title(&self) -> &str {
        &self.app.env.title
    }

    pub fn set_title(&mut self, title: &str) {
        self.app.env.title = title.to_string();
        self.app.dirty = true;
    }

    pub fn set_sub_title(&mut self, sub_title: &str) {
        self.app.env.sub_title = sub_title.to_string();
        self.app.dirty = true;
    }

    /// Add (or replace) app-level CSS at runtime under `key` (e.g. a screen's `CSS`).
    /// Returns the parse errors.
    pub fn add_css(&mut self, key: &str, css: &str) -> Vec<String> {
        let errors: Vec<String> = self
            .app
            .env
            .stylesheet
            .add_source(key, css, textual_css::Origin::App)
            .into_iter()
            .map(|e| format!("{key}:{e}"))
            .collect();
        self.app.css_errors.extend(errors.iter().cloned());
        self.app.dirty = true;
        errors
    }

    /// Whether CSS was added under `key` (see [`Ctx::add_css`]).
    pub fn has_css(&self, key: &str) -> bool {
        self.app.env.stylesheet.has_source(key)
    }

    /// Register (or replace) a theme at runtime (Textual's `App.register_theme`).
    pub fn register_theme(&mut self, theme: textual_css::Theme) {
        self.app.themes.retain(|t| t.name != theme.name);
        self.app.themes.push(theme);
    }

    /// Switch theme by name. Returns false if unknown.
    pub fn set_theme(&mut self, name: &str) -> bool {
        self.app.apply_theme(name)
    }

    pub fn theme(&self) -> &str {
        &self.app.theme
    }

    pub fn theme_names(&self) -> Vec<String> {
        self.app.themes.iter().map(|t| t.name.clone()).collect()
    }

    pub fn is_dark(&self) -> bool {
        self.app.dom.dark
    }

    /// Push a screen; this widget receives the dismissal result as a message.
    pub fn push_screen(&mut self, screen: impl Into<Child>) -> NodeId {
        let pusher = self.node;
        self.app.push_screen(pusher, screen.into(), false)
    }

    /// Push a modal screen (the screen below stays visible behind its translucent background).
    pub fn push_modal(&mut self, screen: impl Into<Child>) -> NodeId {
        let pusher = self.node;
        self.app.push_screen(pusher, screen.into(), true)
    }

    /// Pop the top screen; the pusher receives [`crate::app::ScreenDismissed`].
    pub fn pop_screen(&mut self) {
        self.app.queue.push_back(Pending::PopScreen(None));
    }

    /// Pop the top screen, posting `result` to the widget that pushed it.
    pub fn dismiss<T: Any + Send>(&mut self, result: T) {
        self.app
            .queue
            .push_back(Pending::PopScreen(Some(Box::new(result))));
    }

    pub fn screen_count(&self) -> usize {
        self.app.screens.len()
    }

    // ------------------------------------------------------------------ geometry & scrolling

    /// Border-box region of this widget from the last layout (screen coordinates).
    pub fn region(&self) -> Region {
        self.app.region_of(self.node)
    }

    pub fn region_of(&self, id: NodeId) -> Region {
        self.app.region_of(id)
    }

    /// Content region from the last layout (screen coordinates).
    pub fn content_region(&self) -> Region {
        self.app
            .dom
            .get(self.node)
            .map(|n| n.layout.content_region)
            .unwrap_or_default()
    }

    /// Size of the content region.
    pub fn size(&self) -> Size {
        self.content_region().size()
    }

    /// Terminal size.
    pub fn app_size(&self) -> Size {
        self.app.size
    }

    pub fn scroll_offset(&self) -> Offset {
        self.app
            .dom
            .get(self.node)
            .map(|n| n.scroll_offset)
            .unwrap_or_default()
    }

    /// Maximum scroll offset from the last layout.
    pub fn max_scroll(&self) -> Offset {
        self.app
            .dom
            .get(self.node)
            .map(|n| n.layout.max_scroll())
            .unwrap_or_default()
    }

    /// Set this widget's scroll offset (clamped at the next layout).
    pub fn scroll_to(&mut self, x: i32, y: i32) {
        let id = self.node;
        self.scroll_node_to(id, x, y);
    }

    pub fn scroll_node_to(&mut self, id: NodeId, x: i32, y: i32) {
        // A direct scroll stops a smooth scroll in progress (Textual's `_scroll_to`).
        self.app.animator.cancel_scroll(id);
        if let Some(n) = self.node_mut(id) {
            n.scroll_offset = Offset::new(x.max(0), y.max(0));
        }
    }

    pub fn scroll_by(&mut self, dx: i32, dy: i32) {
        let cur = self.scroll_offset();
        let max = self.max_scroll();
        let next = Offset::new(cur.x + dx, cur.y + dy).clamp(max.x, max.y);
        self.scroll_to(next.x, next.y);
    }

    /// For self-scrolling widgets: scroll so `region` (in virtual/content coordinates) is visible.
    pub fn scroll_to_show(&mut self, region: Region) {
        let size = self.size();
        let cur = self.scroll_offset();
        let viewport = Region::from_offset_size(cur, size);
        let delta = viewport.get_scroll_to_visible(&region);
        let next = cur + delta;
        self.scroll_to(next.x, next.y);
    }

    /// Scroll ancestors so this widget is visible (after the next layout).
    pub fn scroll_visible(&mut self) {
        self.app.scroll_visible_request = Some(self.node);
        self.app.dirty = true;
    }
}

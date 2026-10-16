//! `Tabs`, `Tab` and the tab `Underline` (port of `textual/widgets/_tabs.py`).
//!
//! ```
//! # use rustual::prelude::*;
//! let auto = Tabs::new(["Home", "Settings"]); // auto ids `tab-1`, `tab-2`
//! let explicit = Tabs::new([Tab::new("Home").tab_id("home"), Tab::new("About").tab_id("about")]).active("about");
//! let composed = Tabs::default().with(compose![Tab::new("A").id("a"), Tab::new("B").id("b")]); // like `Tabs(Tab(..), ..)`
//! ```
//!
//! Structure (as in Textual): `Tabs > Container#tabs-scroll > Vertical#tabs-list-bar >
//! (Horizontal#tabs-list > Tab*, Underline)`. Tabs given with `.with(...)` are moved into
//! `#tabs-list` when the `Tabs` is mounted.
//!
//! The underline slides to the newly active tab over 0.3s (`in_out_cubic`, level `basic`), as in
//! Textual. Deviations: the underline's resting position is computed from the active tab's
//! layout at render time (Textual updates it after refresh); the slide interpolates from where
//! the underline was towards the active tab's current position.

use crate::animation::{AnimationLevel, Easing};
use crate::binding::Binding;
use crate::compose;
use crate::context::Ctx;
use crate::dom::NodeId;
use crate::events::Event;
use crate::message::{Flow, Message};
use crate::widget::{Child, Content, RenderCtx, Widget, WidgetExt};
use crate::widgets::progress_bar::BarRenderable;
use crate::widgets::{Container, Horizontal, Vertical};
use rustual_render::{cells::cell_len, Span, Strip, Style, Text};
use std::time::Duration;

const UNDERLINE_CSS: &str = r#"
Underline {
    width: 1fr;
    height: 1;
    & > .underline--bar {
        color: $block-cursor-background;
        background: $foreground 10%;
    }
}

/* ANSI themes (Textual `:ansi`). */
Underline {
    &:ansi {
        & > .underline--bar {
            color: $block-cursor-background;
            background: $border-blurred;
        }
    }
}
"#;

const TAB_CSS: &str = r#"
Tab {
    width: auto;
    height: 1;
    padding: 0 1;
    text-align: center;
    color: $foreground 50%;
    pointer: pointer;

    &:hover {
        color: $foreground;
    }
    &:disabled {
        color: $foreground 25%;
    }

    &.-active {
        color: $foreground;
    }
    &.-hidden {
        display: none;
    }
}

/* ANSI themes (Textual `:ansi`). */
Tab {
    &:ansi {
        text-style: dim;
        &.-active {
            text-style: not dim bold;
        }
    }
}
"#;

const TABS_CSS: &str = r#"
Tabs {
    width: 100%;
    height: 2;
    &:focus {
        .underline--bar {
            background: $foreground 30%;
        }
        & .-active {
            text-style: $block-cursor-text-style;
            color: $block-cursor-foreground;
            background: $block-cursor-background;
        }
    }

    & > #tabs-scroll {
        overflow: hidden;
    }

    #tabs-list {
        width: auto;
    }
    #tabs-list-bar, #tabs-list {
        width: auto;
        height: auto;
        min-width: 100%;
        overflow: hidden hidden;
    }
}
"#;

// ---------------------------------------------------------------- messages

/// Posted by [`Tabs`] when a tab becomes active (Textual: `Tabs.TabActivated`).
/// `msg.sender` is the `Tabs` node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TabActivated {
    /// CSS id of the activated tab.
    pub tab_id: String,
    /// Index of the tab among all tabs (including hidden / disabled ones).
    pub index: usize,
    /// Plain-text label of the tab.
    pub label: String,
}

/// Posted by [`Tabs`] when no tab is active any more (Textual: `Tabs.Cleared`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cleared;

/// Posted by [`Tabs`] when a tab is disabled (Textual: `Tabs.TabDisabled`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TabDisabled {
    pub tab_id: String,
}

/// Posted by [`Tabs`] when a tab is enabled (Textual: `Tabs.TabEnabled`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TabEnabled {
    pub tab_id: String,
}

/// Posted by [`Tabs`] when a tab is hidden (Textual: `Tabs.TabHidden`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TabHidden {
    pub tab_id: String,
}

/// Posted by [`Tabs`] when a tab is shown (Textual: `Tabs.TabShown`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TabShown {
    pub tab_id: String,
}

/// Posted by a [`Tab`] when it is clicked (Textual: `Tab.Clicked`). `msg.sender` is the tab.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TabClicked;

/// Posted by the [`Underline`] when clicked (Textual: `Underline.Clicked`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnderlineClicked {
    /// Screen x coordinate of the click.
    pub screen_x: i32,
}

// ---------------------------------------------------------------- Underline

/// The bar beneath the tabs, highlighting the active tab (component class `underline--bar`).
#[derive(Debug, Clone, Default)]
pub struct Underline;

/// Glyphs used by Textual's `Bar` renderable.
const HALF_BAR_LEFT: &str = "╺";
const BAR: &str = "━";
const HALF_BAR_RIGHT: &str = "╸";

/// Render a `Bar` (port of `textual/renderables/bar.py` for integer ranges).
pub(crate) fn render_bar(
    width: i32,
    range: Option<(i32, i32)>,
    highlight: &Style,
    background: &Style,
) -> Strip {
    let mut strip = Strip::default();
    let Some((start, end)) = range else {
        strip.push(Span::new(
            BAR.repeat(width.max(0) as usize),
            background.clone(),
        ));
        return strip;
    };
    let start = start.max(0);
    let end = end.min(width);
    if (start == 0 && end == 0) || end < 0 || start > end {
        strip.push(Span::new(
            BAR.repeat(width.max(0) as usize),
            background.clone(),
        ));
        return strip;
    }
    if start > 0 {
        strip.push(Span::new(
            BAR.repeat((start - 1) as usize),
            background.clone(),
        ));
        strip.push(Span::new(HALF_BAR_RIGHT, background.clone()));
    }
    strip.push(Span::new(
        BAR.repeat((end - start) as usize),
        highlight.clone(),
    ));
    if end != width {
        strip.push(Span::new(HALF_BAR_LEFT, background.clone()));
        strip.push(Span::new(
            BAR.repeat((width - end - 1).max(0) as usize),
            background.clone(),
        ));
    }
    strip
}

impl Underline {
    /// Highlight range (columns relative to the underline's content region) of the active tab.
    fn highlight_range(&self, ctx: &RenderCtx) -> Option<(i32, i32)> {
        let dom = ctx.dom;
        let me = dom.get(ctx.node)?;
        let bar = me.parent?;
        let list = dom
            .children(bar)
            .iter()
            .copied()
            .find(|c| dom.get(*c).and_then(|n| n.css_id.as_deref()) == Some("tabs-list"))?;
        let active = dom.children(list).iter().copied().find(|c| {
            dom.get(*c)
                .map(|n| n.has_class("-active") && !n.has_class("-hidden") && n.layout.visible)
                .unwrap_or(false)
        })?;
        let tab = dom.get(active)?.layout.content_region;
        let origin = me.layout.content_region.x;
        Some((tab.x - origin, tab.x - origin + tab.width))
    }
}

impl Widget for Underline {
    fn type_names(&self) -> &'static [&'static str] {
        &["Underline"]
    }

    fn default_css(&self) -> &'static str {
        UNDERLINE_CSS
    }

    fn on_event(&mut self, event: &Event, ctx: &mut Ctx) -> Flow {
        if let Event::Click(m) = event {
            ctx.post_message(UnderlineClicked {
                screen_x: m.screen_x,
            });
            return Flow::Stop;
        }
        Flow::Continue
    }

    fn content_height(&self, _ctx: &RenderCtx, _width: i32) -> Option<i32> {
        Some(1)
    }

    fn content_width(&self, _ctx: &RenderCtx) -> Option<i32> {
        Some(0)
    }

    fn render(&self, ctx: &RenderCtx) -> Content {
        let bar = ctx.component_style("underline--bar");
        let highlight = Style {
            fg: bar.fg,
            ..Style::default()
        };
        let background = Style {
            fg: bar.bg,
            ..Style::default()
        };
        let range = self.highlight_range(ctx);
        // Sliding: interpolate from where the underline was towards the active tab.
        let tabs = ctx
            .dom
            .ancestors(ctx.node)
            .into_iter()
            .find_map(|a| ctx.dom.widget::<Tabs>(a));
        if let (Some((from, t)), Some((s, e))) = (tabs.and_then(|t| t.underline_slide()), range) {
            let lerp = |a: f32, b: i32| a + (b as f32 - a) * t;
            let bar = BarRenderable::new(
                (lerp(from.0, s) as f64, lerp(from.1, e) as f64),
                highlight,
                background,
                ctx.size.width,
            );
            return Content::Lines(vec![bar.render()]);
        }
        Content::Lines(vec![render_bar(
            ctx.size.width,
            range,
            &highlight,
            &background,
        )])
    }
}

// ---------------------------------------------------------------- Tab

/// A single tab inside [`Tabs`]. Clicking it posts [`TabClicked`].
#[derive(Debug, Clone)]
pub struct Tab {
    label: String,
    tab_id: Option<String>,
}

impl Tab {
    /// A tab with a (markup) label.
    pub fn new(label: impl Into<String>) -> Self {
        Tab {
            label: label.into(),
            tab_id: None,
        }
    }

    /// The CSS id to give this tab when it is added through [`Tabs::new`] / [`Tabs::add_tab`]
    /// (when mounting it yourself, use `.id(..)` instead).
    pub fn tab_id(mut self, id: impl Into<String>) -> Self {
        self.tab_id = Some(id.into());
        self
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    /// Undecorated text of the label.
    pub fn label_text(&self) -> String {
        Text::from_markup(&self.label).plain_text()
    }

    pub fn set_label(&mut self, label: impl Into<String>) {
        self.label = label.into();
    }
}

impl From<&str> for Tab {
    fn from(s: &str) -> Self {
        Tab::new(s)
    }
}

impl From<String> for Tab {
    fn from(s: String) -> Self {
        Tab::new(s)
    }
}

impl Widget for Tab {
    fn type_names(&self) -> &'static [&'static str] {
        &["Tab"]
    }

    fn default_css(&self) -> &'static str {
        TAB_CSS
    }

    fn on_event(&mut self, event: &Event, ctx: &mut Ctx) -> Flow {
        if let Event::Click(_) = event {
            ctx.post_message(TabClicked);
            return Flow::Stop;
        }
        Flow::Continue
    }

    fn content_width(&self, _ctx: &RenderCtx) -> Option<i32> {
        Some(cell_len(&self.label_text()))
    }

    fn content_height(&self, _ctx: &RenderCtx, _width: i32) -> Option<i32> {
        Some(1)
    }

    fn render(&self, ctx: &RenderCtx) -> Content {
        Content::Text(ctx.markup(&self.label))
    }
}

// ---------------------------------------------------------------- Tabs

/// A focusable row of tabs with an underline beneath the active one.
///
/// Keys: `left` / `right` move to the previous / next tab (wrapping). Clicking a tab (or the
/// underline below it) activates it. Posts [`TabActivated`] when the active tab changes and
/// [`Cleared`] when no tab is active.
#[derive(Debug, Clone, Default)]
pub struct Tabs {
    initial: Vec<Tab>,
    first_active: Option<String>,
    active: String,
    counter: usize,
    /// Extra CSS type names (used by `TabbedContent`'s `ContentTabs`).
    content_tabs: bool,
    /// Underline slide: the range it started from (relative to the underline) and progress.
    underline_from: Option<(f32, f32)>,
    underline_t: f32,
}

impl Tabs {
    /// Tabs from labels or [`Tab`] values. Tabs without an id get `tab-1`, `tab-2`, ...
    pub fn new<T: Into<Tab>>(tabs: impl IntoIterator<Item = T>) -> Self {
        Tabs {
            initial: tabs.into_iter().map(Into::into).collect(),
            ..Default::default()
        }
    }

    /// The id of the tab that should be active on start (default: the first tab).
    pub fn active(mut self, tab_id: impl Into<String>) -> Self {
        self.first_active = Some(tab_id.into());
        self
    }

    /// Mark as the `ContentTabs` of a `TabbedContent` (adds the `ContentTabs` CSS type).
    pub(crate) fn content_tabs_kind(mut self) -> Self {
        self.content_tabs = true;
        self
    }

    /// The id of the active tab, or `""` if no tab is active.
    pub fn active_id(&self) -> &str {
        &self.active
    }

    /// While the underline slides: (start range, eased progress 0..1).
    fn underline_slide(&self) -> Option<((f32, f32), f32)> {
        self.underline_from.map(|f| (f, self.underline_t))
    }

    /// Columns (relative to the underline) under `tab`, from the last layout.
    fn underline_span(ctx: &Ctx, tab: NodeId) -> Option<(f32, f32)> {
        let underline = ctx.query_in(ctx.id(), "Underline").into_iter().next()?;
        let origin = ctx.dom().get(underline)?.layout.content_region;
        let t = ctx.dom().get(tab)?;
        if origin.width == 0 || !t.layout.visible {
            return None;
        }
        let r = t.layout.content_region;
        Some(((r.x - origin.x) as f32, (r.right() - origin.x) as f32))
    }

    /// Where the underline is currently drawn (mid-slide or under the active tab).
    fn underline_now(&self, ctx: &Ctx) -> Option<(f32, f32)> {
        let target = Self::underline_span(ctx, self.active_tab(ctx)?)?;
        Some(match self.underline_slide() {
            Some((from, t)) => (
                from.0 + (target.0 - from.0) * t,
                from.1 + (target.1 - from.1) * t,
            ),
            None => target,
        })
    }

    /// Slide the underline from `from` to the active tab (Textual's `_highlight_active`).
    fn slide_underline(&mut self, from: (f32, f32), ctx: &mut Ctx) {
        let own_ctx = ctx
            .dom()
            .get(ctx.id())
            .map(|n| n.is_type("Tabs"))
            .unwrap_or(false);
        if !own_ctx || ctx.animation_level() == AnimationLevel::None {
            return;
        }
        self.underline_from = Some(from);
        self.underline_t = 0.0;
        ctx.animate_value_with::<Tabs>(
            "underline",
            0.0,
            1.0,
            Duration::from_millis(300),
            Easing::DEFAULT,
            AnimationLevel::Basic,
            |tabs, v, _ctx| tabs.underline_t = v,
            Some(Box::new(|tabs: &mut Tabs, _ctx: &mut Ctx| {
                tabs.underline_from = None;
            })),
        );
    }

    fn next_id(&mut self) -> String {
        self.counter += 1;
        format!("tab-{}", self.counter)
    }

    fn make_child(&mut self, tab: Tab) -> Child {
        let id = match &tab.tab_id {
            Some(id) => id.clone(),
            None => self.next_id(),
        };
        tab.id(&id)
    }

    /// The `#tabs-list` container node.
    fn list_node(ctx: &Ctx) -> Option<NodeId> {
        ctx.dom().find_by_css_id(ctx.id(), "tabs-list")
    }

    /// All tab nodes, in order.
    pub fn tab_nodes(ctx: &Ctx) -> Vec<NodeId> {
        let Some(list) = Self::list_node(ctx) else {
            return Vec::new();
        };
        ctx.dom()
            .children(list)
            .iter()
            .copied()
            .filter(|c| ctx.dom().widget::<Tab>(*c).is_some())
            .collect()
    }

    /// Ids of all tabs, in order.
    pub fn tab_ids(&self, ctx: &Ctx) -> Vec<String> {
        Self::tab_nodes(ctx)
            .into_iter()
            .filter_map(|n| ctx.css_id(n).map(String::from))
            .collect()
    }

    /// Number of tabs.
    pub fn tab_count(&self, ctx: &Ctx) -> usize {
        Self::tab_nodes(ctx).len()
    }

    /// The node of the tab with the given id.
    pub fn get_tab(&self, tab_id: &str, ctx: &Ctx) -> Option<NodeId> {
        Self::tab_nodes(ctx)
            .into_iter()
            .find(|n| ctx.css_id(*n) == Some(tab_id))
    }

    /// The node of the active tab.
    pub fn active_tab(&self, ctx: &Ctx) -> Option<NodeId> {
        if self.active.is_empty() {
            return None;
        }
        self.get_tab(&self.active, ctx)
    }

    /// Tabs that can become active: shown and enabled, plus the active tab.
    fn potentially_active(&self, ctx: &Ctx) -> Vec<NodeId> {
        let active = self.active_tab(ctx);
        Self::tab_nodes(ctx)
            .into_iter()
            .filter(|n| {
                let node = ctx.dom().get(*n);
                let hidden = node.map(|n| n.has_class("-hidden")).unwrap_or(true);
                let disabled = node.map(|n| n.disabled).unwrap_or(true);
                !hidden && (!disabled || Some(*n) == active)
            })
            .collect()
    }

    /// The tab to activate if the active tab goes away.
    fn next_active(&self, ctx: &Ctx) -> Option<NodeId> {
        let active = self.active_tab(ctx)?;
        let mut tabs = self.potentially_active(ctx);
        let index = tabs.iter().position(|t| *t == active)?;
        tabs.remove(index);
        tabs.get(index)
            .or_else(|| index.checked_sub(1).and_then(|i| tabs.get(i)))
            .copied()
    }

    /// Add a tab at the end. If it is the first tab, it becomes active. Returns its id.
    pub fn add_tab(&mut self, tab: impl Into<Tab>, ctx: &mut Ctx) -> Option<String> {
        self.insert_tab(None, tab, ctx)
    }

    /// Add a tab at `index` (`None` = at the end). Returns its id.
    pub fn insert_tab(
        &mut self,
        index: Option<usize>,
        tab: impl Into<Tab>,
        ctx: &mut Ctx,
    ) -> Option<String> {
        let child = self.make_child(tab.into());
        self.mount_tab(child, index, ctx)
    }

    /// Mount a tab given as a [`Child`] (e.g. a `Tab` wrapped by another widget) at `index`
    /// (`None` = at the end); it gets an id `tab-N` if it has none. If it is the first tab, it
    /// becomes active. Returns its id.
    pub fn mount_tab(
        &mut self,
        mut child: Child,
        index: Option<usize>,
        ctx: &mut Ctx,
    ) -> Option<String> {
        let list = Self::list_node(ctx)?;
        let from_empty = self.tab_count(ctx) == 0;
        let id = match child.id.clone() {
            Some(id) => id,
            None => {
                let id = self.next_id();
                child.id = Some(id.clone());
                id
            }
        };
        ctx.mount_to(list, child, index);
        if from_empty {
            self.active.clear();
            self.set_active(&id, ctx);
        }
        Some(id)
    }

    /// Add a tab without changing the active tab.
    pub(crate) fn insert_tab_silent(
        &mut self,
        index: Option<usize>,
        tab: Tab,
        ctx: &mut Ctx,
    ) -> Option<String> {
        let list = Self::list_node(ctx)?;
        let child = self.make_child(tab);
        let id = child.id.clone().unwrap_or_default();
        ctx.mount_to(list, child, index);
        Some(id)
    }

    /// Remove a tab. If it was active, the next (or previous) tab becomes active.
    pub fn remove_tab(&mut self, tab_id: &str, ctx: &mut Ctx) {
        let Some(node) = self.get_tab(tab_id, ctx) else {
            return;
        };
        let was_active = Some(node) == self.active_tab(ctx);
        let next = if was_active {
            self.next_active(ctx)
        } else {
            None
        };
        ctx.remove(node);
        if !was_active {
            return;
        }
        let remaining = Self::tab_nodes(ctx).into_iter().any(|n| n != node);
        match next.and_then(|n| ctx.css_id(n).map(String::from)) {
            Some(next) if remaining => {
                self.set_active(&next, ctx);
            }
            _ => {
                self.set_active("", ctx);
            }
        }
    }

    /// Remove all tabs.
    pub fn clear(&mut self, ctx: &mut Ctx) {
        for n in Self::tab_nodes(ctx) {
            ctx.remove(n);
        }
        self.active.clear();
        ctx.post_message(Cleared);
    }

    /// Make the tab with `tab_id` active (`""` deactivates all tabs). Posts [`TabActivated`]
    /// (or [`Cleared`]) if the active tab changed. Returns false if there is no such tab.
    pub fn set_active(&mut self, tab_id: &str, ctx: &mut Ctx) -> bool {
        if tab_id == self.active {
            return true;
        }
        let tabs = Self::tab_nodes(ctx);
        let target = if tab_id.is_empty() {
            None
        } else {
            match tabs.iter().position(|n| ctx.css_id(*n) == Some(tab_id)) {
                Some(i) => Some((i, tabs[i])),
                None => return false,
            }
        };
        let underline_was = if self.active.is_empty() {
            None
        } else {
            self.underline_now(ctx)
        };
        for t in &tabs {
            ctx.remove_class_from(*t, "-active");
        }
        self.active = tab_id.to_string();
        match target {
            Some((index, node)) => {
                ctx.add_class_to(node, "-active");
                if let Some(from) = underline_was {
                    self.slide_underline(from, ctx);
                }
                self.scroll_active_tab(node, ctx);
                let label = ctx
                    .dom()
                    .widget::<Tab>(node)
                    .map(|t| t.label_text())
                    .unwrap_or_default();
                ctx.post_message(TabActivated {
                    tab_id: tab_id.to_string(),
                    index,
                    label,
                });
            }
            None => ctx.post_message(Cleared),
        }
        true
    }

    /// Scroll `#tabs-scroll` so the active tab is centered (based on the last layout).
    fn scroll_active_tab(&self, tab: NodeId, ctx: &mut Ctx) {
        let Some(scroll) = ctx.dom().find_by_css_id(ctx.id(), "tabs-scroll") else {
            return;
        };
        let Some(list) = Self::list_node(ctx) else {
            return;
        };
        let (Some(s), Some(l), Some(t)) = (
            ctx.dom().get(scroll).map(|n| n.layout.content_region),
            ctx.dom().get(list).map(|n| n.layout.region),
            ctx.dom().get(tab).map(|n| n.layout.region),
        ) else {
            return;
        };
        if s.width == 0 || t.width == 0 {
            return;
        }
        let tab_x = t.x - l.x;
        let x = (tab_x + t.width / 2 - s.width / 2).max(0);
        ctx.scroll_node_to(scroll, x, 0);
    }

    fn activate_node(&mut self, node: NodeId, ctx: &mut Ctx) {
        if let Some(id) = ctx.css_id(node).map(String::from) {
            self.set_active(&id, ctx);
        }
    }

    fn move_tab(&mut self, direction: i32, ctx: &mut Ctx) {
        let tabs = self.potentially_active(ctx);
        if tabs.is_empty() {
            return;
        }
        let next = match self
            .active_tab(ctx)
            .and_then(|a| tabs.iter().position(|t| *t == a))
        {
            None => {
                if direction > 0 {
                    tabs[0]
                } else {
                    tabs[tabs.len() - 1]
                }
            }
            Some(i) => {
                let n = tabs.len() as i32;
                tabs[(i as i32 + direction).rem_euclid(n) as usize]
            }
        };
        self.activate_node(next, ctx);
    }

    /// Activate the next enabled tab (wrapping).
    pub fn next_tab(&mut self, ctx: &mut Ctx) {
        self.move_tab(1, ctx);
    }

    /// Activate the previous enabled tab (wrapping).
    pub fn previous_tab(&mut self, ctx: &mut Ctx) {
        self.move_tab(-1, ctx);
    }

    /// Disable a tab. Returns false if there is no such tab.
    pub fn disable(&mut self, tab_id: &str, ctx: &mut Ctx) -> bool {
        let Some(node) = self.get_tab(tab_id, ctx) else {
            return false;
        };
        ctx.set_disabled(node, true);
        ctx.post_message(TabDisabled {
            tab_id: tab_id.to_string(),
        });
        true
    }

    /// Enable a tab. Returns false if there is no such tab.
    pub fn enable(&mut self, tab_id: &str, ctx: &mut Ctx) -> bool {
        let Some(node) = self.get_tab(tab_id, ctx) else {
            return false;
        };
        ctx.set_disabled(node, false);
        ctx.post_message(TabEnabled {
            tab_id: tab_id.to_string(),
        });
        true
    }

    /// Hide a tab (class `-hidden`). If it was active, the next tab becomes active.
    pub fn hide(&mut self, tab_id: &str, ctx: &mut Ctx) -> bool {
        let Some(node) = self.get_tab(tab_id, ctx) else {
            return false;
        };
        if Some(node) == self.active_tab(ctx) {
            let next = self
                .next_active(ctx)
                .and_then(|n| ctx.css_id(n).map(String::from))
                .unwrap_or_default();
            self.set_active(&next, ctx);
        }
        ctx.add_class_to(node, "-hidden");
        ctx.post_message(TabHidden {
            tab_id: tab_id.to_string(),
        });
        true
    }

    /// Show a hidden tab. If no tab is active, it becomes active.
    pub fn show(&mut self, tab_id: &str, ctx: &mut Ctx) -> bool {
        let Some(node) = self.get_tab(tab_id, ctx) else {
            return false;
        };
        ctx.remove_class_from(node, "-hidden");
        ctx.post_message(TabShown {
            tab_id: tab_id.to_string(),
        });
        if self.active.is_empty() {
            self.set_active(tab_id, ctx);
        }
        true
    }
}

impl Widget for Tabs {
    fn type_names(&self) -> &'static [&'static str] {
        if self.content_tabs {
            &["ContentTabs", "Tabs"]
        } else {
            &["Tabs"]
        }
    }

    fn default_css(&self) -> &'static str {
        TABS_CSS
    }

    fn can_focus(&self) -> bool {
        true
    }

    fn compose(&mut self) -> Vec<Child> {
        let tabs: Vec<Tab> = std::mem::take(&mut self.initial);
        let tabs: Vec<Child> = tabs.into_iter().map(|t| self.make_child(t)).collect();
        compose![Container::new()
            .id("tabs-scroll")
            .with(compose![Vertical::new().id("tabs-list-bar").with(
                compose![Horizontal::new().id("tabs-list").with(tabs), Underline]
            )])]
    }

    fn on_mount(&mut self, ctx: &mut Ctx) {
        // Tabs given with `.with(...)` become children of the tab list.
        if let Some(list) = Self::list_node(ctx) {
            for c in ctx.children() {
                if ctx.dom().widget::<Tab>(c).is_some() {
                    if ctx.css_id(c).is_none() {
                        let id = self.next_id();
                        if let Some(n) = ctx.dom_mut().get_mut(c) {
                            n.css_id = Some(id);
                        }
                    }
                    ctx.dom_mut().append_child(list, c);
                }
            }
        }
        let mut target = self.first_active.take().unwrap_or_default();
        if target.is_empty() || self.get_tab(&target, ctx).is_none() {
            target = Self::tab_nodes(ctx)
                .first()
                .and_then(|n| ctx.css_id(*n).map(String::from))
                .unwrap_or_default();
        }
        if !target.is_empty() {
            self.set_active(&target, ctx);
        }
    }

    fn bindings(&self) -> Vec<Binding> {
        vec![
            Binding::new("left", "previous_tab", "Previous tab").hidden(),
            Binding::new("right", "next_tab", "Next tab").hidden(),
        ]
    }

    fn run_action(&mut self, action: &str, _args: &[String], ctx: &mut Ctx) -> bool {
        match action {
            "previous_tab" => self.previous_tab(ctx),
            "next_tab" => self.next_tab(ctx),
            _ => return false,
        }
        true
    }

    fn on_message(&mut self, msg: &Message, ctx: &mut Ctx) -> Flow {
        if msg.is::<TabClicked>() {
            let tabs = Self::tab_nodes(ctx);
            if tabs.contains(&msg.sender) {
                ctx.focus_self();
                self.activate_node(msg.sender, ctx);
                return Flow::Stop;
            }
        } else if let Some(click) = msg.get::<UnderlineClicked>() {
            if ctx.dom().is_ancestor(ctx.id(), msg.sender) {
                ctx.focus_self();
                let hit = Self::tab_nodes(ctx).into_iter().find(|t| {
                    ctx.dom()
                        .get(*t)
                        .map(|n| {
                            !n.disabled
                                && n.layout.visible
                                && click.screen_x >= n.layout.region.x
                                && click.screen_x < n.layout.region.right()
                        })
                        .unwrap_or(false)
                });
                if let Some(t) = hit {
                    self.activate_node(t, ctx);
                }
                return Flow::Stop;
            }
        }
        Flow::Continue
    }
}

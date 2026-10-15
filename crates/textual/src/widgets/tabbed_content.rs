//! `TabbedContent` and `TabPane` (port of `textual/widgets/_tabbed_content.py`).
//!
//! ```ignore
//! TabbedContent::new().initial("b").with(compose![
//!     TabPane::new("Alpha").id("a").with(compose![Label::new("first")]),
//!     TabPane::new("Beta").id("b").with(compose![Label::new("second")]),
//! ])
//! // or, like `TabbedContent("Alpha", "Beta")` in Textual, plain children wrapped in panes:
//! TabbedContent::new().titles(["Alpha", "Beta"]).with(compose![Label::new("1"), Label::new("2")])
//! ```
//!
//! How children arrive: children given with `.with(...)` are mounted as direct children of
//! the `TabbedContent` node (before the `ContentTabs` and `ContentSwitcher` it composes).
//! When the `TabbedContent` is mounted (its children are already mounted by then) it moves
//! every such child into the `ContentSwitcher` — wrapping non-`TabPane` children in a new
//! `TabPane` titled from [`TabbedContent::titles`] — and adds one tab per pane to the
//! `ContentTabs`. This mirrors Textual's `compose_add_child`.
//!
//! Each tab's id is the pane id prefixed with `--content-tab-` (as in Textual). Panes without
//! an id get `tab-1`, `tab-2`, ...
//!
//! Deviation: Textual activates a pane when a widget inside it is focused programmatically
//! (`TabPane.Focused`); this port does not (focus cannot reach hidden panes anyway).

use crate::compose;
use crate::context::Ctx;
use crate::dom::NodeId;
use crate::message::{Flow, Message};
use crate::widget::{Child, Widget};
use crate::widgets::content_switcher::ContentSwitcher;
use crate::widgets::tabs::{self, Tab, Tabs};

const TABBED_CONTENT_CSS: &str = r#"
TabbedContent {
    height: auto;
    &> ContentTabs {
        dock: top;
    }
}
"#;

const TAB_PANE_CSS: &str = r#"
TabPane {
    height: auto;
}
"#;

/// The prefix of the tab ids created for panes.
pub const CONTENT_TAB_PREFIX: &str = "--content-tab-";

/// Tab id for a pane id.
pub fn add_prefix(pane_id: &str) -> String {
    if pane_id.is_empty() {
        String::new()
    } else {
        format!("{CONTENT_TAB_PREFIX}{pane_id}")
    }
}

/// Pane id for a tab id.
pub fn sans_prefix(tab_id: &str) -> &str {
    tab_id.strip_prefix(CONTENT_TAB_PREFIX).unwrap_or(tab_id)
}

/// Posted when the active pane changes (Textual: `TabbedContent.TabActivated`).
/// `msg.sender` is the `TabbedContent`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TabActivated {
    /// Id of the activated `TabPane`.
    pub pane_id: String,
    /// Id of the corresponding tab (`--content-tab-<pane_id>`).
    pub tab_id: String,
    /// Index of the pane.
    pub index: usize,
}

/// Posted when no pane is active (Textual: `TabbedContent.Cleared`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cleared;

// ---------------------------------------------------------------- TabPane

/// A container for one page of a [`TabbedContent`], with the title shown in its tab.
#[derive(Debug, Clone)]
pub struct TabPane {
    title: String,
}

impl TabPane {
    /// A pane with a (markup) title.
    pub fn new(title: impl Into<String>) -> Self {
        TabPane {
            title: title.into(),
        }
    }

    pub fn title(&self) -> &str {
        &self.title
    }
}

impl Widget for TabPane {
    fn type_names(&self) -> &'static [&'static str] {
        &["TabPane"]
    }

    fn default_css(&self) -> &'static str {
        TAB_PANE_CSS
    }
}

// ---------------------------------------------------------------- TabbedContent

/// Tabs plus a content switcher showing the pane of the active tab.
#[derive(Debug, Clone, Default)]
pub struct TabbedContent {
    titles: Vec<String>,
    initial: String,
    active: String,
    counter: usize,
    tabs: Option<NodeId>,
    switcher: Option<NodeId>,
}

impl TabbedContent {
    pub fn new() -> Self {
        Self::default()
    }

    /// Titles for children that are not `TabPane`s (Textual's positional `*titles`).
    pub fn titles<S: Into<String>>(mut self, titles: impl IntoIterator<Item = S>) -> Self {
        self.titles = titles.into_iter().map(Into::into).collect();
        self
    }

    /// Id of the pane to show initially (default: the first).
    pub fn initial(mut self, pane_id: impl Into<String>) -> Self {
        self.initial = pane_id.into();
        self
    }

    /// Id of the active pane, or `""`.
    pub fn active(&self) -> &str {
        &self.active
    }

    /// The `ContentTabs` node.
    pub fn tabs_node(&self) -> Option<NodeId> {
        self.tabs
    }

    /// The `ContentSwitcher` node.
    pub fn switcher_node(&self) -> Option<NodeId> {
        self.switcher
    }

    /// Activate the pane with id `pane_id` (`""` = none). Posts [`TabActivated`] if it changed.
    pub fn set_active(&mut self, pane_id: &str, ctx: &mut Ctx) -> bool {
        let Some(tabs) = self.tabs else { return false };
        let tab_id = add_prefix(pane_id);
        ctx.with_id::<Tabs, _>(tabs, |t, c| t.set_active(&tab_id, c))
            .unwrap_or(false)
    }

    /// Pane nodes, in order.
    pub fn panes(&self, ctx: &Ctx) -> Vec<NodeId> {
        let Some(s) = self.switcher else {
            return Vec::new();
        };
        ctx.dom()
            .children(s)
            .iter()
            .copied()
            .filter(|c| ctx.dom().widget::<TabPane>(*c).is_some())
            .collect()
    }

    pub fn tab_count(&self, ctx: &Ctx) -> usize {
        self.panes(ctx).len()
    }

    /// The pane node with the given id.
    pub fn get_pane(&self, pane_id: &str, ctx: &Ctx) -> Option<NodeId> {
        self.panes(ctx)
            .into_iter()
            .find(|p| ctx.css_id(*p) == Some(pane_id))
    }

    /// The tab node for the given pane id.
    pub fn get_tab(&self, pane_id: &str, ctx: &Ctx) -> Option<NodeId> {
        let tabs = self.tabs?;
        ctx.dom().find_by_css_id(tabs, &add_prefix(pane_id))
    }

    /// The active pane node.
    pub fn active_pane(&self, ctx: &Ctx) -> Option<NodeId> {
        if self.active.is_empty() {
            return None;
        }
        self.get_pane(&self.active, ctx)
    }

    fn next_pane_id(&mut self) -> String {
        self.counter += 1;
        format!("tab-{}", self.counter)
    }

    /// Move `node` (a mounted `TabPane`) into the switcher and create its tab.
    fn adopt_pane(&mut self, node: NodeId, index: Option<usize>, ctx: &mut Ctx) {
        let (Some(tabs), Some(switcher)) = (self.tabs, self.switcher) else {
            return;
        };
        let id = match ctx.css_id(node) {
            Some(id) => id.to_string(),
            None => {
                let id = self.next_pane_id();
                if let Some(n) = ctx.dom_mut().get_mut(node) {
                    n.css_id = Some(id.clone());
                }
                id
            }
        };
        match index {
            Some(i) => ctx.dom_mut().insert_child(switcher, node, i),
            None => ctx.dom_mut().append_child(switcher, node),
        }
        ctx.set_display(node, false);
        let title = ctx
            .dom()
            .widget::<TabPane>(node)
            .map(|p| p.title.clone())
            .unwrap_or_default();
        let disabled = ctx.dom().get(node).map(|n| n.disabled).unwrap_or(false);
        let tab_id = add_prefix(&id);
        let tab = Tab::new(title).tab_id(tab_id.clone());
        ctx.with_id::<Tabs, _>(tabs, |t, c| t.insert_tab_silent(index, tab, c));
        if disabled {
            if let Some(t) = ctx.dom().find_by_css_id(tabs, &tab_id) {
                ctx.set_disabled(t, true);
            }
        }
    }

    /// Add a pane (a `TabPane` child, e.g. `TabPane::new("x").id("x").with(..)`) at the end.
    /// If it is the first pane it becomes active.
    pub fn add_pane(&mut self, pane: impl Into<Child>, ctx: &mut Ctx) {
        let Some(switcher) = self.switcher else {
            return;
        };
        let from_empty = self.tab_count(ctx) == 0;
        let node = ctx.mount_to(switcher, pane, None);
        self.adopt_pane(node, None, ctx);
        if from_empty {
            if let Some(id) = ctx.css_id(node).map(String::from) {
                self.set_active(&id, ctx);
            }
        }
    }

    /// Remove the pane with id `pane_id` (and its tab).
    pub fn remove_pane(&mut self, pane_id: &str, ctx: &mut Ctx) {
        if let Some(tabs) = self.tabs {
            let tab_id = add_prefix(pane_id);
            ctx.with_id::<Tabs, _>(tabs, |t, c| t.remove_tab(&tab_id, c));
        }
        if let Some(p) = self.get_pane(pane_id, ctx) {
            ctx.remove(p);
        }
    }

    /// Remove all panes.
    pub fn clear_panes(&mut self, ctx: &mut Ctx) {
        if let Some(tabs) = self.tabs {
            ctx.with_id::<Tabs, _>(tabs, |t, c| t.clear(c));
        }
        for p in self.panes(ctx) {
            ctx.remove(p);
        }
    }

    fn with_tabs(&self, ctx: &mut Ctx, f: impl FnOnce(&mut Tabs, &mut Ctx) -> bool) -> bool {
        match self.tabs {
            Some(t) => ctx.with_id::<Tabs, _>(t, f).unwrap_or(false),
            None => false,
        }
    }

    /// Disable the tab (and pane) for `pane_id`.
    pub fn disable_tab(&mut self, pane_id: &str, ctx: &mut Ctx) -> bool {
        let id = add_prefix(pane_id);
        self.with_tabs(ctx, |t, c| t.disable(&id, c))
    }

    /// Enable the tab (and pane) for `pane_id`.
    pub fn enable_tab(&mut self, pane_id: &str, ctx: &mut Ctx) -> bool {
        let id = add_prefix(pane_id);
        self.with_tabs(ctx, |t, c| t.enable(&id, c))
    }

    /// Hide the tab for `pane_id`.
    pub fn hide_tab(&mut self, pane_id: &str, ctx: &mut Ctx) -> bool {
        let id = add_prefix(pane_id);
        self.with_tabs(ctx, |t, c| t.hide(&id, c))
    }

    /// Show the tab for `pane_id`.
    pub fn show_tab(&mut self, pane_id: &str, ctx: &mut Ctx) -> bool {
        let id = add_prefix(pane_id);
        self.with_tabs(ctx, |t, c| t.show(&id, c))
    }

    fn set_switcher(&self, current: Option<&str>, ctx: &mut Ctx) {
        if let Some(s) = self.switcher {
            ctx.with_id::<ContentSwitcher, _>(s, |w, c| w.set_current(current, c));
        }
    }

    fn sync_pane_disabled(&self, tab_id: &str, disabled: bool, ctx: &mut Ctx) {
        if let Some(p) = self.get_pane(sans_prefix(tab_id), ctx) {
            ctx.set_disabled(p, disabled);
        }
    }
}

impl Widget for TabbedContent {
    fn type_names(&self) -> &'static [&'static str] {
        &["TabbedContent"]
    }

    fn allow_maximize(&self) -> Option<bool> {
        Some(true)
    }

    fn default_css(&self) -> &'static str {
        TABBED_CONTENT_CSS
    }

    fn compose(&mut self) -> Vec<Child> {
        compose![Tabs::default().content_tabs_kind(), ContentSwitcher::new()]
    }

    fn on_mount(&mut self, ctx: &mut Ctx) {
        let children = ctx.children();
        self.tabs = children.iter().copied().rev().find(|c| {
            ctx.dom()
                .get(*c)
                .map(|n| n.is_type("ContentTabs"))
                .unwrap_or(false)
        });
        self.switcher = children
            .iter()
            .copied()
            .rev()
            .find(|c| ctx.dom().widget::<ContentSwitcher>(*c).is_some());
        let content: Vec<NodeId> = children
            .into_iter()
            .filter(|c| Some(*c) != self.tabs && Some(*c) != self.switcher)
            .collect();
        for (i, node) in content.into_iter().enumerate() {
            let pane = if ctx.dom().widget::<TabPane>(node).is_some() {
                node
            } else {
                let title = self
                    .titles
                    .get(i)
                    .cloned()
                    .unwrap_or_else(|| format!("Tab {}", i + 1));
                let Some(switcher) = self.switcher else {
                    return;
                };
                let pane = ctx.mount_to(switcher, TabPane::new(title), None);
                ctx.dom_mut().append_child(pane, node);
                pane
            };
            self.adopt_pane(pane, None, ctx);
        }
        let initial = std::mem::take(&mut self.initial);
        let first = self
            .panes(ctx)
            .first()
            .and_then(|p| ctx.css_id(*p).map(String::from));
        let target = if !initial.is_empty() && self.get_pane(&initial, ctx).is_some() {
            Some(initial)
        } else {
            first
        };
        if let Some(t) = target {
            self.set_active(&t, ctx);
        }
    }

    fn on_message(&mut self, msg: &Message, ctx: &mut Ctx) -> Flow {
        if self.tabs != Some(msg.sender) {
            return Flow::Continue;
        }
        if let Some(a) = msg.get::<tabs::TabActivated>() {
            let pane_id = sans_prefix(&a.tab_id).to_string();
            self.set_switcher(Some(&pane_id), ctx);
            self.active = pane_id.clone();
            let index = self
                .panes(ctx)
                .iter()
                .position(|p| ctx.css_id(*p) == Some(pane_id.as_str()))
                .unwrap_or(a.index);
            ctx.post_message(TabActivated {
                pane_id,
                tab_id: a.tab_id.clone(),
                index,
            });
            return Flow::Stop;
        }
        if msg.is::<tabs::Cleared>() {
            self.set_switcher(None, ctx);
            self.active.clear();
            ctx.post_message(Cleared);
            return Flow::Stop;
        }
        if let Some(d) = msg.get::<tabs::TabDisabled>() {
            self.sync_pane_disabled(&d.tab_id, true, ctx);
            return Flow::Stop;
        }
        if let Some(e) = msg.get::<tabs::TabEnabled>() {
            self.sync_pane_disabled(&e.tab_id, false, ctx);
            return Flow::Stop;
        }
        Flow::Continue
    }
}

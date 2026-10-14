//! `Collapsible` and `CollapsibleTitle` (port of `textual/widgets/_collapsible.py`).
//!
//! ```ignore
//! Collapsible::new("Details").collapsed(false).with(compose![Label::new("hidden stuff")])
//! ```
//!
//! Children given with `.with(...)` are moved into the inner `Contents` container when the
//! collapsible is mounted (Textual's `compose_add_child`). The title toggles on click or
//! `enter`; the `-collapsed` class hides `Contents` via CSS.

use crate::binding::Binding;
use crate::compose;
use crate::context::Ctx;
use crate::dom::NodeId;
use crate::events::Event;
use crate::message::{Flow, Message};
use crate::widget::{Child, Content, RenderCtx, Widget};
use textual_render::{cells::cell_len, Text};

const TITLE_CSS: &str = r#"
CollapsibleTitle {
    width: auto;
    height: auto;
    padding: 0 1;
    text-style: $block-cursor-blurred-text-style;
    color: $block-cursor-blurred-foreground;
    pointer: pointer;

    &:hover {
        background: $block-hover-background;
        color: $foreground;
    }
    &:focus {
        text-style: $block-cursor-text-style;
        background: $block-cursor-background;
        color: $block-cursor-foreground;
    }
}
"#;

const COLLAPSIBLE_CSS: &str = r#"
Collapsible {
    width: 1fr;
    height: auto;
    background: $surface;
    border-top: hkey $background;
    padding-bottom: 1;
    padding-left: 1;

    &:focus-within {
        background-tint: $foreground 5%;
    }

    &.-collapsed > Contents {
        display: none;
    }
}
"#;

const CONTENTS_CSS: &str = r#"
Contents {
    width: 100%;
    height: auto;
    padding: 1 0 0 3;
    layout: vertical;
}
"#;

/// Posted when a collapsible is expanded (Textual: `Collapsible.Expanded`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Expanded {
    /// CSS id of the collapsible, if any.
    pub collapsible_id: Option<String>,
}

/// Posted when a collapsible is collapsed (Textual: `Collapsible.Collapsed`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Collapsed {
    /// CSS id of the collapsible, if any.
    pub collapsible_id: Option<String>,
}

/// Posted by a [`CollapsibleTitle`] to request a toggle (Textual: `CollapsibleTitle.Toggle`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Toggle;

// ---------------------------------------------------------------- CollapsibleTitle

/// The focusable title bar of a [`Collapsible`]: `<symbol> <label>`.
#[derive(Debug, Clone)]
pub struct CollapsibleTitle {
    label: String,
    collapsed_symbol: String,
    expanded_symbol: String,
    collapsed: bool,
}

impl CollapsibleTitle {
    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn is_collapsed(&self) -> bool {
        self.collapsed
    }

    fn markup(&self) -> String {
        let symbol = if self.collapsed {
            &self.collapsed_symbol
        } else {
            &self.expanded_symbol
        };
        format!("{symbol} {}", self.label)
    }
}

impl Widget for CollapsibleTitle {
    fn type_names(&self) -> &'static [&'static str] {
        &["CollapsibleTitle"]
    }

    fn default_css(&self) -> &'static str {
        TITLE_CSS
    }

    fn can_focus(&self) -> bool {
        true
    }

    fn bindings(&self) -> Vec<Binding> {
        vec![Binding::new("enter", "toggle_collapsible", "Toggle collapsible").hidden()]
    }

    fn run_action(&mut self, action: &str, _args: &[String], ctx: &mut Ctx) -> bool {
        if action == "toggle_collapsible" {
            ctx.post_message(Toggle);
            return true;
        }
        false
    }

    fn on_event(&mut self, event: &Event, ctx: &mut Ctx) -> Flow {
        if let Event::Click(_) = event {
            ctx.post_message(Toggle);
            return Flow::Stop;
        }
        Flow::Continue
    }

    fn content_width(&self, _ctx: &RenderCtx) -> Option<i32> {
        let plain = Text::from_markup(&self.markup()).plain_text();
        Some(plain.lines().map(cell_len).max().unwrap_or(0))
    }

    fn render(&self, ctx: &RenderCtx) -> Content {
        Content::Text(ctx.markup(&self.markup()))
    }
}

// ---------------------------------------------------------------- Contents

/// The container holding a collapsible's children.
#[derive(Debug, Clone, Default)]
pub struct Contents;

impl Widget for Contents {
    fn type_names(&self) -> &'static [&'static str] {
        &["Contents"]
    }

    fn default_css(&self) -> &'static str {
        CONTENTS_CSS
    }
}

// ---------------------------------------------------------------- Collapsible

/// A container whose contents can be shown / hidden by clicking its title.
#[derive(Debug, Clone)]
pub struct Collapsible {
    title: String,
    collapsed: bool,
    collapsed_symbol: String,
    expanded_symbol: String,
    title_node: Option<NodeId>,
    contents_node: Option<NodeId>,
}

impl Default for Collapsible {
    fn default() -> Self {
        Collapsible::new("Toggle")
    }
}

impl Collapsible {
    /// A collapsible (collapsed by default) with a (markup) title.
    pub fn new(title: impl Into<String>) -> Self {
        Collapsible {
            title: title.into(),
            collapsed: true,
            collapsed_symbol: "▶".into(),
            expanded_symbol: "▼".into(),
            title_node: None,
            contents_node: None,
        }
    }

    /// Initial collapsed state.
    pub fn collapsed(mut self, collapsed: bool) -> Self {
        self.collapsed = collapsed;
        self
    }

    /// Symbol shown before the title when collapsed (default `▶`).
    pub fn collapsed_symbol(mut self, s: impl Into<String>) -> Self {
        self.collapsed_symbol = s.into();
        self
    }

    /// Symbol shown before the title when expanded (default `▼`).
    pub fn expanded_symbol(mut self, s: impl Into<String>) -> Self {
        self.expanded_symbol = s.into();
        self
    }

    pub fn is_collapsed(&self) -> bool {
        self.collapsed
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    /// The `Contents` node holding the children.
    pub fn contents_node(&self) -> Option<NodeId> {
        self.contents_node
    }

    /// Change the title.
    pub fn set_title(&mut self, title: impl Into<String>, ctx: &mut Ctx) {
        self.title = title.into();
        self.update_title(ctx);
    }

    /// Collapse or expand. Posts [`Collapsed`] / [`Expanded`] if the state changed.
    pub fn set_collapsed(&mut self, collapsed: bool, ctx: &mut Ctx) {
        if collapsed == self.collapsed {
            return;
        }
        self.collapsed = collapsed;
        self.update_collapsed(ctx);
        let collapsible_id = ctx.css_id(ctx.id()).map(String::from);
        if collapsed {
            ctx.post_message(Collapsed { collapsible_id });
        } else {
            ctx.post_message(Expanded { collapsible_id });
        }
        ctx.scroll_visible();
    }

    pub fn toggle(&mut self, ctx: &mut Ctx) {
        let c = !self.collapsed;
        self.set_collapsed(c, ctx);
    }

    fn update_title(&self, ctx: &mut Ctx) {
        if let Some(t) = self.title_node {
            if let Some(w) = ctx.get::<CollapsibleTitle>(t) {
                w.label = self.title.clone();
                w.collapsed = self.collapsed;
            }
        }
    }

    fn update_collapsed(&self, ctx: &mut Ctx) {
        self.update_title(ctx);
        ctx.set_class(self.collapsed, "-collapsed");
    }
}

impl Widget for Collapsible {
    fn type_names(&self) -> &'static [&'static str] {
        &["Collapsible"]
    }

    fn default_css(&self) -> &'static str {
        COLLAPSIBLE_CSS
    }

    fn compose(&mut self) -> Vec<Child> {
        compose![
            CollapsibleTitle {
                label: self.title.clone(),
                collapsed_symbol: self.collapsed_symbol.clone(),
                expanded_symbol: self.expanded_symbol.clone(),
                collapsed: self.collapsed,
            },
            Contents
        ]
    }

    fn on_mount(&mut self, ctx: &mut Ctx) {
        let children = ctx.children();
        // The composed title and contents come after any `.with(...)` children.
        self.title_node = children
            .iter()
            .copied()
            .rev()
            .find(|c| ctx.dom().widget::<CollapsibleTitle>(*c).is_some());
        self.contents_node = children
            .iter()
            .copied()
            .rev()
            .find(|c| ctx.dom().widget::<Contents>(*c).is_some());
        if let Some(contents) = self.contents_node {
            for c in children {
                if Some(c) != self.title_node && Some(c) != self.contents_node {
                    ctx.dom_mut().append_child(contents, c);
                }
            }
        }
        self.update_collapsed(ctx);
    }

    fn on_message(&mut self, msg: &Message, ctx: &mut Ctx) -> Flow {
        if msg.is::<Toggle>() && Some(msg.sender) == self.title_node {
            self.toggle(ctx);
            return Flow::Stop;
        }
        Flow::Continue
    }
}

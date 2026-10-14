//! `ContentSwitcher` (port of `textual/widgets/_content_switcher.py`).
//!
//! ```ignore
//! ContentSwitcher::new().initial("a").with(compose![
//!     Label::new("A").id("a"),
//!     Label::new("B").id("b"),
//! ])
//! ```
//!
//! Only the child whose CSS id equals [`ContentSwitcher::current`] is displayed; the others
//! get an inline `display: none`. Children without an id are always hidden.

use crate::context::Ctx;
use crate::dom::NodeId;
use crate::widget::{Child, Widget};

const DEFAULT_CSS: &str = r#"
ContentSwitcher {
    width: 1fr;
    height: auto;
    layout: vertical;
    overflow: hidden hidden;
}
"#;

/// A container that displays one of its children at a time.
#[derive(Debug, Clone, Default)]
pub struct ContentSwitcher {
    initial: Option<String>,
    current: Option<String>,
}

impl ContentSwitcher {
    pub fn new() -> Self {
        Self::default()
    }

    /// The id of the child to show initially (default: nothing is shown).
    pub fn initial(mut self, id: impl Into<String>) -> Self {
        let id = id.into();
        self.initial = (!id.is_empty()).then_some(id);
        self
    }

    /// The id of the currently displayed child.
    pub fn current(&self) -> Option<&str> {
        self.current.as_deref()
    }

    /// Show the child with id `current` (or nothing for `None`) and hide the others.
    pub fn set_current(&mut self, current: Option<&str>, ctx: &mut Ctx) {
        self.current = current.filter(|c| !c.is_empty()).map(String::from);
        self.apply(ctx);
    }

    /// The node of the currently displayed child.
    pub fn visible_content(&self, ctx: &Ctx) -> Option<NodeId> {
        let current = self.current.as_deref()?;
        ctx.children()
            .into_iter()
            .find(|c| ctx.css_id(*c) == Some(current))
    }

    /// Mount `child` (hidden, unless `set_current`). The child must have an id.
    pub fn add_content(&mut self, child: impl Into<Child>, set_current: bool, ctx: &mut Ctx) {
        let child = child.into();
        let id = child.id.clone();
        let node = ctx.mount(child);
        ctx.set_display(node, false);
        if set_current {
            self.set_current(id.as_deref(), ctx);
        }
    }

    fn apply(&self, ctx: &mut Ctx) {
        for c in ctx.children() {
            let show = self.current.is_some() && ctx.css_id(c) == self.current.as_deref();
            ctx.set_display(c, show);
        }
    }
}

impl Widget for ContentSwitcher {
    fn type_names(&self) -> &'static [&'static str] {
        &["ContentSwitcher"]
    }

    fn default_css(&self) -> &'static str {
        DEFAULT_CSS
    }

    fn on_mount(&mut self, ctx: &mut Ctx) {
        self.current = self.initial.clone();
        self.apply(ctx);
    }
}

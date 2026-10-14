//! `Tooltip` (port of `textual/widgets/_tooltip.py`): the floating label shown by the
//! runtime's tooltip support (see [`crate::tooltip`]). Not normally created by apps.

use crate::context::Ctx;
use crate::widget::{Content, RenderCtx, Widget};

/// Textual's Tooltip CSS, except that placement (`margin: 1 0`, `offset-x: -50%`,
/// `constrain: inside inflect`) is computed by the runtime, which sets `offset`, `width` and
/// `height` inline; the tooltip is docked in the `_tooltips` layer so it takes no space.
const DEFAULT_CSS: &str = "
Tooltip {
    layer: _tooltips;
    dock: top;
    padding: 1 2;
    background: $panel;
    width: auto;
    height: auto;
    max-width: 40;
    display: none;
}
";

/// The tooltip overlay widget.
#[derive(Debug, Clone, Default)]
pub struct Tooltip {
    content: String,
}

impl Tooltip {
    pub fn new() -> Self {
        Tooltip::default()
    }

    /// The tooltip markup.
    pub fn content(&self) -> &str {
        &self.content
    }

    pub fn update(&mut self, content: impl Into<String>) {
        self.content = content.into();
    }
}

impl Widget for Tooltip {
    fn type_names(&self) -> &'static [&'static str] {
        &["Tooltip"]
    }

    fn default_css(&self) -> &'static str {
        DEFAULT_CSS
    }

    fn on_mount(&mut self, ctx: &mut Ctx) {
        ctx.add_class("-textual-system");
    }

    fn render(&self, ctx: &RenderCtx) -> Content {
        Content::Text(ctx.markup(&self.content))
    }
}

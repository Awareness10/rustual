//! `Placeholder` (port of `textual/widgets/_placeholder.py`): a stand-in widget for designing
//! layouts. Shows its label (or `#id`), its size, or lorem ipsum; clicking cycles the variant.
//!
//! Consecutive placeholders get consecutive background colors from Textual's palette. Textual
//! counts placeholders created per app; textual-rs counts the placeholders already mounted in
//! the app when this one mounts (so the order is document order).

use crate::context::Ctx;
use crate::events::Event;
use crate::message::Flow;
use crate::widget::{Content, RenderCtx, Widget};
use textual_render::{Color, Text};

const DEFAULT_CSS: &str = "
Placeholder {
    content-align: center middle;
    overflow: hidden;
    color: $text;

    &:disabled {
        opacity: 0.7;
    }
}
Placeholder.-text {
    padding: 1;
}
";

/// Background colors cycled through by consecutive placeholders (applied at 50% alpha).
pub const PLACEHOLDER_BACKGROUND_COLORS: [&str; 12] = [
    "#881177", "#aa3355", "#cc6666", "#ee9944", "#eedd00", "#99dd55", "#44dd88", "#22ccbb",
    "#00bbcc", "#0099cc", "#3366bb", "#663399",
];

pub const LOREM_IPSUM_PLACEHOLDER_TEXT: &str = "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Etiam feugiat ac elit sit amet accumsan. Suspendisse bibendum nec libero quis gravida. Phasellus id eleifend ligula. Nullam imperdiet sem tellus, sed vehicula nisl faucibus sit amet. Praesent iaculis tempor ultricies. Sed lacinia, tellus id rutrum lacinia, sapien sapien congue mauris, sit amet pellentesque quam quam vel nisl. Curabitur vulputate erat pellentesque mauris posuere, non dictum risus mattis.";

/// What a placeholder shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PlaceholderVariant {
    /// The label, or `#id`, or `Placeholder`.
    #[default]
    Default,
    /// The content size, e.g. `20 x 5`.
    Size,
    /// Lorem ipsum text.
    Text,
}

impl PlaceholderVariant {
    pub fn name(&self) -> &'static str {
        match self {
            PlaceholderVariant::Default => "default",
            PlaceholderVariant::Size => "size",
            PlaceholderVariant::Text => "text",
        }
    }

    /// The class set on the widget (`-default`, `-size`, `-text`).
    pub fn class(&self) -> &'static str {
        match self {
            PlaceholderVariant::Default => "-default",
            PlaceholderVariant::Size => "-size",
            PlaceholderVariant::Text => "-text",
        }
    }

    /// The next variant in the cycle (default → size → text → default).
    pub fn next(&self) -> PlaceholderVariant {
        match self {
            PlaceholderVariant::Default => PlaceholderVariant::Size,
            PlaceholderVariant::Size => PlaceholderVariant::Text,
            PlaceholderVariant::Text => PlaceholderVariant::Default,
        }
    }
}

/// A placeholder widget. Click it to cycle through its variants.
#[derive(Debug, Clone, Default)]
pub struct Placeholder {
    label: Option<String>,
    variant: PlaceholderVariant,
    color_offset: usize,
}

impl Placeholder {
    pub fn new() -> Self {
        Placeholder::default()
    }

    /// A placeholder showing `label` (instead of its id) in the default variant.
    pub fn with_label(label: impl Into<String>) -> Self {
        Placeholder {
            label: Some(label.into()),
            ..Default::default()
        }
    }

    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn variant(mut self, variant: PlaceholderVariant) -> Self {
        self.variant = variant;
        self
    }

    pub fn get_variant(&self) -> PlaceholderVariant {
        self.variant
    }

    /// Index into [`PLACEHOLDER_BACKGROUND_COLORS`] (mod its length) chosen at mount.
    pub fn color_offset(&self) -> usize {
        self.color_offset
    }

    pub fn set_variant(&mut self, variant: PlaceholderVariant, ctx: &mut Ctx) {
        ctx.remove_class(self.variant.class());
        self.variant = variant;
        ctx.add_class(variant.class());
    }

    /// Switch to the next variant.
    pub fn cycle_variant(&mut self, ctx: &mut Ctx) {
        let next = self.variant.next();
        self.set_variant(next, ctx);
    }
}

impl Widget for Placeholder {
    fn type_names(&self) -> &'static [&'static str] {
        &["Placeholder"]
    }

    fn default_css(&self) -> &'static str {
        DEFAULT_CSS
    }

    fn on_mount(&mut self, ctx: &mut Ctx) {
        let me = ctx.id();
        let ids = ctx.query("Placeholder");
        self.color_offset = ids
            .into_iter()
            .filter(|id| *id != me)
            .filter(|id| ctx.dom().get(*id).map(|n| n.mounted).unwrap_or(false))
            .count();
        let hex =
            PLACEHOLDER_BACKGROUND_COLORS[self.color_offset % PLACEHOLDER_BACKGROUND_COLORS.len()];
        let color = Color::parse(hex).unwrap_or_default().with_alpha(0.5);
        ctx.set_styles(|s| s.background = Some(color));
        ctx.add_class(self.variant.class());
    }

    fn on_event(&mut self, event: &Event, ctx: &mut Ctx) -> Flow {
        if let Event::Click(_) = event {
            self.cycle_variant(ctx);
            return Flow::Stop;
        }
        Flow::Continue
    }

    fn render(&self, ctx: &RenderCtx) -> Content {
        match self.variant {
            PlaceholderVariant::Default => {
                let label = match &self.label {
                    Some(l) => l.clone(),
                    None => match ctx.dom.get(ctx.node).and_then(|n| n.css_id.as_deref()) {
                        Some(id) => format!("#{id}"),
                        None => "Placeholder".to_string(),
                    },
                };
                Content::Text(Text::plain(label))
            }
            PlaceholderVariant::Size => {
                // Textual shows the size from the last `Resize` event (the widget's size).
                let size = ctx
                    .dom
                    .get(ctx.node)
                    .map(|n| n.layout.region.size())
                    .filter(|s| s.area() > 0)
                    .unwrap_or(ctx.size);
                Content::Text(Text::from_markup(&format!(
                    "[b]{} x {}[/b]",
                    size.width, size.height
                )))
            }
            PlaceholderVariant::Text => {
                Content::Text(Text::plain([LOREM_IPSUM_PLACEHOLDER_TEXT; 5].join("\n\n")))
            }
        }
    }
}

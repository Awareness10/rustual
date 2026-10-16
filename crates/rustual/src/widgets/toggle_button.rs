//! `ToggleButton`: the shared core of `Checkbox` and `RadioButton`
//! (port of `textual/widgets/_toggle_button.py`).
//!
//! Renders `▐X▌ label` (the button glyph styled by the `toggle--button` component class,
//! the label by `toggle--label`); `enter` / `space` / click toggle the value and the `-on`
//! class reflects it.

use crate::binding::Binding;
use crate::context::Ctx;
use crate::events::Event;
use crate::message::Flow;
use crate::widget::{Content, RenderCtx, Widget};
use rustual_render::{Span, Style, Text};

pub(crate) const DEFAULT_CSS: &str = r#"
ToggleButton {
    width: auto;
    height: auto;
    border: tall $border-blurred;
    padding: 0 1;
    background: $surface;
    text-wrap: nowrap;
    text-overflow: ellipsis;
    pointer: pointer;

    &.-textual-compact {
        border: none !important;
        padding: 0;
        &:focus {
            border: tall $border;
            background-tint: $foreground 5%;
            & > .toggle--label {
                color: $block-cursor-foreground;
                background: $block-cursor-background;
                text-style: $block-cursor-text-style;
            }
        }
    }

    & > .toggle--button {
        color: $panel-darken-2;
        background: $panel;
    }

    &.-on > .toggle--button {
        color: $text-success;
        background: $panel;
    }

    &:focus {
        border: tall $border;
        background-tint: $foreground 5%;

        & > .toggle--label {
            color: $block-cursor-foreground;
            background: $block-cursor-background;
            text-style: $block-cursor-text-style;
        }
    }
    &:blur:hover {
        & > .toggle--label {
            background: $block-hover-background;
        }
    }
}

/* ANSI themes (Textual `:ansi`). */
ToggleButton {
    &:ansi {
        background: ansi_default;
        & > .toggle--button {
            background: $ansi-background;
            color: $ansi-foreground;
            text-style: dim;
        }
        &.-on > .toggle--button {
            color: $accent;
            background: $ansi-background;
            text-style: bold not dim;
        }
    }
}
"#;

/// Posted when a plain `ToggleButton` changes (Textual: `ToggleButton.Changed`).
/// `Checkbox` and `RadioButton` post their own `Changed` messages instead.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Changed {
    pub value: bool,
    pub toggle_button_id: Option<String>,
}

/// Which concrete widget a [`ToggleButton`] core belongs to (decides the posted message).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ToggleKind {
    Toggle,
    Checkbox,
    Radio,
}

/// A labelled on/off toggle. Usually used through `Checkbox` or `RadioButton`.
#[derive(Debug, Clone)]
pub struct ToggleButton {
    label: String,
    value: bool,
    button_first: bool,
    compact: bool,
    inner: &'static str,
    pub(crate) kind: ToggleKind,
    /// Whether the widget takes focus (a `RadioSet` takes over focus of its buttons).
    pub(crate) focusable: bool,
}

/// Left side glyph of the button.
pub const BUTTON_LEFT: &str = "▐";
/// Right side glyph of the button.
pub const BUTTON_RIGHT: &str = "▌";

impl ToggleButton {
    /// A toggle with a (markup) label; only its first line is used.
    pub fn new(label: impl Into<String>) -> Self {
        Self::with_kind(label.into(), "X", ToggleKind::Toggle)
    }

    pub(crate) fn with_kind(label: String, inner: &'static str, kind: ToggleKind) -> Self {
        ToggleButton {
            label: first_line(label),
            value: false,
            button_first: true,
            compact: false,
            inner,
            kind,
            focusable: true,
        }
    }

    /// Initial value (no `Changed` message is posted for it).
    pub fn value(mut self, value: bool) -> Self {
        self.value = value;
        self
    }

    /// Put the button before (default) or after the label.
    pub fn button_first(mut self, first: bool) -> Self {
        self.button_first = first;
        self
    }

    /// Compact style (no border or padding).
    pub fn compact(mut self, compact: bool) -> Self {
        self.compact = compact;
        self
    }

    pub fn get_value(&self) -> bool {
        self.value
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    /// The label without markup.
    pub fn plain_label(&self) -> String {
        Text::from_markup(&self.label).plain_text()
    }

    pub fn set_label(&mut self, label: impl Into<String>) {
        self.label = first_line(label.into());
    }

    /// Set the value; updates the `-on` class and posts `Changed` if it differs.
    pub fn set_value(&mut self, value: bool, ctx: &mut Ctx) {
        if self.value == value {
            return;
        }
        self.value = value;
        ctx.set_class(value, "-on");
        let id = ctx.css_id(ctx.id()).map(String::from);
        match self.kind {
            ToggleKind::Toggle => ctx.post_message(Changed {
                value,
                toggle_button_id: id,
            }),
            ToggleKind::Checkbox => ctx.post_message(super::checkbox::Changed {
                value,
                checkbox_id: id,
            }),
            ToggleKind::Radio => ctx.post_message(super::radio_button::Changed {
                value,
                radio_button_id: id,
            }),
        }
    }

    /// Set the value without posting a message (Textual: `with self.prevent(Changed)`).
    pub fn set_value_silent(&mut self, value: bool, ctx: &mut Ctx) {
        self.value = value;
        ctx.set_class(value, "-on");
    }

    pub fn toggle(&mut self, ctx: &mut Ctx) {
        let v = !self.value;
        self.set_value(v, ctx);
    }

    fn button_width(&self) -> i32 {
        rustual_render::cells::cell_len(self.inner) + 2
    }

    pub(crate) fn mount(&mut self, ctx: &mut Ctx) {
        ctx.set_class(self.value, "-on");
        ctx.set_class(self.compact, "-textual-compact");
    }

    pub(crate) fn bindings() -> Vec<Binding> {
        vec![Binding::new("enter,space", "toggle_button", "Toggle").hidden()]
    }

    pub(crate) fn action(&mut self, action: &str, ctx: &mut Ctx) -> bool {
        if action == "toggle_button" {
            self.toggle(ctx);
            return true;
        }
        false
    }

    pub(crate) fn event(&mut self, event: &Event, ctx: &mut Ctx) -> Flow {
        if let Event::Click(_) = event {
            // Not stopped (like Textual), so a containing `RadioSet` also sees the click.
            self.toggle(ctx);
        }
        Flow::Continue
    }

    pub(crate) fn content_width_impl(&self) -> i32 {
        let label = self.plain_label();
        let lw = rustual_render::cells::cell_len(&label);
        self.button_width() + if label.is_empty() { 0 } else { 2 } + lw
    }

    pub(crate) fn render_impl(&self, ctx: &RenderCtx) -> Content {
        let button_style = ctx.component_style("toggle--button");
        // The side glyphs are half blocks: their foreground is the button's background.
        let side_style = Style {
            fg: button_style.bg,
            bg: Some(ctx.background()),
            ..Style::default()
        };
        let mut button = Text::new();
        button
            .spans
            .push(Span::new(BUTTON_LEFT, side_style.clone()));
        button.spans.push(Span::new(self.inner, button_style));
        button.spans.push(Span::new(BUTTON_RIGHT, side_style));

        let label_style = ctx.component_style("toggle--label");
        let mut label = Text::new();
        label.push(" ", Style::default());
        label.append(ctx.markup(&self.label));
        label.push(" ", Style::default());
        label.stylize_base(&label_style);

        let mut out = Text::new();
        if self.button_first {
            out.append(button);
            out.append(label);
        } else {
            out.append(label);
            out.append(button);
        }
        Content::Text(out)
    }
}

fn first_line(label: String) -> String {
    match label.lines().next() {
        Some(l) => l.trim_end().to_string(),
        None => String::new(),
    }
}

/// Implements `Widget` for a newtype wrapping a [`ToggleButton`] in field `.0`.
macro_rules! toggle_widget {
    ($name:ident, [$($type_name:literal),+]) => {
        impl $crate::widget::Widget for $name {
            fn type_names(&self) -> &'static [&'static str] {
                &[$($type_name),+]
            }
            fn default_css(&self) -> &'static str {
                $crate::widgets::toggle_button::DEFAULT_CSS
            }
            fn can_focus(&self) -> bool {
                self.0.focusable
            }
            fn on_mount(&mut self, ctx: &mut $crate::context::Ctx) {
                self.0.mount(ctx);
            }
            fn bindings(&self) -> Vec<$crate::binding::Binding> {
                $crate::widgets::toggle_button::ToggleButton::bindings()
            }
            fn run_action(
                &mut self,
                action: &str,
                _args: &[String],
                ctx: &mut $crate::context::Ctx,
            ) -> bool {
                self.0.action(action, ctx)
            }
            fn on_event(
                &mut self,
                event: &$crate::events::Event,
                ctx: &mut $crate::context::Ctx,
            ) -> $crate::message::Flow {
                self.0.event(event, ctx)
            }
            fn content_width(&self, _ctx: &$crate::widget::RenderCtx) -> Option<i32> {
                Some(self.0.content_width_impl())
            }
            fn content_height(&self, _ctx: &$crate::widget::RenderCtx, _w: i32) -> Option<i32> {
                Some(1)
            }
            fn render(&self, ctx: &$crate::widget::RenderCtx) -> $crate::widget::Content {
                self.0.render_impl(ctx)
            }
        }

        impl std::ops::Deref for $name {
            type Target = $crate::widgets::toggle_button::ToggleButton;
            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }

        impl std::ops::DerefMut for $name {
            fn deref_mut(&mut self) -> &mut Self::Target {
                &mut self.0
            }
        }
    };
}
pub(crate) use toggle_widget;

impl Widget for ToggleButton {
    fn type_names(&self) -> &'static [&'static str] {
        &["ToggleButton", "Static"]
    }
    fn default_css(&self) -> &'static str {
        DEFAULT_CSS
    }
    fn can_focus(&self) -> bool {
        self.focusable
    }
    fn on_mount(&mut self, ctx: &mut Ctx) {
        self.mount(ctx);
    }
    fn bindings(&self) -> Vec<Binding> {
        ToggleButton::bindings()
    }
    fn run_action(&mut self, action: &str, _args: &[String], ctx: &mut Ctx) -> bool {
        self.action(action, ctx)
    }
    fn on_event(&mut self, event: &Event, ctx: &mut Ctx) -> Flow {
        self.event(event, ctx)
    }
    fn content_width(&self, _ctx: &RenderCtx) -> Option<i32> {
        Some(self.content_width_impl())
    }
    fn content_height(&self, _ctx: &RenderCtx, _width: i32) -> Option<i32> {
        Some(1)
    }
    fn render(&self, ctx: &RenderCtx) -> Content {
        self.render_impl(ctx)
    }
}

//! `Button` (port of `textual/widgets/_button.py`).

use crate::binding::Binding;
use crate::context::Ctx;
use crate::events::Event;
use crate::message::Flow;
use crate::widget::{Content, RenderCtx, Widget};
use rustual_render::{cells::cell_len, Text};
use std::time::Duration;

const DEFAULT_CSS: &str = r#"
Button {
    width: auto;
    min-width: 16;
    height:auto;
    line-pad: 1;
    text-align: center;
    content-align: center middle;
    pointer: pointer;

    &.-style-flat {
        text-style: bold;
        color: auto 90%;
        background: $surface;
        border: block $surface;
        &:hover {
            background: $primary;
            border: block $primary;
        }
        &:focus {
            text-style: $button-focus-text-style;
        }
        &.-active {
            background: $surface;
            border: block $surface;
            tint: $background 30%;
        }
        &:disabled {
            color: auto 50%;
            pointer: not-allowed;
        }

        &.-primary {
            background: $primary-muted;
            border: block $primary-muted;
            color: $text-primary;
            &:hover {
                color: $text;
                background: $primary;
                border: block $primary;
            }
        }
        &.-success {
            background: $success-muted;
            border: block $success-muted;
            color: $text-success;
            &:hover {
                color: $text;
                background: $success;
                border: block $success;
            }
        }
        &.-warning {
            background: $warning-muted;
            border: block $warning-muted;
            color: $text-warning;
            &:hover {
                color: $text;
                background: $warning;
                border: block $warning;
            }
        }
        &.-error {
            background: $error-muted;
            border: block $error-muted;
            color: $text-error;
            &:hover {
                color: $text;
                background: $error;
                border: block $error;
            }
        }
    }
    &.-style-default {
        text-style: bold;
        color: $button-foreground;
        background: $surface;
        border: none;
        border-top: tall $surface-lighten-1;
        border-bottom: tall $surface-darken-1;


        &.-textual-compact {
            border: none !important;
        }

        &:disabled {
            text-opacity: 0.6;
            pointer: not-allowed;
        }

        &:focus {
            text-style: $button-focus-text-style;
            background-tint: $foreground 5%;
        }
        &:hover {
            border-top: tall $surface;
            background: $surface-darken-1;
        }

        &.-active {
            background: $surface;
            border-bottom: tall $surface-lighten-1;
            border-top: tall $surface-darken-1;
            tint: $background 30%;
        }

        &.-primary {
            color: $button-color-foreground;
            background: $primary;
            border-top: tall $primary-lighten-3;
            border-bottom: tall $primary-darken-3;

            &:hover {
                background: $primary-darken-2;
                border-top: tall $primary;
            }

            &.-active {
                background: $primary;
                border-bottom: tall $primary-lighten-3;
                border-top: tall $primary-darken-3;
            }
        }

        &.-success {
            color: $button-color-foreground;
            background: $success;
            border-top: tall $success-lighten-2;
            border-bottom: tall $success-darken-3;

            &:hover {
                background: $success-darken-2;
                border-top: tall $success;
            }

            &.-active {
                background: $success;
                border-bottom: tall $success-lighten-2;
                border-top: tall $success-darken-2;
            }
        }

        &.-warning{
            color: $button-color-foreground;
            background: $warning;
            border-top: tall $warning-lighten-2;
            border-bottom: tall $warning-darken-3;

            &:hover {
                background: $warning-darken-2;
                border-top: tall $warning;
            }

            &.-active {
                background: $warning;
                border-bottom: tall $warning-lighten-2;
                border-top: tall $warning-darken-2;
            }
        }

        &.-error {
            color: $button-color-foreground;
            background: $error;
            border-top: tall $error-lighten-2;
            border-bottom: tall $error-darken-3;

            &:hover {
                background: $error-darken-1;
                border-top: tall $error;
            }

            &.-active {
                background: $error;
                border-bottom: tall $error-lighten-2;
                border-top: tall $error-darken-2;
            }
        }
    }
}
"#;

/// Posted when a button is pressed (Textual: `Button.Pressed`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pressed {
    /// The CSS id of the pressed button, if it has one.
    pub button_id: Option<String>,
    /// The button's label (plain text).
    pub label: String,
}

/// Visual variant; sets the `-<variant>` class.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ButtonVariant {
    #[default]
    Default,
    Primary,
    Success,
    Warning,
    Error,
}

impl ButtonVariant {
    pub fn class(&self) -> &'static str {
        match self {
            ButtonVariant::Default => "-default",
            ButtonVariant::Primary => "-primary",
            ButtonVariant::Success => "-success",
            ButtonVariant::Warning => "-warning",
            ButtonVariant::Error => "-error",
        }
    }
}

/// A clickable button. Press with a click, `enter`, or [`Button::press`].
#[derive(Debug, Clone)]
pub struct Button {
    label: String,
    variant: ButtonVariant,
    flat: bool,
    /// Optional action string run instead of posting [`Pressed`].
    action: Option<String>,
    active_effect: Duration,
    active: bool,
}

impl Button {
    /// A button with a (markup) label.
    pub fn new(label: impl Into<String>) -> Self {
        Button {
            label: label.into(),
            variant: ButtonVariant::Default,
            flat: false,
            action: None,
            active_effect: Duration::from_millis(200),
            active: false,
        }
    }

    pub fn primary(label: impl Into<String>) -> Self {
        Self::new(label).variant(ButtonVariant::Primary)
    }
    pub fn success(label: impl Into<String>) -> Self {
        Self::new(label).variant(ButtonVariant::Success)
    }
    pub fn warning(label: impl Into<String>) -> Self {
        Self::new(label).variant(ButtonVariant::Warning)
    }
    pub fn error(label: impl Into<String>) -> Self {
        Self::new(label).variant(ButtonVariant::Error)
    }

    pub fn variant(mut self, variant: ButtonVariant) -> Self {
        self.variant = variant;
        self
    }

    /// Flat style (no 3D borders).
    pub fn flat(mut self, flat: bool) -> Self {
        self.flat = flat;
        self
    }

    /// Run `action` (e.g. `"app.quit"`) on press instead of posting `Pressed`.
    pub fn action(mut self, action: &str) -> Self {
        self.action = Some(action.to_string());
        self
    }

    /// Duration of the `-active` press effect (zero disables it).
    pub fn active_effect(mut self, d: Duration) -> Self {
        self.active_effect = d;
        self
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn set_label(&mut self, label: impl Into<String>) {
        self.label = label.into();
    }

    pub fn get_variant(&self) -> ButtonVariant {
        self.variant
    }

    /// Change the variant (needs a ctx to update classes).
    pub fn set_variant(&mut self, variant: ButtonVariant, ctx: &mut Ctx) {
        ctx.remove_class(self.variant.class());
        self.variant = variant;
        ctx.add_class(variant.class());
    }

    /// Simulate a press: show the active effect and post [`Pressed`] (or run the action).
    pub fn press(&mut self, ctx: &mut Ctx) {
        if ctx.is_disabled() || self.active {
            return;
        }
        if !self.active_effect.is_zero() {
            self.active = true;
            ctx.add_class("-active");
            ctx.set_timer(self.active_effect, |b: &mut Button, ctx| {
                b.active = false;
                ctx.remove_class("-active");
            });
        }
        match &self.action {
            Some(action) => ctx.run_action(action),
            None => {
                let label = Text::from_markup(&self.label).plain_text();
                let button_id = ctx.css_id(ctx.id()).map(String::from);
                ctx.post_message(Pressed { button_id, label });
            }
        }
    }
}

impl Widget for Button {
    fn type_names(&self) -> &'static [&'static str] {
        &["Button"]
    }

    fn default_css(&self) -> &'static str {
        DEFAULT_CSS
    }

    fn can_focus(&self) -> bool {
        true
    }

    fn on_mount(&mut self, ctx: &mut Ctx) {
        ctx.add_class(self.variant.class());
        ctx.set_class(self.flat, "-style-flat");
        ctx.set_class(!self.flat, "-style-default");
    }

    fn bindings(&self) -> Vec<Binding> {
        vec![Binding::new("enter", "press", "Press button").hidden()]
    }

    fn run_action(&mut self, action: &str, _args: &[String], ctx: &mut Ctx) -> bool {
        if action == "press" {
            self.press(ctx);
            return true;
        }
        false
    }

    fn on_event(&mut self, event: &Event, ctx: &mut Ctx) -> Flow {
        if let Event::Click(_) = event {
            self.press(ctx);
            return Flow::Stop;
        }
        Flow::Continue
    }

    fn content_width(&self, _ctx: &RenderCtx) -> Option<i32> {
        let plain = Text::from_markup(&self.label).plain_text();
        Some(plain.lines().map(cell_len).max().unwrap_or(0) + 2)
    }

    fn render(&self, ctx: &RenderCtx) -> Content {
        Content::Text(ctx.markup(&self.label))
    }
}

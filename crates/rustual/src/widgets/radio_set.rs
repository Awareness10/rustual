//! `RadioSet` (port of `textual/widgets/_radio_set.py`): groups [`RadioButton`]s so that at most
//! one is pressed. The set takes focus itself (its buttons don't) and moves a `-selected`
//! highlight with the arrow keys; `enter` / `space` press the highlighted button.
//!
//! ```
//! # use rustual::prelude::*;
//! let colors = RadioSet::new(["Red", "Green", "Blue"]).id("colors");
//! // or with explicit buttons:
//! let ab = RadioSet::default().with(compose![RadioButton::new("A"), RadioButton::new("B").value(true)]);
//! ```

use super::radio_button::{self, RadioButton};
use crate::binding::Binding;
use crate::context::Ctx;
use crate::dom::NodeId;
use crate::events::Event;
use crate::message::{Flow, Message};
use crate::widget::{Child, Widget};

const DEFAULT_CSS: &str = r#"
RadioSet {
    layout: vertical;
    overflow-x: hidden;
    overflow-y: auto;
    border: tall $border-blurred;
    background: $surface;
    padding: 0 1;
    height: auto;
    width: 1fr;
    pointer: pointer;

    &.-textual-compact {
        border: none !important;
        padding: 0;
    }

    & > RadioButton {
        background: transparent;
        border: none;
        padding: 0;
        width: 1fr;

        & > .toggle--button {
            color: $panel-darken-2;
            background: $panel;
        }
    }

    & > RadioButton.-on .toggle--button {
        color: $text-success;
    }

    &:blur {
        & > RadioButton.-selected {
            & > .toggle--label {
                background: $block-cursor-blurred-background;
            }
        }
    }

    &:focus {
        /* The following rules/styles mimic similar ToggleButton:focus rules in
        * ToggleButton. If those styles ever get updated, these should be too.
        */
        border: tall $border;
        background-tint: $foreground 5%;
        & > RadioButton.-selected {
            & > .toggle--label {
                background: $block-cursor-background;
                color: $block-cursor-foreground;
                text-style: $block-cursor-text-style;
            }
        }
    }
}
"#;

/// Posted when the pressed button in the set changes (Textual: `RadioSet.Changed`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Changed {
    /// Index of the pressed button among the set's radio buttons.
    pub pressed_index: usize,
    /// Plain-text label of the pressed button.
    pub pressed_label: String,
    /// Node of the pressed button.
    pub pressed: NodeId,
    /// The CSS id of the radio set, if any.
    pub radio_set_id: Option<String>,
}

/// A mutually exclusive group of radio buttons.
#[derive(Debug, Clone, Default)]
pub struct RadioSet {
    labels: Vec<String>,
    compact: bool,
    selected: Option<usize>,
    pressed: Option<NodeId>,
    pressed_index: Option<usize>,
}

impl RadioSet {
    /// A set with one radio button per label (more can be added with `.with([...])`).
    pub fn new<S: Into<String>>(labels: impl IntoIterator<Item = S>) -> Self {
        RadioSet {
            labels: labels.into_iter().map(Into::into).collect(),
            ..Default::default()
        }
    }

    /// Compact style (no border or padding).
    pub fn compact(mut self, compact: bool) -> Self {
        self.compact = compact;
        self
    }

    /// Index of the pressed button, if any.
    pub fn pressed_index(&self) -> Option<usize> {
        self.pressed_index
    }

    /// Node of the pressed button, if any.
    pub fn pressed_button(&self) -> Option<NodeId> {
        self.pressed
    }

    /// Index of the highlighted (keyboard-selected) button.
    pub fn selected_index(&self) -> Option<usize> {
        self.selected
    }

    /// The radio buttons of this set, in order.
    pub fn buttons(&self, ctx: &Ctx) -> Vec<NodeId> {
        ctx.children()
            .into_iter()
            .filter(|id| ctx.dom().widget::<RadioButton>(*id).is_some())
            .collect()
    }

    fn update_selected(&mut self, ctx: &mut Ctx) {
        let buttons = self.buttons(ctx);
        for (i, id) in buttons.iter().enumerate() {
            ctx.set_class_on(*id, Some(i) == self.selected, "-selected");
        }
        if let Some(id) = self.selected.and_then(|i| buttons.get(i).copied()) {
            ctx.with_id::<RadioButton, _>(id, |_, ctx| ctx.scroll_visible());
        }
    }

    /// Move the highlight by `direction` (±1) to the next enabled button, wrapping around
    /// (Textual's `_widget_navigation.find_next_enabled`).
    fn navigate(&mut self, direction: i32, ctx: &mut Ctx) {
        let buttons = self.buttons(ctx);
        let enabled: Vec<bool> = buttons
            .iter()
            .map(|id| !ctx.dom().get(*id).map(|n| n.disabled).unwrap_or(true))
            .collect();
        let n = buttons.len() as i32;
        if n == 0 {
            self.selected = None;
            return;
        }
        self.selected = match self.selected {
            None => {
                if direction > 0 {
                    enabled.iter().position(|e| *e)
                } else {
                    enabled.iter().rposition(|e| *e)
                }
            }
            Some(anchor) => {
                let mut found = Some(anchor);
                for step in 1..=n {
                    let i = (anchor as i32 + direction * step).rem_euclid(n) as usize;
                    if enabled[i] {
                        found = Some(i);
                        break;
                    }
                }
                found
            }
        };
        self.update_selected(ctx);
    }

    pub fn action_next_button(&mut self, ctx: &mut Ctx) {
        self.navigate(1, ctx);
    }

    pub fn action_previous_button(&mut self, ctx: &mut Ctx) {
        self.navigate(-1, ctx);
    }

    /// Toggle the highlighted button (pressing it, if it isn't already).
    pub fn action_toggle_button(&mut self, ctx: &mut Ctx) {
        let buttons = self.buttons(ctx);
        if let Some(id) = self.selected.and_then(|i| buttons.get(i).copied()) {
            ctx.with_id::<RadioButton, _>(id, |b, ctx| b.toggle(ctx));
        }
    }
}

impl Widget for RadioSet {
    fn type_names(&self) -> &'static [&'static str] {
        &["RadioSet", "VerticalScroll", "ScrollableContainer"]
    }

    fn allow_maximize(&self) -> Option<bool> {
        Some(true)
    }

    fn default_css(&self) -> &'static str {
        DEFAULT_CSS
    }

    fn can_focus(&self) -> bool {
        true
    }

    fn can_focus_children(&self) -> bool {
        false
    }

    fn compose(&mut self) -> Vec<Child> {
        self.labels
            .iter()
            .map(|l| Child::from(RadioButton::new(l.clone())))
            .collect()
    }

    fn on_mount(&mut self, ctx: &mut Ctx) {
        ctx.set_class(self.compact, "-textual-compact");
        let buttons = self.buttons(ctx);
        // The set handles focus and movement between its buttons.
        for id in &buttons {
            if let Some(b) = ctx.get::<RadioButton>(*id) {
                b.set_focusable(false);
            }
            if let Some(node) = ctx.dom_mut().get_mut(*id) {
                node.can_focus = false;
            }
        }
        self.action_next_button(ctx);
        // If several buttons start pressed, keep only the first.
        let on: Vec<(usize, NodeId)> = buttons
            .iter()
            .enumerate()
            .filter(|(_, id)| {
                ctx.dom()
                    .widget::<RadioButton>(**id)
                    .map(|b| b.get_value())
                    .unwrap_or(false)
            })
            .map(|(i, id)| (i, *id))
            .collect();
        for (_, id) in on.iter().skip(1) {
            ctx.with_id::<RadioButton, _>(*id, |b, ctx| b.set_value_silent(false, ctx));
        }
        if let Some((i, id)) = on.first() {
            self.pressed = Some(*id);
            self.pressed_index = Some(*i);
        }
    }

    fn bindings(&self) -> Vec<Binding> {
        vec![
            Binding::new("down,right", "next_button", "Next option").hidden(),
            Binding::new("enter,space", "toggle_button", "Toggle").hidden(),
            Binding::new("up,left", "previous_button", "Previous option").hidden(),
        ]
    }

    fn run_action(&mut self, action: &str, _args: &[String], ctx: &mut Ctx) -> bool {
        match action {
            "next_button" => self.action_next_button(ctx),
            "previous_button" => self.action_previous_button(ctx),
            "toggle_button" => self.action_toggle_button(ctx),
            _ => return false,
        }
        true
    }

    fn on_event(&mut self, event: &Event, ctx: &mut Ctx) -> Flow {
        if let Event::Click(_) = event {
            ctx.focus_self();
        }
        Flow::Continue
    }

    fn on_message(&mut self, msg: &Message, ctx: &mut Ctx) -> Flow {
        let Some(changed) = msg.get::<radio_button::Changed>() else {
            return Flow::Continue;
        };
        let sender = msg.sender;
        let buttons = self.buttons(ctx);
        let Some(index) = buttons.iter().position(|b| *b == sender) else {
            return Flow::Continue;
        };
        // Consume the button's message; the set reports its own `Changed` instead.
        if changed.value {
            if let Some(prev) = self.pressed.filter(|p| *p != sender) {
                ctx.with_id::<RadioButton, _>(prev, |b, ctx| b.set_value_silent(false, ctx));
            }
            self.pressed = Some(sender);
            self.pressed_index = Some(index);
            let pressed_label = ctx
                .dom()
                .widget::<RadioButton>(sender)
                .map(|b| b.plain_label())
                .unwrap_or_default();
            let radio_set_id = ctx.css_id(ctx.id()).map(String::from);
            ctx.post_message(Changed {
                pressed_index: index,
                pressed_label,
                pressed: sender,
                radio_set_id,
            });
            self.selected = Some(index);
            self.update_selected(ctx);
        } else {
            // A pressed button can't be clicked off.
            ctx.with_id::<RadioButton, _>(sender, |b, ctx| b.set_value_silent(true, ctx));
        }
        Flow::Stop
    }
}

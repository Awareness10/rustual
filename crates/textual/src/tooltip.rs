//! Tooltips (port of the tooltip handling in Textual's `screen.py`).
//!
//! A widget's tooltip is set with [`Child::tooltip`](crate::Child::tooltip) /
//! [`WidgetExt::tooltip`](crate::WidgetExt::tooltip) or [`Ctx::set_tooltip`]. When the mouse
//! rests over a widget (or a descendant of one) with a tooltip for [`TOOLTIP_DELAY`], a
//! [`Tooltip`](crate::widgets::Tooltip) is shown near the pointer: centered horizontally on
//! it, one row below, flipped above when there is no room, and kept on screen (Textual's
//! `margin: 1 0; offset-x: -50%; constrain: inside inflect`).
//!
//! As in Textual, any mouse movement restarts the delay; moving while a tooltip is shown
//! hides it; key presses and mouse buttons hide it too.
//!
//! The `Tooltip` widget is mounted lazily (one per screen, id `textual-tooltip`) as the last
//! child of the screen, docked in its own `_tooltips` layer so it overlays everything.

use crate::app::{AppState, MouseKind, TimerId};
use crate::context::Ctx;
use crate::dom::NodeId;
use crate::widget::Child;
use crate::widgets::tooltip::Tooltip;
use std::time::Duration;
use textual_css::Styles;
use textual_render::Text;

/// Delay before a tooltip appears (Textual: `App.TOOLTIP_DELAY`).
pub const TOOLTIP_DELAY: Duration = Duration::from_millis(500);

/// Maximum tooltip width including padding (Textual's `max-width: 40`).
const MAX_WIDTH: i32 = 40;
const PAD_X: i32 = 2;
const PAD_Y: i32 = 1;

#[derive(Debug, Default)]
pub(crate) struct TooltipState {
    timer: Option<TimerId>,
    /// The widget under the mouse the tooltip (timer) is for (Textual: `_tooltip_widget`).
    widget: Option<NodeId>,
    /// The tooltip node currently displayed.
    shown: Option<NodeId>,
}

fn displayed(state: &AppState) -> bool {
    state
        .ux
        .tooltip
        .shown
        .is_some_and(|t| state.dom.contains(t))
}

fn cancel_timer(state: &mut AppState) {
    if let Some(t) = state.ux.tooltip.timer.take() {
        state.cancel_timer(t);
    }
}

fn hide(state: &mut AppState) {
    if let Some(t) = state.ux.tooltip.shown.take() {
        if let Some(n) = state.dom.get_mut(t) {
            n.inline_styles.display = Some(textual_css::Display::None);
            state.dirty = true;
        }
    }
}

/// Hide any tooltip and cancel a pending one (Textual: `Screen._clear_tooltip`).
pub(crate) fn clear(state: &mut AppState) {
    cancel_timer(state);
    hide(state);
}

/// The tooltip text for `widget`: its own, or the nearest ancestor's.
fn tooltip_for(state: &AppState, widget: NodeId) -> Option<String> {
    state
        .dom
        .ancestors_with_self(widget)
        .into_iter()
        .find_map(|id| state.dom.get(id).and_then(|n| n.tooltip.clone()))
}

/// Called for every mouse event (before dispatch) with the widget under the pointer.
pub(crate) fn on_mouse(state: &mut AppState, kind: MouseKind, target: Option<NodeId>) {
    match kind {
        MouseKind::Move => {}
        MouseKind::Down | MouseKind::Up => {
            clear(state);
            return;
        }
        _ => return,
    }
    let Some(widget) = target else {
        clear(state);
        return;
    };
    if state.ux.tooltip.widget != Some(widget) || !displayed(state) {
        if state.ux.tooltip.widget != Some(widget) {
            hide(state);
        }
        state.ux.tooltip.widget = Some(widget);
        cancel_timer(state);
        if tooltip_for(state, widget).is_some() {
            let id = state.add_timer(
                widget,
                TOOLTIP_DELAY,
                false,
                Box::new(|_w, ctx| {
                    let node = ctx.node;
                    ctx.app.ux.tooltip.timer = None;
                    show(ctx.app, node);
                }),
            );
            state.ux.tooltip.timer = Some(id);
        }
    } else {
        hide(state);
    }
}

/// Show the tooltip for `widget` at the mouse position (Textual: `_handle_tooltip_timer`).
fn show(state: &mut AppState, widget: NodeId) {
    let Some(content) = tooltip_for(state, widget) else {
        hide(state);
        return;
    };
    let Some(screen) = state.dom.screen_of(widget) else {
        return;
    };
    if state.active_screen() != Some(screen) {
        return;
    }
    let Some(mouse) = state.mouse_pos else {
        return;
    };
    if state
        .ux
        .tooltip
        .shown
        .is_some_and(|t| state.dom.screen_of(t) != Some(screen))
    {
        hide(state);
    }

    let existing = state
        .dom
        .children(screen)
        .iter()
        .copied()
        .find(|c| state.dom.widget::<Tooltip>(*c).is_some());
    let tooltip = match existing {
        Some(t) => {
            // Keep it the last child so it paints above everything else.
            state.dom.append_child(screen, t);
            t
        }
        None => state.mount(
            screen,
            Child::new(Box::new(Tooltip::new())).id("textual-tooltip"),
            None,
        ),
    };

    // Size: wrap to at most MAX_WIDTH (border box), like `width: auto; max-width: 40`.
    let text = Text::from_markup(&content);
    let content_width = text.cell_len().clamp(1, MAX_WIDTH - 2 * PAD_X);
    let lines = text.height_for_width(content_width).max(1);
    let width = content_width + 2 * PAD_X;
    let height = lines + 2 * PAD_Y;

    // Position: centered under the pointer, one row below; above if it doesn't fit.
    let size = state.size;
    let origin = state
        .dom
        .get(screen)
        .map(|n| n.layout.content_region.offset())
        .unwrap_or_default();
    let x = (mouse.x - width / 2).clamp(0, (size.width - width).max(0));
    let mut y = mouse.y + 1;
    if y + height > size.height {
        y = mouse.y - 1 - height;
    }
    let y = y.clamp(0, (size.height - height).max(0));

    if let Some(w) = state.dom.widget_mut::<Tooltip>(tooltip) {
        w.update(content);
    }
    let css = format!(
        "display: block; width: {width}; height: {height}; offset: {} {};",
        x - origin.x,
        y - origin.y
    );
    if let (Ok(parsed), Some(n)) = (Styles::parse_inline(&css), state.dom.get_mut(tooltip)) {
        n.inline_styles.merge(&parsed);
    }
    state.ux.tooltip.shown = Some(tooltip);
    state.dirty = true;
}

impl Ctx<'_> {
    /// Set (or clear) this widget's tooltip (Textual: `widget.tooltip = ...`).
    pub fn set_tooltip(&mut self, tooltip: Option<&str>) {
        let id = self.node;
        self.set_tooltip_of(id, tooltip);
    }

    /// Set (or clear) the tooltip of another node.
    pub fn set_tooltip_of(&mut self, id: NodeId, tooltip: Option<&str>) {
        if let Some(n) = self.app.dom.get_mut(id) {
            n.tooltip = tooltip.map(String::from);
        }
        // Update a tooltip that is currently displayed for this widget (`Screen._update_tooltip`).
        let state = &mut *self.app;
        if displayed(state) {
            if let Some(w) = state.ux.tooltip.widget {
                if w == id || state.dom.is_ancestor(id, w) {
                    show(state, w);
                }
            }
        }
    }

    /// This widget's tooltip.
    pub fn tooltip(&self) -> Option<&str> {
        self.app
            .dom
            .get(self.node)
            .and_then(|n| n.tooltip.as_deref())
    }
}

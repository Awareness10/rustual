//! Runtime glue for the command palette, tooltips, the key / help panels and `@click` markup links.
//!
//! Kept out of `app.rs`: the runtime only calls the hooks here (`builtin_action`,
//! `click_link`) and in [`crate::tooltip`].

use crate::app::AppState;
use crate::binding::Action;
use crate::command::CommandProvider;
use crate::context::Ctx;
use crate::dom::NodeId;
use crate::events::{Event, MouseEvent};
use crate::layout::Arrangement;
use crate::widget::Child;
use std::rc::Rc;

/// Per-app state for the features in this module.
pub(crate) struct UxState {
    pub tooltip: crate::tooltip::TooltipState,
    pub providers: Vec<Rc<dyn CommandProvider>>,
    pub search_hooks: Vec<crate::command::SearchHook>,
    pub palette_enabled: bool,
    pub system_commands: bool,
    /// Hits delivered while their palette was busy (e.g. from inside its own search hook):
    /// `(palette, generation, hits)`, picked up by the palette once its hooks return.
    pub pending_hits: Vec<(crate::dom::NodeId, u64, Vec<crate::command::Command>)>,
}

impl Default for UxState {
    fn default() -> Self {
        UxState {
            tooltip: Default::default(),
            providers: Vec::new(),
            search_hooks: Vec::new(),
            palette_enabled: true,
            system_commands: true,
            pending_hits: Vec::new(),
        }
    }
}

/// Builtin app actions provided by this module. Returns false for unknown actions.
pub(crate) fn builtin_action(state: &mut AppState, action: &Action) -> bool {
    match action.name.as_str() {
        "command_palette" => crate::command::open_palette(state),
        "change_theme" => crate::command::open_theme_palette(state),
        "show_keys" => show_key_panel(state),
        "hide_keys" => hide_key_panel(state),
        "show_help_panel" => show_help_panel(state),
        "hide_help_panel" => hide_help_panel(state),
        "toggle_keys" => {
            if key_panels(state).is_empty() {
                show_key_panel(state)
            } else {
                hide_key_panel(state)
            }
        }
        _ => return false,
    }
    state.dirty = true;
    true
}

// ---------------------------------------------------------------------------------- key panel

fn panels(state: &AppState, selector: &str) -> Vec<NodeId> {
    let Some(screen) = state.active_screen() else {
        return Vec::new();
    };
    state
        .dom
        .query_str(screen, selector)
        .into_iter()
        .filter(|id| !state.pending_removals.contains(id))
        .collect()
}

/// Stand-alone key panels (not the one embedded in a help panel).
fn key_panels(state: &AppState) -> Vec<NodeId> {
    let mut out = panels(state, "KeyPanel");
    out.retain(|id| {
        !state
            .dom
            .ancestors(*id)
            .iter()
            .any(|a| state.dom.get(*a).is_some_and(|n| n.is_type("HelpPanel")))
    });
    out
}

/// Mount a [`HelpPanel`](crate::widgets::HelpPanel) on the active screen (Textual: `action_show_help_panel`).
pub(crate) fn show_help_panel(state: &mut AppState) {
    if !panels(state, "HelpPanel").is_empty() {
        return;
    }
    if let Some(screen) = state.active_screen() {
        let panel = crate::widgets::help_panel::HelpPanel::new();
        state.mount(screen, Child::new(Box::new(panel)), None);
    }
}

/// Remove help panels from the active screen (Textual: `action_hide_help_panel`).
pub(crate) fn hide_help_panel(state: &mut AppState) {
    for id in panels(state, "HelpPanel") {
        state.request_remove(id);
    }
}

/// Mount a stand-alone [`KeyPanel`](crate::widgets::KeyPanel) on the active screen.
pub(crate) fn show_key_panel(state: &mut AppState) {
    if !key_panels(state).is_empty() {
        return;
    }
    if let Some(screen) = state.active_screen() {
        let panel = crate::widgets::key_panel::KeyPanel::new();
        state.mount(screen, Child::new(Box::new(panel)), None);
    }
}

/// Remove stand-alone key panels from the active screen.
pub(crate) fn hide_key_panel(state: &mut AppState) {
    for id in key_panels(state) {
        state.request_remove(id);
    }
}

impl Ctx<'_> {
    /// Show the keys panel on the active screen.
    pub fn show_key_panel(&mut self) {
        show_key_panel(self.app);
    }

    /// Hide the keys panel.
    pub fn hide_key_panel(&mut self) {
        hide_key_panel(self.app);
    }

    /// Show the keys panel if hidden, hide it if shown.
    pub fn toggle_key_panel(&mut self) {
        if key_panels(self.app).is_empty() {
            show_key_panel(self.app)
        } else {
            hide_key_panel(self.app)
        }
    }

    /// Whether the active screen shows a keys panel.
    pub fn key_panel_visible(&self) -> bool {
        !key_panels(self.app).is_empty()
    }

    /// Show the help panel (focused widget's help + key bindings) on the active screen.
    pub fn show_help_panel(&mut self) {
        show_help_panel(self.app);
    }

    /// Hide the help panel.
    pub fn hide_help_panel(&mut self) {
        hide_help_panel(self.app);
    }

    /// Whether the active screen shows a help panel.
    pub fn help_panel_visible(&self) -> bool {
        !panels(self.app, "HelpPanel").is_empty()
    }
}

// ---------------------------------------------------------------------------------- @click links

/// The link (`[@click=...]` action) of the cell at screen `(x, y)` in the current frame.
pub(crate) fn link_at(state: &AppState, x: i32, y: i32) -> Option<String> {
    if let Some(frame) = state.last_frame.as_ref().filter(|f| f.size() == state.size) {
        return frame.get(x, y)?.link.as_deref().map(String::from);
    }
    // Headless (tests) or before the first frame: paint the current layout.
    let layers: Vec<(NodeId, &Arrangement)> = state
        .screens
        .iter()
        .filter_map(|s| state.arrangements.get(s).map(|a| (*s, a)))
        .collect();
    if layers.is_empty() {
        return None;
    }
    let canvas = crate::compositor::render(&state.dom, &state.env, &layers, state.size);
    canvas.get(x, y)?.link.as_deref().map(String::from)
}

/// Handle a click on `target` at a cell carrying a `@click` link: the widget gets the
/// `Click` (without bubbling) and the action runs with the widget as its owner (Textual:
/// `App._broker_event`, which stops the event). Returns false if there is no link.
pub(crate) fn click_link(state: &mut AppState, target: NodeId, m: MouseEvent) -> bool {
    if state.dom.is_disabled(target) {
        return false;
    }
    let Some(link) = link_at(state, m.screen_x, m.screen_y) else {
        return false;
    };
    let region = state.region_of(target);
    let mut ev = m;
    ev.x = m.screen_x - region.x;
    ev.y = m.screen_y - region.y;
    state.call_widget(target, |w, ctx| w.on_event(&Event::Click(ev), ctx));
    state.run_action(target, &link);
    true
}

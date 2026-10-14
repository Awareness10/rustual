//! `RadioButton` (port of `textual/widgets/_radio_button.py`): a [`ToggleButton`] showing
//! `▐●▌ label`. Best used inside a [`super::RadioSet`].

use super::toggle_button::{toggle_widget, ToggleButton, ToggleKind};

/// Posted when the value of a radio button changes (Textual: `RadioButton.Changed`).
/// Inside a `RadioSet` these are consumed by the set, which posts `radio_set::Changed` instead.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Changed {
    pub value: bool,
    pub radio_button_id: Option<String>,
}

/// A radio button representing a boolean value.
///
/// State is available through `Deref` to [`ToggleButton`].
#[derive(Debug, Clone)]
pub struct RadioButton(ToggleButton);

impl RadioButton {
    /// A radio button with a (markup) label.
    pub fn new(label: impl Into<String>) -> Self {
        RadioButton(ToggleButton::with_kind(
            label.into(),
            "\u{25cf}",
            ToggleKind::Radio,
        ))
    }

    /// Initial value (no `Changed` message is posted for it).
    pub fn value(self, value: bool) -> Self {
        RadioButton(self.0.value(value))
    }

    /// Put the button before (default) or after the label.
    pub fn button_first(self, first: bool) -> Self {
        RadioButton(self.0.button_first(first))
    }

    /// Compact style (no border or padding).
    pub fn compact(self, compact: bool) -> Self {
        RadioButton(self.0.compact(compact))
    }

    /// Used by `RadioSet`, which takes over focus handling for its buttons.
    pub(crate) fn set_focusable(&mut self, focusable: bool) {
        self.0.focusable = focusable;
    }
}

toggle_widget!(RadioButton, ["RadioButton", "ToggleButton", "Static"]);

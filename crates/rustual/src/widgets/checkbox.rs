//! `Checkbox` (port of `textual/widgets/_checkbox.py`): a [`ToggleButton`] showing `▐X▌ label`.

use super::toggle_button::{toggle_widget, ToggleButton, ToggleKind};

/// Posted when the value of a checkbox changes (Textual: `Checkbox.Changed`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Changed {
    /// The new value.
    pub value: bool,
    /// The CSS id of the checkbox, if any.
    pub checkbox_id: Option<String>,
}

/// A check box representing a boolean value. Toggle with `enter`, `space` or a click.
///
/// State is available through `Deref` to [`ToggleButton`]: `checkbox.get_value()`,
/// `checkbox.toggle(ctx)`, `checkbox.set_value(true, ctx)`.
#[derive(Debug, Clone)]
pub struct Checkbox(ToggleButton);

impl Checkbox {
    /// A checkbox with a (markup) label.
    pub fn new(label: impl Into<String>) -> Self {
        Checkbox(ToggleButton::with_kind(
            label.into(),
            "X",
            ToggleKind::Checkbox,
        ))
    }

    /// Initial value (no `Changed` message is posted for it).
    pub fn value(self, value: bool) -> Self {
        Checkbox(self.0.value(value))
    }

    /// Put the button before (default) or after the label.
    pub fn button_first(self, first: bool) -> Self {
        Checkbox(self.0.button_first(first))
    }

    /// Compact style (no border or padding).
    pub fn compact(self, compact: bool) -> Self {
        Checkbox(self.0.compact(compact))
    }
}

toggle_widget!(Checkbox, ["Checkbox", "ToggleButton", "Static"]);

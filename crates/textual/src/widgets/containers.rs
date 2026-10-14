//! Container widgets (port of `textual/containers.py`). Children are given with
//! `.with([...])`: `Horizontal::new().with(compose![Button::new("A"), Button::new("B")])`.

use crate::binding::Binding;
use crate::widget::Widget;

macro_rules! container {
    ($(#[$doc:meta])* $name:ident, [$($type_name:literal),+], $css:literal) => {
        $(#[$doc])*
        #[derive(Debug, Default, Clone)]
        pub struct $name;

        impl $name {
            pub fn new() -> Self {
                $name
            }
        }

        impl Widget for $name {
            fn type_names(&self) -> &'static [&'static str] {
                &[$($type_name),+]
            }

            fn default_css(&self) -> &'static str {
                $css
            }
        }
    };
}

macro_rules! scroll_container {
    ($(#[$doc:meta])* $name:ident, [$($type_name:literal),+], $css:literal) => {
        $(#[$doc])*
        #[derive(Debug, Default, Clone)]
        pub struct $name;

        impl $name {
            pub fn new() -> Self {
                $name
            }
        }

        impl Widget for $name {
            fn type_names(&self) -> &'static [&'static str] {
                &[$($type_name),+]
            }

            fn default_css(&self) -> &'static str {
                $css
            }

            fn can_focus(&self) -> bool {
                true
            }

            fn bindings(&self) -> Vec<Binding> {
                scroll_bindings()
            }
        }
    };
}

/// Keyboard bindings shared by scrollable containers (handled by the builtin scroll actions).
pub fn scroll_bindings() -> Vec<Binding> {
    vec![
        Binding::new("up", "scroll_up", "Scroll Up").hidden(),
        Binding::new("down", "scroll_down", "Scroll Down").hidden(),
        Binding::new("left", "scroll_left", "Scroll Left").hidden(),
        Binding::new("right", "scroll_right", "Scroll Right").hidden(),
        Binding::new("home", "scroll_home", "Scroll Home").hidden(),
        Binding::new("end", "scroll_end", "Scroll End").hidden(),
        Binding::new("pageup", "page_up", "Page Up").hidden(),
        Binding::new("pagedown", "page_down", "Page Down").hidden(),
    ]
}

container!(
    /// Expanding container with vertical layout, no scrollbars.
    Container, ["Container"],
    "Container { width: 1fr; height: 1fr; layout: vertical; overflow: hidden hidden; }"
);
container!(
    /// Expanding container with vertical layout, no scrollbars.
    Vertical, ["Vertical"],
    "Vertical { width: 1fr; height: 1fr; layout: vertical; overflow: hidden hidden; }"
);
container!(
    /// Non-expanding (auto height) vertical container.
    VerticalGroup, ["VerticalGroup"],
    "VerticalGroup { width: 1fr; height: auto; layout: vertical; overflow: hidden hidden; }"
);
container!(
    /// Expanding container with horizontal layout, no scrollbars.
    Horizontal, ["Horizontal"],
    "Horizontal { width: 1fr; height: 1fr; layout: horizontal; overflow: hidden hidden; }"
);
container!(
    /// Non-expanding (auto height) horizontal container.
    HorizontalGroup, ["HorizontalGroup"],
    "HorizontalGroup { width: 1fr; height: auto; layout: horizontal; overflow: hidden hidden; }"
);
container!(
    /// Centers children horizontally.
    Center, ["Center"],
    "Center { align-horizontal: center; width: 1fr; height: auto; }"
);
container!(
    /// Aligns children to the right.
    Right, ["Right"],
    "Right { align-horizontal: right; width: 1fr; height: auto; }"
);
container!(
    /// Centers children vertically.
    Middle, ["Middle"],
    "Middle { align-vertical: middle; width: auto; height: 1fr; }"
);
container!(
    /// Centers children on both axes.
    CenterMiddle, ["CenterMiddle"],
    "CenterMiddle { align: center middle; width: 1fr; height: 1fr; }"
);
container!(
    /// Grid layout container (`grid-size`, `grid-columns`, ...).
    Grid, ["Grid"],
    "Grid { width: 1fr; height: 1fr; layout: grid; }"
);
container!(
    /// Grid with auto height.
    ItemGrid, ["ItemGrid"],
    "ItemGrid { width: 1fr; height: auto; layout: grid; }"
);
scroll_container!(
    /// Focusable container with scrollbars on both axes as needed.
    ScrollableContainer, ["ScrollableContainer"],
    "ScrollableContainer { width: 1fr; height: 1fr; layout: vertical; overflow: auto auto; }"
);
scroll_container!(
    /// Vertical layout with a vertical scrollbar as needed.
    VerticalScroll, ["VerticalScroll", "ScrollableContainer"],
    "VerticalScroll { width: 1fr; height: 1fr; layout: vertical; overflow-x: hidden; overflow-y: auto; }"
);
scroll_container!(
    /// Horizontal layout with a horizontal scrollbar as needed.
    HorizontalScroll, ["HorizontalScroll", "ScrollableContainer"],
    "HorizontalScroll { width: 1fr; height: 1fr; layout: horizontal; overflow-y: hidden; overflow-x: auto; }"
);

//! Built-in widgets.

pub mod button;
mod containers;
mod static_;

pub use button::{Button, ButtonVariant};
pub use containers::*;
pub use static_::{Label, Static, StaticContent};

// Form controls: Input, ToggleButton/Checkbox, RadioButton/RadioSet, Switch.
pub mod checkbox;
pub mod input;
pub mod radio_button;
pub mod radio_set;
pub mod switch;
pub mod toggle_button;
pub use checkbox::Checkbox;
pub use input::{Input, InputType, Selection};
pub use radio_button::RadioButton;
pub use radio_set::RadioSet;
pub use switch::Switch;
pub use toggle_button::ToggleButton;

// Navigation & structure widgets.
pub mod content_switcher;
pub mod tabbed_content;
pub mod tabs;
pub use content_switcher::ContentSwitcher;
pub use tabbed_content::{TabPane, TabbedContent};
pub use tabs::{Tab, Tabs, Underline};
pub mod collapsible;
pub mod tree;
pub use collapsible::Collapsible;
pub use tree::{Tree, TreeNode, TreeNodeId};
pub mod select;
pub use select::Select;

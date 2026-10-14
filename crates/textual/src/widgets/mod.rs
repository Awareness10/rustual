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

// App chrome & display widgets.
pub mod digits;
pub mod footer;
pub mod header;
pub mod loading_indicator;
pub mod placeholder;
pub mod progress_bar;
pub mod rule;
pub mod sparkline;
pub mod toast;
pub use digits::Digits;
pub use footer::Footer;
pub use header::Header;
pub use loading_indicator::LoadingIndicator;
pub use placeholder::{Placeholder, PlaceholderVariant};
pub use progress_bar::ProgressBar;
pub use rule::{LineStyle, Rule, RuleOrientation};
pub use sparkline::Sparkline;
pub use toast::{Toast, ToastRack};

// Lists & data widgets.
pub mod data_table;
pub mod list_view;
pub mod log;
pub mod option_list;
pub mod rich_log;
pub use data_table::{CellKey, CellValue, ColumnKey, Coordinate, CursorType, DataTable, RowKey};
pub use list_view::{ListItem, ListView};
pub use log::Log;
pub use option_list::{OptionItem, OptionList};
pub use rich_log::RichLog;

// Extra widgets: SelectionList, MaskedInput, DirectoryTree, Pretty, Welcome.
pub mod selection_list;
pub use selection_list::SelectionList;
pub mod masked_input;
pub use masked_input::MaskedInput;
pub mod directory_tree;
pub use directory_tree::{DirEntry, DirectoryTree};
pub mod pretty;
pub mod welcome;
pub use pretty::Pretty;
pub use welcome::Welcome;

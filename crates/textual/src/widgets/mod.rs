//! Built-in widgets.

pub mod button;
mod containers;
mod static_;

pub use button::{Button, ButtonVariant};
pub use containers::*;
pub use static_::{Label, Static, StaticContent};

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

//! Built-in widgets.

pub mod button;
mod containers;
mod static_;

pub use button::{Button, ButtonVariant};
pub use containers::*;
pub use static_::{Label, Static, StaticContent};

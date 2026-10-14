//! # textual
//!
//! A Rust re-creation of Python's [Textual](https://textual.textualize.io) TUI framework:
//! a DOM of widgets styled with Textual CSS, laid out with vertical/horizontal/grid/dock
//! layouts, driven by an async (tokio) event loop.

pub mod app;
pub mod binding;
pub mod compositor;
pub mod context;
pub mod dom;
pub mod driver;
pub mod events;
pub mod layout;
pub mod message;
pub mod pilot;
pub mod screen;
pub mod widget;

pub use app::{App, AppHandle, Severity, TimerId, WorkerHandle, ScreenDismissed, Notified};
pub use binding::{Action, Binding};
pub use pilot::Pilot;
pub use screen::Screen;
pub use context::Ctx;
pub use dom::{Dom, Node, NodeId, NodeLayout};
pub use events::{Event, KeyEvent, MouseButton, MouseEvent};
pub use message::{Flow, Message};
pub use widget::{Child, Content, RenderCtx, RenderEnv, Widget, WidgetExt};

pub use textual_css as css;
pub use textual_render as render;
pub use textual_render::{Color, Offset, Region, Size, Spacing, Strip, Style, Text};

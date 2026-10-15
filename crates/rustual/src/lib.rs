//! # rustual
//!
//! A Rust re-creation of Python's [Textual](https://textual.textualize.io) TUI framework:
//! a DOM of widgets styled with Textual CSS, laid out with vertical/horizontal/grid/dock
//! layouts, driven by an async (tokio) event loop.

pub mod animation;
pub mod app;
pub mod binding;
pub mod command;
pub mod compositor;
pub mod context;
pub mod dom;
pub mod driver;
pub mod events;
pub mod layout;
pub mod message;
pub mod pilot;
pub mod screen;
pub mod screens;
pub mod scrollbar;
pub mod terminal;
pub mod tooltip;
pub mod transition;
mod ux;
pub mod widget;
pub mod widgets;
pub mod workers;

pub use animation::{AnimateOptions, AnimationId, AnimationLevel, Easing, OnComplete, StyleProp};
pub use app::{App, AppHandle, Notified, ScreenDismissed, Severity, TimerId, WorkerHandle};
pub use binding::{Action, Binding};
pub use command::{Command, CommandAction, CommandProvider, Matcher};
pub use context::Ctx;
pub use dom::{Dom, Node, NodeId, NodeLayout};
pub use events::{Event, KeyEvent, MouseButton, MouseEvent};
pub use message::{Flow, Message};
pub use pilot::Pilot;
pub use screen::Screen;
pub use screens::DEFAULT_MODE;
pub use widget::{Child, Content, RenderCtx, RenderEnv, Widget, WidgetExt};
pub use workers::{WorkerId, WorkerOptions, WorkerState};

pub use rustual_css as css;
pub use rustual_render as render;
pub use rustual_render::{Color, Offset, Region, Size, Spacing, Strip, Style, Text};

/// Everything needed to write an app: `use rustual::prelude::*;`
pub mod prelude {
    pub use crate::animation::{AnimateOptions, AnimationLevel, Easing, StyleProp};
    pub use crate::app::{
        App, AppHandle, Notified, ScreenDismissed, Severity, TimerId, WorkerHandle,
    };
    pub use crate::binding::Binding;
    pub use crate::command::{Command, CommandProvider};
    pub use crate::compose;
    pub use crate::context::Ctx;
    pub use crate::dom::NodeId;
    pub use crate::events::{Event, KeyEvent, MouseButton, MouseEvent};
    pub use crate::message::{Flow, Message};
    pub use crate::pilot::Pilot;
    pub use crate::widget::{Child, Content, RenderCtx, Widget, WidgetExt};
    pub use crate::widgets::*;
    pub use crate::workers::{self as worker, WorkerId, WorkerOptions, WorkerState};
    pub use rustual_render::{Color, Offset, Region, Size, Spacing, Strip, Style, Text};
}

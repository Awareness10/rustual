//! Rendering primitives for rustual.
//!
//! This crate plays the role that `rich` plays for Python Textual: colors, styles,
//! styled text with markup, line strips, and a 2D cell canvas with diffing.

pub mod border;
pub mod canvas;
pub mod cells;
pub mod color;
mod color_names;
pub mod export;
pub mod geometry;
pub mod markup;
pub mod strip;
pub mod style;
pub mod text;

pub use border::BorderKind;
pub use canvas::{Canvas, Cell, CellRun};
pub use color::{Color, ColorParseError, Gradient, GradientError};
pub use geometry::{Offset, Region, Size, Spacing};
pub use strip::Strip;
pub use style::{Attrs, Style, StyleParseError};
pub use text::{Justify, Span, Text, TextOverflow};

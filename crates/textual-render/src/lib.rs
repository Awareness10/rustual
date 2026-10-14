//! Rendering primitives for textual-rs.
//!
//! This crate plays the role that `rich` plays for Python Textual: colors, styles,
//! styled text with markup, line strips, and a 2D cell canvas with diffing.

pub mod border;
pub mod canvas;
pub mod cells;
pub mod color;
pub mod export;
mod color_names;
pub mod geometry;
pub mod markup;
pub mod strip;
pub mod style;
pub mod text;

pub use border::BorderKind;
pub use canvas::{Canvas, Cell, CellRun};
pub use color::{Color, ColorParseError};
pub use geometry::{Offset, Region, Size, Spacing};
pub use strip::Strip;
pub use style::{Attrs, Style, StyleParseError};
pub use text::{Justify, Span, Text, TextOverflow};

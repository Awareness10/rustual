//! Rendering primitives for rustual.
//!
//! This crate plays the role that `rich` plays for Python Textual: colors, styles,
//! styled text with markup, line strips, and a 2D cell canvas with diffing.
//!
//! ```
//! use rustual_render::{Color, Text};
//!
//! // Textual markup → styled text; widths are measured in terminal cells.
//! let text = Text::from_markup("[b red]Hello[/] 世界");
//! assert_eq!(text.plain_text(), "Hello 世界");
//! assert_eq!(text.cell_len(), 10); // CJK characters are two cells wide
//!
//! assert_eq!(Color::parse("#ff8800").unwrap(), Color::rgb(255, 136, 0));
//! ```

pub mod border;
pub mod canvas;
pub mod cells;
pub mod color;
mod color_names;
pub mod export;
pub mod geometry;
pub mod markup;
pub mod renderables;
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

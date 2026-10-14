//! Textual CSS (TCSS) for textual-rs: tokenizer, parser, selectors, typed styles,
//! cascade, and the theme/design system that produces `$variables`.

pub mod model;
pub mod parser;
pub mod scalar;
pub mod selector;
pub mod styles;
pub mod stylesheet;
pub mod theme;
pub mod tokenizer;

pub use model::{CssError, Declaration, Rule, RuleSet};
pub use scalar::{Dimension, Scalar, Unit};
pub use selector::{SelectorList, SelectorNode, Specificity};
pub use styles::*;
pub use stylesheet::{Origin, Stylesheet};
pub use theme::Theme;

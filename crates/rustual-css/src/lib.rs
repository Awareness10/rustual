//! Textual CSS (TCSS) for rustual: tokenizer, parser, selectors, typed styles,
//! cascade, and the theme/design system that produces `$variables`.

pub mod model;
pub mod parser;
mod properties;
pub mod scalar;
pub mod selector;
pub mod styles;
pub mod stylesheet;
pub mod theme;
pub mod tokenizer;
pub mod transition;

pub use model::{CssError, Declaration, Rule, RuleSet};
pub use properties::{
    is_known_property, parse_border_kind, parse_color_value, parse_text_style, IGNORED_PROPERTIES,
};
pub use scalar::{Dimension, Scalar, Unit};
pub use selector::{SelectorList, SelectorNode, Specificity};
pub use styles::*;
pub use stylesheet::{Origin, Stylesheet};
pub use theme::Theme;
pub use transition::{parse_duration, parse_transitions, Transition, EASING_NAMES};

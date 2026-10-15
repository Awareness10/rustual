//! Parsed stylesheet model.

use crate::selector::{SelectorList, Specificity};
use crate::styles::Styles;
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub struct CssError {
    pub message: String,
    pub line: usize,
    pub column: usize,
}

impl fmt::Display for CssError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}: {}", self.line, self.column, self.message)
    }
}

impl std::error::Error for CssError {}

/// A single `name: value [!important]` declaration (value has variables substituted).
#[derive(Debug, Clone, PartialEq)]
pub struct Declaration {
    pub name: String,
    pub value: String,
    pub important: bool,
}

/// A flattened rule: one selector list (nesting already expanded) and its declarations,
/// pre-parsed into `styles` (normal) and `important_styles`.
#[derive(Debug, Clone)]
pub struct Rule {
    pub selectors: SelectorList,
    pub declarations: Vec<Declaration>,
    pub styles: Styles,
    pub important_styles: Styles,
    /// Position in the combined stylesheet (later wins on equal specificity).
    pub source_order: usize,
}

/// The parsed result of one CSS source.
#[derive(Debug, Clone, Default)]
pub struct RuleSet {
    pub rules: Vec<Rule>,
}

pub type _Specificity = Specificity;

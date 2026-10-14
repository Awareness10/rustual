//! TCSS parser: variables, nested rules (`&`), selector lists, declarations.

use crate::model::{CssError, RuleSet};
use std::collections::HashMap;

/// Parse `css`. `variables` are theme variables (names without `$`); variables defined
/// in the source (`$name: value;`) add to / override them for the rest of the source.
/// Unknown properties or invalid values produce errors but parsing continues; errors are returned
/// alongside whatever rules parsed successfully.
pub fn parse_stylesheet(css: &str, variables: &HashMap<String, String>, source_order_start: usize) -> (RuleSet, Vec<CssError>) {
    todo!("{css}{variables:?}{source_order_start}")
}

//! Stylesheet: collects sources, resolves the cascade for a node.

use crate::model::{CssError, Rule};
use crate::selector::SelectorNode;
use crate::styles::Styles;
use std::collections::HashMap;

/// Where CSS came from. Precedence for equal specificity & importance: Default < App.
/// (Textual: widget DEFAULT_CSS always loses to app CSS regardless of specificity, unless !important.)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Origin {
    /// Widget `default_css()`.
    Default,
    /// App CSS / CSS files.
    App,
}

#[derive(Debug, Clone, Default)]
pub struct Stylesheet {
    /// Raw sources kept so the sheet can be re-parsed when variables change.
    sources: Vec<(String, Origin, String)>,
    variables: HashMap<String, String>,
    rules: Vec<(Origin, Rule)>,
}

impl Stylesheet {
    pub fn new() -> Self {
        Self::default()
    }

    /// Replace theme variables (names without `$`) and re-parse all sources.
    pub fn set_variables(&mut self, vars: HashMap<String, String>) -> Vec<CssError> {
        todo!("{vars:?}")
    }

    pub fn variables(&self) -> &HashMap<String, String> {
        &self.variables
    }

    /// Add a CSS source. `key` identifies it (e.g. widget type name or file path); adding the same
    /// key again replaces the old source. Returns parse errors (rules that parsed are kept).
    pub fn add_source(&mut self, key: &str, css: &str, origin: Origin) -> Vec<CssError> {
        todo!("{key}{css}{origin:?}")
    }

    pub fn has_source(&self, key: &str) -> bool {
        self.sources.iter().any(|(k, _, _)| k == key)
    }

    /// Cascade for one node (no inheritance; caller applies `inherit_from`).
    /// Order (lowest → highest): Default normal, App normal, Default !important, App !important;
    /// within each tier by specificity then source order.
    pub fn compute<N: SelectorNode>(&self, node: N) -> Styles {
        todo!("{:?}", node.css_id())
    }
}

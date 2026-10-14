//! Selectors, specificity and matching against a DOM through [`SelectorNode`].

/// Read-only view of a DOM node for selector matching. Implemented by the framework
/// (e.g. a `(dom, node_id)` handle). Cheap to copy.
pub trait SelectorNode: Copy {
    /// CSS type names, most specific first, e.g. `["Button", "Static", "Widget"]`.
    fn type_names(&self) -> &[&'static str];
    fn css_id(&self) -> Option<&str>;
    fn has_class(&self, class: &str) -> bool;
    /// `hover`, `focus`, `focus-within`, `disabled`, `enabled`, `dark`, `light`,
    /// `first-child`, `last-child`, `even`, `odd`, `empty`, ... (without the colon).
    fn has_pseudo_class(&self, pseudo: &str) -> bool;
    fn parent(&self) -> Option<Self>;
}

/// (ids, classes + pseudo-classes, types) — compared lexicographically.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Specificity(pub u32, pub u32, pub u32);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Combinator {
    /// Whitespace: any ancestor.
    Descendant,
    /// `>`: direct parent.
    Child,
}

/// A compound selector like `Button.primary#ok:hover` (or `*`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Compound {
    /// `None` = universal / unspecified.
    pub type_name: Option<String>,
    pub id: Option<String>,
    pub classes: Vec<String>,
    pub pseudo_classes: Vec<String>,
}

/// A complex selector: compounds joined by combinators, left to right.
/// `parts[0]` has no combinator (Descendant is used as placeholder).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selector {
    pub parts: Vec<(Combinator, Compound)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectorList {
    pub selectors: Vec<Selector>,
}

impl Selector {
    pub fn specificity(&self) -> Specificity {
        todo!()
    }

    pub fn matches<N: SelectorNode>(&self, node: N) -> bool {
        todo!("{:?}", node.css_id())
    }
}

impl SelectorList {
    /// Parse a selector list such as `"Button.primary:hover, #sidebar > Label"`.
    pub fn parse(s: &str) -> Result<SelectorList, String> {
        todo!("{s}")
    }

    /// Highest specificity among matching selectors, or `None` if no selector matches.
    pub fn match_specificity<N: SelectorNode>(&self, node: N) -> Option<Specificity> {
        todo!("{:?}", node.css_id())
    }

    pub fn matches<N: SelectorNode>(&self, node: N) -> bool {
        self.match_specificity(node).is_some()
    }
}

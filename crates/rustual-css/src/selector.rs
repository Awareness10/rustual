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

impl Compound {
    fn is_empty(&self) -> bool {
        *self == Compound::default()
    }

    fn specificity(&self) -> Specificity {
        Specificity(
            self.id.is_some() as u32,
            (self.classes.len() + self.pseudo_classes.len()) as u32,
            self.type_name.is_some() as u32,
        )
    }

    /// Does this compound match `node` (ignoring combinators)?
    pub fn matches<N: SelectorNode>(&self, node: N) -> bool {
        if let Some(t) = &self.type_name {
            if !node.type_names().iter().any(|n| n == t) {
                return false;
            }
        }
        if let Some(id) = &self.id {
            if node.css_id() != Some(id.as_str()) {
                return false;
            }
        }
        self.classes.iter().all(|c| node.has_class(c))
            && self.pseudo_classes.iter().all(|p| node.has_pseudo_class(p))
    }
}

impl Selector {
    pub fn specificity(&self) -> Specificity {
        self.parts
            .iter()
            .fold(Specificity::default(), |acc, (_, c)| {
                let s = c.specificity();
                Specificity(acc.0 + s.0, acc.1 + s.1, acc.2 + s.2)
            })
    }

    pub fn matches<N: SelectorNode>(&self, node: N) -> bool {
        if self.parts.is_empty() {
            return false;
        }
        self.match_at(self.parts.len() - 1, node)
    }

    /// Match `parts[..=index]` with `parts[index]` against `node`, walking parents right-to-left.
    fn match_at<N: SelectorNode>(&self, index: usize, node: N) -> bool {
        let (combinator, compound) = &self.parts[index];
        if !compound.matches(node) {
            return false;
        }
        if index == 0 {
            return true;
        }
        match combinator {
            Combinator::Child => node.parent().is_some_and(|p| self.match_at(index - 1, p)),
            Combinator::Descendant => {
                let mut current = node.parent();
                while let Some(p) = current {
                    if self.match_at(index - 1, p) {
                        return true;
                    }
                    current = p.parent();
                }
                false
            }
        }
    }

    /// Parse a single complex selector (no commas).
    pub fn parse(s: &str) -> Result<Selector, String> {
        let chars: Vec<char> = s.chars().collect();
        let mut parts: Vec<(Combinator, Compound)> = Vec::new();
        let mut current = Compound::default();
        let mut pending = Combinator::Descendant;
        let mut i = 0;
        let ident = |i: &mut usize| -> Result<String, String> {
            let start = *i;
            while *i < chars.len() && is_ident_char(chars[*i]) {
                *i += 1;
            }
            if *i == start {
                Err(format!("expected identifier in selector {s:?}"))
            } else {
                Ok(chars[start..*i].iter().collect())
            }
        };
        // Universal `*` sets nothing, so track it to allow a lone `*`.
        let mut saw_universal = false;
        while i < chars.len() {
            let c = chars[i];
            match c {
                c if c.is_whitespace() => {
                    if !current.is_empty() || saw_universal {
                        parts.push((pending.clone(), std::mem::take(&mut current)));
                        pending = Combinator::Descendant;
                        saw_universal = false;
                    }
                    i += 1;
                }
                '>' => {
                    if !current.is_empty() || saw_universal {
                        parts.push((pending.clone(), std::mem::take(&mut current)));
                        saw_universal = false;
                    }
                    if parts.is_empty() {
                        return Err(format!("unexpected '>' in selector {s:?}"));
                    }
                    pending = Combinator::Child;
                    i += 1;
                }
                '*' => {
                    if !current.is_empty() || saw_universal {
                        return Err(format!("unexpected '*' in selector {s:?}"));
                    }
                    saw_universal = true;
                    i += 1;
                }
                '.' => {
                    i += 1;
                    current.classes.push(ident(&mut i)?);
                }
                '#' => {
                    i += 1;
                    let id = ident(&mut i)?;
                    if current.id.is_some() {
                        return Err(format!("multiple ids in selector {s:?}"));
                    }
                    current.id = Some(id);
                }
                ':' => {
                    i += 1;
                    current.pseudo_classes.push(ident(&mut i)?);
                }
                c if is_ident_start(c) => {
                    if current.type_name.is_some()
                        || !current.classes.is_empty()
                        || current.id.is_some()
                        || !current.pseudo_classes.is_empty()
                        || saw_universal
                    {
                        return Err(format!("unexpected type name in selector {s:?}"));
                    }
                    current.type_name = Some(ident(&mut i)?);
                }
                other => return Err(format!("unexpected {other:?} in selector {s:?}")),
            }
        }
        if !current.is_empty() || saw_universal {
            parts.push((pending.clone(), current));
            pending = Combinator::Descendant;
        }
        if pending == Combinator::Child {
            return Err(format!("selector {s:?} ends with '>'"));
        }
        if parts.is_empty() {
            return Err("empty selector".into());
        }
        parts[0].0 = Combinator::Descendant;
        Ok(Selector { parts })
    }
}

fn is_ident_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_' || c == '-'
}

fn is_ident_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '-'
}

impl SelectorList {
    /// Parse a selector list such as `"Button.primary:hover, #sidebar > Label"`.
    pub fn parse(s: &str) -> Result<SelectorList, String> {
        let selectors = s
            .split(',')
            .map(|part| Selector::parse(part.trim()))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(SelectorList { selectors })
    }

    /// Highest specificity among matching selectors, or `None` if no selector matches.
    pub fn match_specificity<N: SelectorNode>(&self, node: N) -> Option<Specificity> {
        self.selectors
            .iter()
            .filter(|s| s.matches(node))
            .map(Selector::specificity)
            .max()
    }

    pub fn matches<N: SelectorNode>(&self, node: N) -> bool {
        self.match_specificity(node).is_some()
    }
}

/// Combine a nested rule's selector with its parent's (one complex selector each).
/// `&` is replaced by the parent selector (`&:hover` → `Button:hover`); without `&` the nested
/// selector is a descendant (`Label` → `Button Label`). Mirrors `parse_rule_set` in
/// `textual/css/parse.py`.
pub(crate) fn combine_nested(parent: &str, child: &str) -> String {
    if child.contains('&') {
        child.replace('&', parent)
    } else {
        format!("{parent} {child}")
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A tiny mock DOM for matching tests.
    pub struct MockNode {
        pub types: Vec<&'static str>,
        pub id: Option<String>,
        pub classes: Vec<String>,
        pub pseudo: Vec<String>,
        pub parent: Option<usize>,
    }

    pub struct MockDom {
        pub nodes: Vec<MockNode>,
    }

    impl MockDom {
        pub fn new() -> Self {
            MockDom { nodes: vec![] }
        }
        /// `spec` like `"Button#ok.a.b:hover"`; types are given explicitly.
        pub fn add(
            &mut self,
            parent: Option<usize>,
            types: &[&'static str],
            id: Option<&str>,
            classes: &[&str],
            pseudo: &[&str],
        ) -> usize {
            self.nodes.push(MockNode {
                types: types.to_vec(),
                id: id.map(str::to_string),
                classes: classes.iter().map(|s| s.to_string()).collect(),
                pseudo: pseudo.iter().map(|s| s.to_string()).collect(),
                parent,
            });
            self.nodes.len() - 1
        }
        pub fn node(&self, i: usize) -> Ref<'_> {
            Ref {
                dom: self,
                index: i,
            }
        }
    }

    #[derive(Clone, Copy)]
    pub struct Ref<'a> {
        dom: &'a MockDom,
        index: usize,
    }

    impl<'a> SelectorNode for Ref<'a> {
        fn type_names(&self) -> &[&'static str] {
            &self.dom.nodes[self.index].types
        }
        fn css_id(&self) -> Option<&str> {
            self.dom.nodes[self.index].id.as_deref()
        }
        fn has_class(&self, class: &str) -> bool {
            self.dom.nodes[self.index]
                .classes
                .iter()
                .any(|c| c == class)
        }
        fn has_pseudo_class(&self, pseudo: &str) -> bool {
            self.dom.nodes[self.index]
                .pseudo
                .iter()
                .any(|c| c == pseudo)
        }
        fn parent(&self) -> Option<Self> {
            self.dom.nodes[self.index].parent.map(|index| Ref {
                dom: self.dom,
                index,
            })
        }
    }

    fn spec(s: &str) -> Specificity {
        SelectorList::parse(s).unwrap().selectors[0].specificity()
    }

    #[test]
    fn parse_compound() {
        let sel = Selector::parse("Button.primary#ok:hover:focus-within").unwrap();
        assert_eq!(sel.parts.len(), 1);
        let c = &sel.parts[0].1;
        assert_eq!(c.type_name.as_deref(), Some("Button"));
        assert_eq!(c.id.as_deref(), Some("ok"));
        assert_eq!(c.classes, vec!["primary"]);
        assert_eq!(c.pseudo_classes, vec!["hover", "focus-within"]);
    }

    #[test]
    fn parse_combinators() {
        let sel = Selector::parse("Screen  #side>Label .x").unwrap();
        let combs: Vec<_> = sel.parts.iter().map(|p| p.0.clone()).collect();
        assert_eq!(
            combs,
            vec![
                Combinator::Descendant,
                Combinator::Descendant,
                Combinator::Child,
                Combinator::Descendant
            ]
        );
        let sel = Selector::parse("A > B").unwrap();
        assert_eq!(sel.parts[1].0, Combinator::Child);
        let sel = Selector::parse("*").unwrap();
        assert_eq!(sel.parts.len(), 1);
        assert_eq!(sel.parts[0].1, Compound::default());
        let sel = Selector::parse("* > *:hover").unwrap();
        assert_eq!(sel.parts.len(), 2);
    }

    #[test]
    fn parse_list_and_errors() {
        let list = SelectorList::parse("A, .b , #c").unwrap();
        assert_eq!(list.selectors.len(), 3);
        assert!(SelectorList::parse("").is_err());
        assert!(SelectorList::parse("A,").is_err());
        assert!(SelectorList::parse("A >").is_err());
        assert!(SelectorList::parse("> A").is_err());
        assert!(SelectorList::parse("A..b").is_err());
        assert!(SelectorList::parse("A{").is_err());
        assert!(SelectorList::parse("A & B").is_err());
        assert!(SelectorList::parse(".-hyphen-class.--x").is_ok());
    }

    #[test]
    fn specificity() {
        assert_eq!(spec("*"), Specificity(0, 0, 0));
        assert_eq!(spec("Button"), Specificity(0, 0, 1));
        assert_eq!(spec("Button.a:hover"), Specificity(0, 2, 1));
        assert_eq!(spec("#x Label > .y"), Specificity(1, 1, 1));
        assert!(spec("#x") > spec(".a.b.c Button"));
        assert!(spec(".a") > spec("A B C"));
    }

    #[test]
    fn matching() {
        let mut dom = MockDom::new();
        let screen = dom.add(None, &["Screen"], None, &[], &[]);
        let side = dom.add(
            Some(screen),
            &["Vertical", "Widget"],
            Some("side"),
            &["box"],
            &[],
        );
        let button = dom.add(
            Some(side),
            &["Button", "Static", "Widget"],
            Some("ok"),
            &["primary"],
            &["hover"],
        );
        let label = dom.add(Some(button), &["Label", "Static", "Widget"], None, &[], &[]);
        let m = |s: &str, n: usize| SelectorList::parse(s).unwrap().matches(dom.node(n));

        assert!(m("Button", button));
        assert!(m("Static", button));
        assert!(m("Widget", button));
        assert!(!m("Label", button));
        assert!(m("*", button));
        assert!(m("#ok", button));
        assert!(!m("#nope", button));
        assert!(m("Button.primary:hover", button));
        assert!(!m("Button:focus", button));
        assert!(!m("Button.secondary", button));
        assert!(m("Screen Button", button));
        assert!(m("Screen Label", label));
        assert!(m("#side > Button", button));
        assert!(!m("Screen > Button", button));
        assert!(m("Screen > .box > Button > Label", label));
        assert!(m("Screen Static Label", label));
        assert!(!m("Label Screen", label));
        assert!(m("Label, Nope", label));
        assert!(m(".box Button:hover Label", label));
        // Backtracking: first Widget candidate (button) fails `> ...` but side works.
        assert!(m("Screen > Widget Label", label));
        assert!(!m("Vertical > Label", label));
    }

    #[test]
    fn match_specificity_picks_highest() {
        let mut dom = MockDom::new();
        let b = dom.add(None, &["Button"], Some("ok"), &["a"], &[]);
        let list = SelectorList::parse("Button, #ok, Nope.a.b.c").unwrap();
        assert_eq!(
            list.match_specificity(dom.node(b)),
            Some(Specificity(1, 0, 0))
        );
        let list = SelectorList::parse("Nope").unwrap();
        assert_eq!(list.match_specificity(dom.node(b)), None);
    }

    #[test]
    fn nesting_combination() {
        assert_eq!(combine_nested("Button", "&:hover"), "Button:hover");
        assert_eq!(combine_nested("Button", "&.primary"), "Button.primary");
        assert_eq!(combine_nested("Button", "Label"), "Button Label");
        assert_eq!(combine_nested("A B", "& > .c"), "A B > .c");
    }
}

//! Stylesheet: collects sources, resolves the cascade for a node.

use crate::model::{CssError, Rule};
use crate::parser::parse_stylesheet;
use crate::selector::{SelectorNode, Specificity};
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
    /// Error count per source from the last full re-parse (to attribute errors to a source).
    last_error_counts: Vec<(String, usize)>,
}

impl Stylesheet {
    pub fn new() -> Self {
        Self::default()
    }

    /// Replace theme variables (names without `$`) and re-parse all sources.
    pub fn set_variables(&mut self, vars: HashMap<String, String>) -> Vec<CssError> {
        self.variables = vars;
        self.reparse()
    }

    pub fn variables(&self) -> &HashMap<String, String> {
        &self.variables
    }

    /// Add a CSS source. `key` identifies it (e.g. widget type name or file path); adding the same
    /// key again replaces the old source. Returns parse errors (rules that parsed are kept).
    pub fn add_source(&mut self, key: &str, css: &str, origin: Origin) -> Vec<CssError> {
        if let Some(entry) = self.sources.iter_mut().find(|(k, _, _)| k == key) {
            entry.1 = origin;
            entry.2 = css.to_string();
            // Replacing a source changes the source order of everything after it.
            let errors = self.reparse();
            return self.errors_for(key, errors);
        }
        let start = self.rules.len();
        let (ruleset, errors) = parse_stylesheet(css, &self.variables, start);
        self.rules
            .extend(ruleset.rules.into_iter().map(|r| (origin, r)));
        self.sources
            .push((key.to_string(), origin, css.to_string()));
        errors
    }

    pub fn has_source(&self, key: &str) -> bool {
        self.sources.iter().any(|(k, _, _)| k == key)
    }

    /// Cascade for one node (no inheritance; caller applies `inherit_from`).
    /// Order (lowest → highest): Default normal, App normal, Default !important, App !important;
    /// within each tier by specificity then source order.
    pub fn compute<N: SelectorNode>(&self, node: N) -> Styles {
        // (tier, specificity, source_order, styles)
        let mut matched: Vec<(u8, Specificity, usize, &Styles)> = Vec::new();
        for (origin, rule) in &self.rules {
            let Some(spec) = rule.selectors.match_specificity(node) else {
                continue;
            };
            let base = match origin {
                Origin::Default => 0,
                Origin::App => 1,
            };
            if rule.declarations.iter().any(|d| !d.important) {
                matched.push((base, spec, rule.source_order, &rule.styles));
            }
            if rule.declarations.iter().any(|d| d.important) {
                matched.push((base + 2, spec, rule.source_order, &rule.important_styles));
            }
        }
        matched.sort_by_key(|(tier, spec, order, _)| (*tier, *spec, *order));
        let mut styles = Styles::new();
        for (_, _, _, s) in matched {
            styles.merge(s);
        }
        styles
    }

    /// Re-parse every source with the current variables. Returns all errors.
    fn reparse(&mut self) -> Vec<CssError> {
        self.rules.clear();
        let mut all_errors = Vec::new();
        let mut per_source = Vec::new();
        for (key, origin, css) in &self.sources {
            let (ruleset, errors) = parse_stylesheet(css, &self.variables, self.rules.len());
            self.rules
                .extend(ruleset.rules.into_iter().map(|r| (*origin, r)));
            per_source.push((key.clone(), errors.len()));
            all_errors.extend(errors);
        }
        self.last_error_counts = per_source;
        all_errors
    }

    /// From the errors of a full re-parse, the ones belonging to source `key`.
    fn errors_for(&self, key: &str, mut errors: Vec<CssError>) -> Vec<CssError> {
        let mut offset = 0;
        for (k, n) in &self.last_error_counts {
            if k == key {
                return errors.drain(offset..offset + n).collect();
            }
            offset += n;
        }
        Vec::new()
    }

    /// Number of parsed rules (all sources).
    pub fn rule_count(&self) -> usize {
        self.rules.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scalar::Scalar;
    use crate::selector::tests::MockDom;
    use crate::styles::ColorValue;
    use textual_render::Color;

    fn width(s: &Styles) -> Option<f32> {
        s.width.map(|w| w.value)
    }

    fn dom() -> (MockDom, usize) {
        let mut dom = MockDom::new();
        let screen = dom.add(None, &["Screen"], None, &[], &[]);
        let b = dom.add(
            Some(screen),
            &["Button", "Static", "Widget"],
            Some("ok"),
            &["primary"],
            &["hover"],
        );
        (dom, b)
    }

    #[test]
    fn specificity_then_source_order() {
        let (dom, b) = dom();
        let mut sheet = Stylesheet::new();
        let errors = sheet.add_source(
            "app",
            "#ok { width: 1; } Button { width: 2; height: 5; } .primary { width: 3; }",
            Origin::App,
        );
        assert!(errors.is_empty());
        let s = sheet.compute(dom.node(b));
        assert_eq!(width(&s), Some(1.0)); // id beats later class/type
        assert_eq!(s.height, Some(Scalar::cells(5.0)));

        let mut sheet = Stylesheet::new();
        sheet.add_source(
            "app",
            "Button:hover { width: 2; } .primary { width: 1; }",
            Origin::App,
        );
        // (0,1,1) beats the later (0,1,0).
        assert_eq!(width(&sheet.compute(dom.node(b))), Some(2.0));

        let mut sheet = Stylesheet::new();
        sheet.add_source(
            "app",
            "Button { width: 1; } Button { width: 2; }",
            Origin::App,
        );
        assert_eq!(width(&sheet.compute(dom.node(b))), Some(2.0));
    }

    #[test]
    fn app_css_beats_default_css_regardless_of_specificity() {
        let (dom, b) = dom();
        let mut sheet = Stylesheet::new();
        sheet.add_source("Button", "#ok.primary:hover { width: 1; }", Origin::Default);
        sheet.add_source("app", "Widget { width: 2; }", Origin::App);
        assert_eq!(width(&sheet.compute(dom.node(b))), Some(2.0));
        // Order of adding doesn't matter.
        let mut sheet = Stylesheet::new();
        sheet.add_source("app", "Widget { width: 2; }", Origin::App);
        sheet.add_source("Button", "#ok.primary:hover { width: 1; }", Origin::Default);
        assert_eq!(width(&sheet.compute(dom.node(b))), Some(2.0));
    }

    #[test]
    fn default_rules_ordered_by_specificity() {
        let (dom, b) = dom();
        let mut sheet = Stylesheet::new();
        sheet.add_source("Button", "#ok { width: 1; }", Origin::Default);
        sheet.add_source("Static", "Static { width: 2; }", Origin::Default);
        assert_eq!(width(&sheet.compute(dom.node(b))), Some(1.0));
    }

    #[test]
    fn important_tiers() {
        let (dom, b) = dom();
        let mut sheet = Stylesheet::new();
        sheet.add_source("Button", "Widget { width: 1 !important; }", Origin::Default);
        sheet.add_source("app", "#ok { width: 2; }", Origin::App);
        assert_eq!(width(&sheet.compute(dom.node(b))), Some(1.0));
        sheet.add_source("app2", "Widget { width: 3 !important; }", Origin::App);
        assert_eq!(width(&sheet.compute(dom.node(b))), Some(3.0));
        // Default !important loses to App !important even with higher specificity.
        sheet.add_source("Static", "#ok { width: 4 !important; }", Origin::Default);
        assert_eq!(width(&sheet.compute(dom.node(b))), Some(3.0));
        // Within the App important tier, specificity applies.
        sheet.add_source(
            "app3",
            "Screen Button { width: 6 !important; }",
            Origin::App,
        );
        sheet.add_source("app4", "Button { width: 5 !important; }", Origin::App);
        assert_eq!(width(&sheet.compute(dom.node(b))), Some(6.0));
    }

    #[test]
    fn non_matching_rules_ignored() {
        let (dom, b) = dom();
        let mut sheet = Stylesheet::new();
        sheet.add_source(
            "app",
            "Label { width: 1; } Button:focus { width: 2 }",
            Origin::App,
        );
        assert_eq!(sheet.compute(dom.node(b)), Styles::default());
    }

    #[test]
    fn same_key_replaces_source() {
        let (dom, b) = dom();
        let mut sheet = Stylesheet::new();
        sheet.add_source("k", "Button { width: 1; }", Origin::App);
        sheet.add_source("other", "Button { height: 1; }", Origin::App);
        assert!(sheet.has_source("k"));
        assert!(!sheet.has_source("nope"));
        sheet.add_source("k", "Button { width: 9; }", Origin::App);
        assert_eq!(sheet.rule_count(), 2);
        let s = sheet.compute(dom.node(b));
        assert_eq!(width(&s), Some(9.0));
        assert_eq!(s.height, Some(Scalar::cells(1.0)));
        // Errors from a replaced source are attributed to it.
        let errors = sheet.add_source("k", "Button { bogus: 1; }", Origin::App);
        assert_eq!(errors.len(), 1);
    }

    #[test]
    fn set_variables_reparses() {
        let (dom, b) = dom();
        let mut sheet = Stylesheet::new();
        let errors = sheet.add_source("app", "Button { background: $primary; }", Origin::App);
        assert_eq!(errors.len(), 1); // undefined variable
        let vars: HashMap<String, String> = [("primary".to_string(), "#ff0000".to_string())].into();
        assert!(sheet.set_variables(vars).is_empty());
        assert_eq!(sheet.variables()["primary"], "#ff0000");
        assert_eq!(
            sheet.compute(dom.node(b)).background,
            Some(Color::rgb(255, 0, 0))
        );
        let vars: HashMap<String, String> = [("primary".to_string(), "#00ff00".to_string())].into();
        sheet.set_variables(vars);
        assert_eq!(
            sheet.compute(dom.node(b)).background,
            Some(Color::rgb(0, 255, 0))
        );
    }

    #[test]
    fn rules_merge_per_property() {
        let (dom, b) = dom();
        let mut sheet = Stylesheet::new();
        sheet.add_source(
            "app",
            "Button { color: red; width: 1; } #ok { color: auto 50%; }",
            Origin::App,
        );
        let s = sheet.compute(dom.node(b));
        assert_eq!(s.color, Some(ColorValue::Auto(0.5)));
        assert_eq!(width(&s), Some(1.0));
    }

    #[test]
    fn real_button_css_cascade() {
        let mut dom = MockDom::new();
        let screen = dom.add(None, &["Screen"], None, &[], &[]);
        let b = dom.add(
            Some(screen),
            &["Button", "Widget"],
            None,
            &["-style-default", "-primary"],
            &["hover"],
        );
        let mut sheet = Stylesheet::new();
        sheet.set_variables(
            crate::Theme::by_name("textual-dark")
                .unwrap()
                .to_variables(),
        );
        let css = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/default_css/Button.tcss"
        ))
        .unwrap();
        assert!(sheet.add_source("Button", &css, Origin::Default).is_empty());
        let s = sheet.compute(dom.node(b));
        let vars = sheet.variables().clone();
        // `Button { &.-style-default { &.-primary { &:hover { background: $primary-darken-2 } } } }`
        assert_eq!(
            s.background,
            Some(Color::parse(&vars["primary-darken-2"]).unwrap())
        );
        assert_eq!(s.min_width, Some(Scalar::cells(16.0)));
        assert_eq!(s.text_style.as_ref().and_then(|t| t.bold), Some(true));
        // hover: `border-top: tall $primary`
        assert_eq!(
            s.border_top.map(|e| e.color),
            Some(Color::parse(&vars["primary"]).unwrap())
        );
    }
}

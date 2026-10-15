//! Compatibility tests against real Textual data:
//! - `fixtures/theme_variables.tsv`: output of Textual 8.2.8's `ColorSystem.generate()` (+ theme
//!   variables) for every non-ANSI builtin theme.
//! - `fixtures/default_css/*.tcss`: `DEFAULT_CSS` of Textual's widgets, extracted verbatim.

use rustual_css::{Origin, Stylesheet, Theme};
use rustual_render::Color;
use std::collections::{BTreeSet, HashMap};
use std::path::Path;

fn fixtures() -> &'static Path {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures"))
}

fn close(a: &str, b: &str) -> bool {
    if a.eq_ignore_ascii_case(b) {
        return true;
    }
    match (Color::parse(a), Color::parse(b)) {
        (Ok(x), Ok(y)) => {
            let d = |p: u8, q: u8| (p as i32 - q as i32).abs();
            d(x.r, y.r) <= 2 && d(x.g, y.g) <= 2 && d(x.b, y.b) <= 2 && (x.a - y.a).abs() < 0.01
        }
        _ => false,
    }
}

#[test]
fn theme_variables_match_textual() {
    let text = std::fs::read_to_string(fixtures().join("theme_variables.tsv")).unwrap();
    let mut expected: HashMap<String, HashMap<String, String>> = HashMap::new();
    for line in text.lines() {
        let mut it = line.splitn(3, '\t');
        let (t, k, v) = (it.next().unwrap(), it.next().unwrap(), it.next().unwrap());
        expected
            .entry(t.to_string())
            .or_default()
            .insert(k.to_string(), v.to_string());
    }
    let mut mismatches = Vec::new();
    for (theme_name, exp) in &expected {
        let theme = Theme::by_name(theme_name)
            .unwrap_or_else(|| panic!("builtin theme {theme_name} missing"));
        let got = theme.to_variables();
        let exp_keys: BTreeSet<_> = exp.keys().collect();
        let got_keys: BTreeSet<_> = got.keys().collect();
        assert_eq!(
            exp_keys, got_keys,
            "variable names differ for theme {theme_name}"
        );
        for (k, v) in exp {
            if !close(&got[k], v) {
                mismatches.push(format!("{theme_name}: ${k}: got {} expected {v}", got[k]));
            }
        }
    }
    assert!(mismatches.is_empty(), "{}", mismatches.join("\n"));
}

#[test]
fn textual_default_css_parses_without_errors() {
    let vars = Theme::by_name("textual-dark").unwrap().to_variables();
    let mut sheet = Stylesheet::new();
    assert!(sheet.set_variables(vars).is_empty());
    let mut entries: Vec<_> = std::fs::read_dir(fixtures().join("default_css"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    entries.sort();
    assert!(entries.len() > 50);
    let mut failures = Vec::new();
    for path in &entries {
        let css = std::fs::read_to_string(path).unwrap();
        let key = path.file_stem().unwrap().to_string_lossy().to_string();
        let errors = sheet.add_source(&key, &css, Origin::Default);
        for e in errors {
            failures.push(format!("{key}: {e}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    assert!(sheet.rule_count() > 300, "{}", sheet.rule_count());
}

#[test]
fn default_css_parses_with_every_theme() {
    let names = [
        "Button",
        "Input",
        "DataTable",
        "Footer",
        "Screen",
        "Widget",
        "Tabs",
    ];
    for theme in Theme::builtin() {
        let mut sheet = Stylesheet::new();
        sheet.set_variables(theme.to_variables());
        for n in names {
            let css =
                std::fs::read_to_string(fixtures().join("default_css").join(format!("{n}.tcss")))
                    .unwrap();
            let errors = sheet.add_source(n, &css, Origin::Default);
            assert!(errors.is_empty(), "{} / {n}: {errors:?}", theme.name);
        }
    }
}

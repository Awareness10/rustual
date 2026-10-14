//! TCSS parser: variables, nested rules (`&`), selector lists, declarations.

use crate::model::{CssError, Declaration, Rule, RuleSet};
use crate::properties::{parse_color_word, strip_important};
use crate::selector::{combine_nested, SelectorList};
use crate::styles::Styles;
use crate::tokenizer::{is_var_char, tokenize, TokenKind};
use std::collections::HashMap;
use textual_render::Color;

/// Parse `css`. `variables` are theme variables (names without `$`); variables defined
/// in the source (`$name: value;`) add to / override them for the rest of the source.
/// Unknown properties or invalid values produce errors but parsing continues; errors are returned
/// alongside whatever rules parsed successfully.
///
/// Rules are numbered `source_order_start, source_order_start + 1, ...` in source order, a
/// nested rule coming after its parent (as in Textual).
pub fn parse_stylesheet(
    css: &str,
    variables: &HashMap<String, String>,
    source_order_start: usize,
) -> (RuleSet, Vec<CssError>) {
    let (tokens, mut errors) = tokenize(css);
    let mut vars = variables.clone();
    let mut border_color = default_border_color(&vars);
    let mut stack: Vec<Frame> = Vec::new();
    let mut finished: Vec<(usize, Rule)> = Vec::new();
    let mut next_order = 0usize;

    for token in tokens {
        let err = |message: String| CssError {
            message,
            line: token.line,
            column: token.column,
        };
        match token.kind {
            TokenKind::VariableDef { name, value } => match substitute(&value, &vars) {
                Ok(v) => {
                    vars.insert(name.clone(), v);
                    if name == "foreground" {
                        border_color = default_border_color(&vars);
                    }
                }
                Err(e) => errors.push(err(e)),
            },
            TokenKind::BlockStart(text) => {
                let order = next_order;
                next_order += 1;
                let children: Vec<&str> = text.split(',').map(str::trim).collect();
                let (selectors, valid) = match stack.last() {
                    Some(parent) if !parent.valid => (vec![], false),
                    Some(parent) => (
                        parent
                            .selectors
                            .iter()
                            .flat_map(|p| children.iter().map(move |c| combine_nested(p, c)))
                            .collect(),
                        true,
                    ),
                    None => {
                        if text.contains('&') {
                            errors.push(err(format!(
                                "nesting selector '&' used outside of a nested rule in {text:?}"
                            )));
                            (vec![], false)
                        } else {
                            (children.iter().map(|c| c.to_string()).collect(), true)
                        }
                    }
                };
                let mut frame = Frame {
                    selectors,
                    valid,
                    order,
                    list: None,
                    declarations: Vec::new(),
                    styles: Styles::new(),
                    important_styles: Styles::new(),
                };
                if frame.valid {
                    match SelectorList::parse(&frame.selectors.join(", ")) {
                        Ok(list) => frame.list = Some(list),
                        Err(e) => {
                            errors.push(err(format!("invalid selector {text:?}: {e}")));
                            frame.valid = false;
                        }
                    }
                }
                stack.push(frame);
            }
            TokenKind::Declaration { name, value } => {
                let Some(frame) = stack.last_mut() else {
                    errors.push(err(format!("declaration '{name}' outside of a rule set")));
                    continue;
                };
                if !frame.valid {
                    continue;
                }
                let value = match substitute(&value, &vars) {
                    Ok(v) => v,
                    Err(e) => {
                        errors.push(err(format!("{name}: {e}")));
                        continue;
                    }
                };
                let (value, important) = strip_important(&value);
                let target = if important {
                    &mut frame.important_styles
                } else {
                    &mut frame.styles
                };
                match target.set_declaration_with(&name, value, border_color) {
                    Ok(()) => frame.declarations.push(Declaration {
                        name,
                        value: value.to_string(),
                        important,
                    }),
                    Err(e) => errors.push(err(e)),
                }
            }
            TokenKind::BlockEnd => {
                let Some(frame) = stack.pop() else { continue };
                if let (true, Some(list)) = (frame.valid, frame.list) {
                    if !frame.declarations.is_empty() {
                        finished.push((
                            frame.order,
                            Rule {
                                selectors: list,
                                declarations: frame.declarations,
                                styles: frame.styles,
                                important_styles: frame.important_styles,
                                source_order: 0,
                            },
                        ));
                    }
                }
            }
        }
    }

    finished.sort_by_key(|(order, _)| *order);
    let rules = finished
        .into_iter()
        .enumerate()
        .map(|(i, (_, mut rule))| {
            rule.source_order = source_order_start + i;
            rule
        })
        .collect();
    (RuleSet { rules }, errors)
}

struct Frame {
    /// Fully expanded complex selectors (one per list entry, nesting resolved).
    selectors: Vec<String>,
    /// False if this block's (or an ancestor's) selector failed to parse.
    valid: bool,
    order: usize,
    list: Option<SelectorList>,
    declarations: Vec<Declaration>,
    styles: Styles,
    important_styles: Styles,
}

/// Border color used when a `border:` declaration gives no color: `$foreground` when defined.
fn default_border_color(vars: &HashMap<String, String>) -> Color {
    vars.get("foreground")
        .and_then(|v| parse_color_word(v.trim()).ok())
        .unwrap_or(Color::WHITE)
}

/// Replace `$name` references with their values. Undefined variables are an error.
pub fn substitute(value: &str, vars: &HashMap<String, String>) -> Result<String, String> {
    if !value.contains('$') {
        return Ok(value.to_string());
    }
    let mut out = String::with_capacity(value.len());
    let mut chars = value.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        if c != '$' {
            out.push(c);
            continue;
        }
        let start = i + 1;
        let mut end = start;
        while let Some(&(j, d)) = chars.peek() {
            if is_var_char(d) {
                end = j + d.len_utf8();
                chars.next();
            } else {
                break;
            }
        }
        let name = &value[start..end];
        if name.is_empty() {
            return Err("expected variable name after '$'".into());
        }
        match vars.get(name) {
            Some(v) => out.push_str(v),
            None => return Err(format!("reference to undefined variable '${name}'")),
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::styles::*;
    use crate::Scalar;
    use textual_render::BorderKind;

    fn parse(css: &str) -> RuleSet {
        let (rules, errors) = parse_stylesheet(css, &HashMap::new(), 0);
        assert!(errors.is_empty(), "unexpected errors: {errors:?}");
        rules
    }

    fn vars(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn simple_rule() {
        let rs = parse("Button { width: 10; color: red; }");
        assert_eq!(rs.rules.len(), 1);
        let r = &rs.rules[0];
        assert_eq!(r.styles.width, Some(Scalar::cells(10.0)));
        assert_eq!(
            r.styles.color,
            Some(ColorValue::Color(Color::parse("red").unwrap()))
        );
        assert_eq!(r.declarations.len(), 2);
        assert_eq!(r.source_order, 0);
    }

    #[test]
    fn last_declaration_without_semicolon() {
        let rs = parse("A { width: 1; height: 2 }");
        assert_eq!(rs.rules[0].styles.height, Some(Scalar::cells(2.0)));
    }

    #[test]
    fn selector_lists() {
        let rs = parse("A, .b, #c > D { width: 1 }");
        assert_eq!(rs.rules.len(), 1);
        assert_eq!(rs.rules[0].selectors.selectors.len(), 3);
    }

    #[test]
    fn nesting_expands_selectors() {
        let rs = parse(
            "Button { width: 1; &:hover { width: 2; } &.primary { width: 3; } Label { width: 4; } }",
        );
        assert_eq!(rs.rules.len(), 4);
        let expected = [
            SelectorList::parse("Button").unwrap(),
            SelectorList::parse("Button:hover").unwrap(),
            SelectorList::parse("Button.primary").unwrap(),
            SelectorList::parse("Button Label").unwrap(),
        ];
        for (rule, exp) in rs.rules.iter().zip(expected.iter()) {
            assert_eq!(&rule.selectors, exp);
        }
        let orders: Vec<_> = rs.rules.iter().map(|r| r.source_order).collect();
        assert_eq!(orders, vec![0, 1, 2, 3]);
    }

    #[test]
    fn nesting_with_lists_is_cartesian() {
        let rs = parse("A, B { &:hover, .x { width: 1 } }");
        assert_eq!(rs.rules.len(), 1);
        assert_eq!(
            rs.rules[0].selectors,
            SelectorList::parse("A:hover, A .x, B:hover, B .x").unwrap()
        );
    }

    #[test]
    fn deep_nesting() {
        let rs = parse("A { &.b { &:focus { C { width: 1 } } } }");
        assert_eq!(rs.rules.len(), 1);
        assert_eq!(
            rs.rules[0].selectors,
            SelectorList::parse("A.b:focus C").unwrap()
        );
    }

    #[test]
    fn source_order_start_offsets() {
        let (rs, _) = parse_stylesheet("A { width: 1 } B { width: 2 }", &HashMap::new(), 10);
        let orders: Vec<_> = rs.rules.iter().map(|r| r.source_order).collect();
        assert_eq!(orders, vec![10, 11]);
    }

    #[test]
    fn variables_defined_in_source() {
        let rs = parse("$w: 7;\n$c: #ff0000;\n$c2: $c 50%;\nA { width: $w; background: $c2; }");
        let r = &rs.rules[0];
        assert_eq!(r.styles.width, Some(Scalar::cells(7.0)));
        assert_eq!(r.styles.background, Some(Color::rgba(255, 0, 0, 0.5)));
    }

    #[test]
    fn theme_variables_and_overrides() {
        let theme = vars(&[("primary", "#0000ff"), ("primary-lighten-1", "#00ff00")]);
        let (rs, errors) = parse_stylesheet(
            "A { background: $primary-lighten-1; color: $primary 20%; }\n$primary: #ff0000;\nB { background: $primary; }",
            &theme,
            0,
        );
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(rs.rules[0].styles.background, Some(Color::rgb(0, 255, 0)));
        assert_eq!(
            rs.rules[0].styles.color,
            Some(ColorValue::Color(Color::rgba(0, 0, 255, 0.2)))
        );
        assert_eq!(rs.rules[1].styles.background, Some(Color::rgb(255, 0, 0)));
    }

    #[test]
    fn undefined_variable_is_error() {
        let (rs, errors) =
            parse_stylesheet("A {\n  color: $nope;\n  width: 3;\n}", &HashMap::new(), 0);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].message.contains("$nope"), "{}", errors[0].message);
        assert_eq!((errors[0].line, errors[0].column), (2, 3));
        assert_eq!(rs.rules[0].styles.width, Some(Scalar::cells(3.0)));
        assert_eq!(rs.rules[0].styles.color, None);
    }

    #[test]
    fn important_declarations() {
        let rs = parse("A { width: 1 !important; height: 2; }");
        let r = &rs.rules[0];
        assert_eq!(r.important_styles.width, Some(Scalar::cells(1.0)));
        assert_eq!(r.styles.width, None);
        assert_eq!(r.styles.height, Some(Scalar::cells(2.0)));
        assert!(r.declarations[0].important);
        assert!(!r.declarations[1].important);
    }

    #[test]
    fn errors_reported_and_parsing_continues() {
        let css = "A {\n  bogus: 1;\n  width: nope;\n  height: 3;\n}\nB { width: 4 }";
        let (rs, errors) = parse_stylesheet(css, &HashMap::new(), 0);
        assert_eq!(errors.len(), 2, "{errors:?}");
        assert_eq!(errors[0].line, 2);
        assert!(errors[0].message.contains("unknown property"));
        assert_eq!(errors[1].line, 3);
        assert_eq!(rs.rules.len(), 2);
        assert_eq!(rs.rules[0].styles.height, Some(Scalar::cells(3.0)));
    }

    #[test]
    fn invalid_selector_skips_block() {
        let (rs, errors) = parse_stylesheet(
            "A..b { width: 1; C { width: 2 } } D { width: 3 }",
            &HashMap::new(),
            0,
        );
        assert_eq!(errors.len(), 1, "{errors:?}");
        assert_eq!(rs.rules.len(), 1);
        assert_eq!(rs.rules[0].selectors, SelectorList::parse("D").unwrap());
    }

    #[test]
    fn ampersand_at_top_level_is_error() {
        let (_, errors) = parse_stylesheet("&:hover { width: 1 }", &HashMap::new(), 0);
        assert_eq!(errors.len(), 1);
    }

    #[test]
    fn empty_rules_are_dropped() {
        let rs = parse("A { } B { C { width: 1 } }");
        assert_eq!(rs.rules.len(), 1);
    }

    #[test]
    fn border_default_color_uses_foreground() {
        let theme = vars(&[("foreground", "#e0e0e0")]);
        let (rs, errors) = parse_stylesheet("A { border: round; }", &theme, 0);
        assert!(errors.is_empty());
        assert_eq!(
            rs.rules[0].styles.border_top,
            Some(BorderEdge {
                kind: BorderKind::Round,
                color: Color::parse("#e0e0e0").unwrap()
            })
        );
        let rs = parse("A { border: round; }");
        assert_eq!(rs.rules[0].styles.border_left.unwrap().color, Color::WHITE);
    }

    #[test]
    fn comment_lines_inside_rules() {
        let rs = parse("A {\n  # color: initial;\n  width: 1;\n  /* height: 2; */\n}");
        assert_eq!(rs.rules[0].declarations.len(), 1);
    }

    #[test]
    fn substitute_greedy_names() {
        let v = vars(&[("a", "1"), ("a-b", "2")]);
        assert_eq!(substitute("$a-b $a", &v).unwrap(), "2 1");
        assert!(substitute("$", &v).is_err());
        assert!(substitute("$zz", &v).is_err());
    }
}

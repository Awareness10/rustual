//! The `transition` property (port of `textual/css/transition.py` and
//! `_styles_builder.py::process_transition`).
//!
//! ```css
//! Button { transition: background 300ms in_out_cubic, offset 0.5s linear 100ms; }
//! ```
//!
//! Each comma-separated item is `<property> <duration> [<easing>] [<delay>]`. Durations are
//! `300ms`, `0.3s` or a bare number of seconds. Defaults follow Textual: duration `1s`, easing
//! `linear`, delay `0`. A later item for the same property wins.

use std::time::Duration;

/// Easing names accepted in `transition` (Textual's `textual._easing.EASING` keys).
pub const EASING_NAMES: &[&str] = &[
    "none",
    "round",
    "linear",
    "in_sine",
    "in_out_sine",
    "out_sine",
    "in_quad",
    "in_out_quad",
    "out_quad",
    "in_cubic",
    "in_out_cubic",
    "out_cubic",
    "in_quart",
    "in_out_quart",
    "out_quart",
    "in_quint",
    "in_out_quint",
    "out_quint",
    "in_expo",
    "in_out_expo",
    "out_expo",
    "in_circ",
    "in_out_circ",
    "out_circ",
    "in_back",
    "in_out_back",
    "out_back",
    "in_elastic",
    "in_out_elastic",
    "out_elastic",
    "in_bounce",
    "in_out_bounce",
    "out_bounce",
];

/// One `transition` item: how changes of `property` are animated.
#[derive(Debug, Clone, PartialEq)]
pub struct Transition {
    /// CSS property name, normalized to use `-` (`offset`, `text-opacity`, `background`).
    pub property: String,
    pub duration: Duration,
    /// Easing name as in [`EASING_NAMES`] (normalized to use `_`).
    pub easing: String,
    pub delay: Duration,
}

impl Default for Transition {
    fn default() -> Self {
        Transition {
            property: String::new(),
            duration: Duration::from_secs(1),
            easing: "linear".to_string(),
            delay: Duration::ZERO,
        }
    }
}

/// Parse a duration: `300ms`, `0.3s`, or a bare number of seconds (Textual's
/// `_duration_as_seconds`).
pub fn parse_duration(s: &str) -> Result<Duration, String> {
    let s = s.trim();
    let (num, scale) = if let Some(n) = s.strip_suffix("ms") {
        (n, 0.001)
    } else if let Some(n) = s.strip_suffix('s') {
        (n, 1.0)
    } else {
        (s, 1.0)
    };
    let v: f64 = num
        .trim()
        .parse()
        .map_err(|_| format!("{s:?} is not a valid duration"))?;
    if !v.is_finite() || v < 0.0 {
        return Err(format!("{s:?} is not a valid duration"));
    }
    Ok(Duration::from_secs_f64(v * scale))
}

fn is_duration(s: &str) -> bool {
    s.chars()
        .next()
        .is_some_and(|c| c.is_ascii_digit() || c == '.')
        && parse_duration(s).is_ok()
}

/// Parse the value of a `transition` declaration. `none` gives an empty list.
pub fn parse_transitions(value: &str) -> Result<Vec<Transition>, String> {
    let value = value.trim();
    if value.eq_ignore_ascii_case("none") {
        return Ok(Vec::new());
    }
    let mut out: Vec<Transition> = Vec::new();
    for group in value.split(',') {
        let tokens: Vec<&str> = group.split_whitespace().collect();
        if tokens.is_empty() {
            continue;
        }
        let property = tokens[0];
        if !property
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
            || !property.starts_with(|c: char| c.is_ascii_alphabetic())
        {
            return Err(format!("transition: expected property, found {property:?}"));
        }
        let mut t = Transition {
            property: property.to_ascii_lowercase().replace('_', "-"),
            ..Default::default()
        };
        let mut rest = tokens[1..].iter();
        if let Some(d) = rest.next() {
            if !is_duration(d) {
                return Err(format!(
                    "transition: expected duration or number, found {d:?}"
                ));
            }
            t.duration = parse_duration(d)?;
        }
        if let Some(e) = rest.next() {
            let name = e.replace('-', "_");
            if !EASING_NAMES.contains(&name.as_str()) {
                return Err(format!("transition: expected easing function; found {e:?}"));
            }
            t.easing = name;
        }
        if let Some(d) = rest.next() {
            if !is_duration(d) {
                return Err(format!(
                    "transition: expected duration or number, found {d:?}"
                ));
            }
            t.delay = parse_duration(d)?;
        }
        if let Some(extra) = rest.next() {
            return Err(format!("transition: unexpected {extra:?}"));
        }
        out.retain(|o| o.property != t.property);
        out.push(t);
    }
    if out.is_empty() {
        return Err("missing value for 'transition'".to_string());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Styles;

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    #[test]
    fn durations() {
        assert_eq!(parse_duration("300ms").unwrap(), ms(300));
        assert_eq!(parse_duration("0.3s").unwrap(), ms(300));
        assert_eq!(parse_duration("2").unwrap(), ms(2000));
        assert!(parse_duration("fast").is_err());
        assert!(parse_duration("-1s").is_err());
    }

    #[test]
    fn single_and_defaults() {
        let t = parse_transitions("offset 500ms").unwrap();
        assert_eq!(
            t,
            vec![Transition {
                property: "offset".into(),
                duration: ms(500),
                easing: "linear".into(),
                delay: Duration::ZERO,
            }]
        );
        let t = parse_transitions("opacity").unwrap();
        assert_eq!(t[0].duration, ms(1000));
    }

    #[test]
    fn full_list() {
        let t = parse_transitions("background 0.3s in_out_cubic, text_opacity 200ms out-bounce 1s")
            .unwrap();
        assert_eq!(t.len(), 2);
        assert_eq!(t[0].property, "background");
        assert_eq!(t[0].duration, ms(300));
        assert_eq!(t[0].easing, "in_out_cubic");
        assert_eq!(t[1].property, "text-opacity");
        assert_eq!(t[1].easing, "out_bounce");
        assert_eq!(t[1].delay, ms(1000));
        // Later duplicates win.
        let t = parse_transitions("width 1s, width 2s").unwrap();
        assert_eq!(t.len(), 1);
        assert_eq!(t[0].duration, ms(2000));
        assert_eq!(parse_transitions("none").unwrap(), vec![]);
    }

    #[test]
    fn errors() {
        assert!(parse_transitions("offset fast").is_err());
        assert!(parse_transitions("offset 1s bogus").is_err());
        assert!(parse_transitions("offset 1s linear soon").is_err());
        assert!(parse_transitions("offset 1s linear 1s extra").is_err());
        assert!(parse_transitions("1s").is_err());
        assert!(Styles::parse_inline("transition:").is_err());
    }

    #[test]
    fn styles_field() {
        let st = Styles::parse_inline("transition: offset 1s, background 300ms out_cubic").unwrap();
        let t = st.transitions.as_ref().unwrap();
        assert_eq!(t.len(), 2);
        assert_eq!(st.get_transition("background").unwrap().easing, "out_cubic");
        assert_eq!(st.get_transition("offset").unwrap().duration, ms(1000));
        assert!(st.get_transition("width").is_none());
        // `initial` leaves it unset.
        assert_eq!(
            Styles::parse_inline("transition: initial")
                .unwrap()
                .transitions,
            None
        );
        // Merge replaces the whole list; not inherited.
        let mut a = st.clone();
        a.merge(&Styles::parse_inline("transition: width 2s").unwrap());
        assert_eq!(a.transitions.as_ref().unwrap().len(), 1);
        let mut child = Styles::new();
        child.inherit_from(&st);
        assert_eq!(child.transitions, None);
        // Through a stylesheet.
        let css = "Button { transition: tint 250ms in_quad 50ms; }";
        let (rules, errors) = crate::parser::parse_stylesheet(css, &Default::default(), 0);
        assert!(errors.is_empty(), "{errors:?}");
        let tr = rules.rules[0].styles.get_transition("tint").unwrap();
        assert_eq!((tr.duration, tr.delay), (ms(250), ms(50)));
    }
}

//! Textual content markup: `"[bold red]Hello[/] [on $accent]world[/on $accent]"`.
//!
//! Tags: style strings accepted by [`Style::parse_with`] (`b`, `bold`, `i`, `u`, `s`, `d`, `r`,
//! colors, `on <color>`, `$var`), `[@click=action]` (stored in `Style::link`).
//! `[/]` closes the most recent tag, `[/tag]` closes the matching tag. `\[` escapes a bracket.
//! A `[` not followed by a valid tag (e.g. `[1, 2]`) is treated as literal text, like Textual.

use crate::color::Color;
use crate::strip::push_merged;
use crate::style::{split_style_words, Style};
use crate::text::Text;
use std::fmt;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkupError(pub String);

impl fmt::Display for MarkupError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "markup error: {}", self.0)
    }
}

impl std::error::Error for MarkupError {}

/// An open tag on the style stack.
struct OpenTag {
    /// Trimmed tag body, e.g. `"bold red"` or `"@click=app.bell"`.
    raw: String,
    style: Style,
}

impl OpenTag {
    /// Does closing tag body `close` (trimmed, non-empty) close this tag?
    /// Matches on equal text, on an equal parsed style (so `[/b]` closes `[bold]`), or on the
    /// key alone for `key=value` tags (`[/@click]` closes `[@click=app.bell]`).
    fn matches(&self, close: &str, close_style: Option<&Style>) -> bool {
        if self.raw == close {
            return true;
        }
        if let Some(cs) = close_style {
            if *cs == self.style {
                return true;
            }
        }
        self.raw
            .strip_prefix(close)
            .is_some_and(|rest| rest.starts_with('=') && !close.contains('='))
    }
}

fn is_key(k: &str) -> bool {
    let mut chars = k.chars();
    matches!(chars.next(), Some(c) if c == '@' || c == '_' || c == '-' || c.is_ascii_alphabetic())
        && chars.all(|c| c == '_' || c == '-' || c.is_ascii_alphanumeric())
}

fn unquote(v: &str) -> &str {
    let b = v.as_bytes();
    if b.len() >= 2 && (b[0] == b'\'' || b[0] == b'"') && b[b.len() - 1] == b[0] {
        &v[1..v.len() - 1]
    } else {
        v
    }
}

/// Parse a tag body into a style. `@click=...` / `link=...` set [`Style::link`]; other
/// `key=value` pairs are accepted and ignored (Textual stores them as meta).
fn parse_tag(body: &str, vars: &dyn Fn(&str) -> Option<Color>) -> Option<Style> {
    let mut style_words = Vec::new();
    let mut link: Option<Arc<str>> = None;
    for word in split_style_words(body) {
        if let Some((key, value)) = word.split_once('=') {
            if is_key(key) {
                if key == "@click" || key == "link" {
                    link = Some(Arc::from(unquote(value)));
                }
                continue;
            }
            return None;
        }
        style_words.push(word);
    }
    let mut style = match Style::parse_with(&style_words.join(" "), vars) {
        Ok(style) => style,
        Err(_) => parse_lenient(&style_words, vars)?,
    };
    if link.is_some() {
        style.link = link;
    }
    Some(style)
}

/// Is `word` a token Textual's markup tokenizer accepts in a tag (identifier, `$variable`,
/// color or percentage)? Tags made only of such tokens are tags even when a word isn't a known
/// style (`[foo]`, `[on $unknown]`); anything else (`[1, 2]`, `[repr.str]`) is literal text.
fn is_style_token(word: &str) -> bool {
    let ident = |w: &str, first_digit_ok: bool| {
        let mut chars = w.chars();
        matches!(chars.next(), Some(c) if c == '_' || c.is_ascii_alphabetic()
            || (first_digit_ok && (c == '-' || c.is_ascii_alphanumeric())))
            && chars.all(|c| c == '_' || c == '-' || c.is_ascii_alphanumeric())
    };
    if let Some(var) = word.strip_prefix('$') {
        return ident(var, true);
    }
    if let Some(hex) = word.strip_prefix('#') {
        return matches!(hex.len(), 3 | 4 | 6 | 8) && hex.chars().all(|c| c.is_ascii_hexdigit());
    }
    let lower = word.to_ascii_lowercase();
    if ["rgb(", "rgba(", "hsl(", "hsla("]
        .iter()
        .any(|p| lower.starts_with(p))
        && lower.ends_with(')')
    {
        return true;
    }
    if let Some(num) = word.strip_suffix('%') {
        return num.trim_start_matches('-').parse::<f32>().is_ok();
    }
    ident(word, false)
}

/// Parse style words, skipping the ones that aren't styles (as Textual does). `on` / `not`
/// are kept together with the following word. Returns `None` if a word isn't a valid token.
fn parse_lenient(words: &[&str], vars: &dyn Fn(&str) -> Option<Color>) -> Option<Style> {
    if !words.iter().all(|w| is_style_token(w)) {
        return None;
    }
    let mut kept: Vec<&str> = Vec::new();
    let mut i = 0;
    while i < words.len() {
        let pair = matches!(words[i].to_ascii_lowercase().as_str(), "on" | "not");
        let take = if pair { 2 } else { 1 }.min(words.len() - i);
        let chunk = &words[i..i + take];
        let mut trial = kept.clone();
        trial.extend_from_slice(chunk);
        if Style::parse_with(&trial.join(" "), vars).is_ok() {
            kept = trial;
        }
        i += take;
    }
    Style::parse_with(&kept.join(" "), vars).ok()
}

/// Find the `]` closing a tag opened at byte `open` (which holds `[`). Returns `None` if the
/// tag is unterminated or contains a nested `[` outside a `key=value` expression, in which
/// case the `[` is literal text.
fn find_tag_end(s: &str, open: usize) -> Option<usize> {
    let mut in_value = false;
    let mut depth = 0i32;
    let mut quote: Option<char> = None;
    for (off, c) in s[open + 1..].char_indices() {
        let i = open + 1 + off;
        if let Some(q) = quote {
            if c == q {
                quote = None;
            }
            continue;
        }
        if in_value {
            match c {
                '\'' | '"' => quote = Some(c),
                '(' | '[' | '{' => depth += 1,
                ']' if depth == 0 => return Some(i),
                ')' | ']' | '}' => depth -= 1,
                c if c.is_whitespace() && depth == 0 => in_value = false,
                _ => {}
            }
            continue;
        }
        match c {
            ']' => return Some(i),
            '[' => return None,
            '=' => in_value = true,
            _ => {}
        }
    }
    None
}

pub fn parse_markup(s: &str, vars: &dyn Fn(&str) -> Option<Color>) -> Result<Text, MarkupError> {
    let mut stack: Vec<OpenTag> = Vec::new();
    let mut spans = Vec::new();
    let mut buf = String::new();

    let flush = |buf: &mut String, stack: &[OpenTag], spans: &mut Vec<_>| {
        if buf.is_empty() {
            return;
        }
        let style = stack
            .iter()
            .fold(Style::default(), |acc, t| acc.combine(&t.style));
        push_merged(spans, buf, &style);
        buf.clear();
    };

    let mut i = 0;
    while i < s.len() {
        let rest = &s[i..];
        if rest.starts_with("\\[") {
            buf.push('[');
            i += 2;
            continue;
        }
        if !rest.starts_with('[') {
            let c = rest.chars().next().expect("non-empty");
            buf.push(c);
            i += c.len_utf8();
            continue;
        }
        let Some(end) = find_tag_end(s, i) else {
            buf.push('[');
            i += 1;
            continue;
        };
        let body = &s[i + 1..end];
        if let Some(close) = body.strip_prefix('/') {
            let close = close.trim();
            flush(&mut buf, &stack, &mut spans);
            if close.is_empty() {
                if stack.pop().is_none() {
                    return Err(MarkupError(
                        "auto closing tag ('[/]') has nothing to close".into(),
                    ));
                }
            } else {
                let close_style = parse_tag(close, vars);
                match stack
                    .iter()
                    .rposition(|t| t.matches(close, close_style.as_ref()))
                {
                    Some(pos) => {
                        stack.remove(pos);
                    }
                    None => {
                        return Err(MarkupError(format!(
                            "closing tag '[/{close}]' does not match any open tag"
                        )))
                    }
                }
            }
        } else {
            let trimmed = body.trim();
            match (!trimmed.is_empty())
                .then(|| parse_tag(trimmed, vars))
                .flatten()
            {
                Some(style) => {
                    flush(&mut buf, &stack, &mut spans);
                    stack.push(OpenTag {
                        raw: trimmed.to_string(),
                        style,
                    });
                }
                None => buf.push_str(&s[i..=end]),
            }
        }
        i = end + 1;
    }
    flush(&mut buf, &stack, &mut spans);
    Ok(Text { spans })
}

/// Escape `[` so text is rendered literally when interpreted as markup.
pub fn escape(s: &str) -> String {
    s.replace('[', "\\[")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(s: &str) -> Text {
        parse_markup(s, &|_| None).unwrap()
    }

    fn red() -> Color {
        Color::parse("red").unwrap()
    }

    #[test]
    fn plain() {
        let t = parse("hello world");
        assert_eq!(t.spans.len(), 1);
        assert_eq!(t.plain_text(), "hello world");
        assert!(t.spans[0].style.is_empty());
        assert!(parse("").spans.is_empty());
    }

    #[test]
    fn simple_tag() {
        let t = parse("a [bold red]b[/bold red] c");
        assert_eq!(t.plain_text(), "a b c");
        assert_eq!(t.spans.len(), 3);
        assert_eq!(t.spans[1].text, "b");
        assert_eq!(t.spans[1].style.bold, Some(true));
        assert_eq!(t.spans[1].style.fg, Some(red()));
        assert!(t.spans[2].style.is_empty());
    }

    #[test]
    fn auto_close_and_nesting() {
        let t = parse("[b]x[i]y[/]z[/]w");
        assert_eq!(t.plain_text(), "xyzw");
        let styles: Vec<_> = t
            .spans
            .iter()
            .map(|s| (s.style.bold, s.style.italic))
            .collect();
        assert_eq!(
            styles,
            vec![
                (Some(true), None),
                (Some(true), Some(true)),
                (Some(true), None),
                (None, None)
            ]
        );
    }

    #[test]
    fn close_by_abbreviation_and_out_of_order() {
        let t = parse("[bold][italic]x[/b]y[/i]");
        assert_eq!(t.plain_text(), "xy");
        assert_eq!(t.spans[0].style.bold, Some(true));
        assert_eq!(t.spans[0].style.italic, Some(true));
        assert_eq!(t.spans[1].style.bold, None);
        assert_eq!(t.spans[1].style.italic, Some(true));
    }

    #[test]
    fn background_and_vars() {
        let vars = |n: &str| (n == "panel").then_some(Color::rgb(1, 2, 3));
        let t = parse_markup("[on $panel]x[/on $panel]", &vars).unwrap();
        assert_eq!(t.spans[0].style.bg, Some(Color::rgb(1, 2, 3)));
        // unknown variable: no effect (Textual)
        let t = parse("[on $panel]x");
        assert_eq!(t.plain_text(), "x");
        assert_eq!(t.spans[0].style.bg, None);
    }

    #[test]
    fn click_link() {
        let t = parse("[@click=app.bell]ring[/] done");
        assert_eq!(t.spans[0].text, "ring");
        assert_eq!(t.spans[0].style.link.as_deref(), Some("app.bell"));
        assert_eq!(t.spans[1].style.link, None);
        let t = parse("[@click=app.set_theme('my theme') bold]x[/@click]y");
        assert_eq!(
            t.spans[0].style.link.as_deref(),
            Some("app.set_theme('my theme')")
        );
        assert_eq!(t.spans[0].style.bold, Some(true));
        assert_eq!(t.plain_text(), "xy");
        let t = parse("[@click=\"app.bell\"]x");
        assert_eq!(t.spans[0].style.link.as_deref(), Some("app.bell"));
    }

    #[test]
    fn escapes() {
        let t = parse("\\[bold]not bold");
        assert_eq!(t.plain_text(), "[bold]not bold");
        assert!(t.spans[0].style.is_empty());
        assert_eq!(parse(&escape("[b]x[/b]")).plain_text(), "[b]x[/b]");
    }

    #[test]
    fn literal_brackets() {
        assert_eq!(parse("list [1, 2]").plain_text(), "list [1, 2]");
        assert_eq!(parse("[ ]").plain_text(), "[ ]");
        assert_eq!(parse("[]").plain_text(), "[]");
        assert_eq!(parse("a [ b").plain_text(), "a [ b");
        assert_eq!(parse("trailing [").plain_text(), "trailing [");
        let t = parse("[[b]x]");
        assert_eq!(t.plain_text(), "[x]");
        assert_eq!(t.spans[1].style.bold, Some(true));
    }

    #[test]
    fn unclosed_extends_to_end() {
        let t = parse("a[red]bc");
        assert_eq!(t.spans.len(), 2);
        assert_eq!(t.spans[1].text, "bc");
        assert_eq!(t.spans[1].style.fg, Some(red()));
    }

    #[test]
    fn errors() {
        assert!(parse_markup("x[/]", &|_| None).is_err());
        assert!(parse_markup("[b]x[/i]", &|_| None).is_err());
        assert!(parse_markup("[/red]", &|_| None).is_err());
        // Text::from_markup falls back to plain text
        assert_eq!(Text::from_markup("x[/]").plain_text(), "x[/]");
    }

    #[test]
    fn alpha_tag_composites() {
        let t = parse("[on #000000][on #ff0000 50%]x");
        let bg = t.spans[0].style.bg.unwrap();
        assert_eq!(bg.a, 1.0);
        assert_eq!(bg.r, 127);
    }

    #[test]
    fn wide_text_preserved() {
        let t = parse("[b]中文[/b]😀");
        assert_eq!(t.plain_text(), "中文😀");
        assert_eq!(t.spans.len(), 2);
    }

    #[test]
    fn unknown_style_words_are_ignored() {
        // Textual keeps tags made of valid tokens; words that aren't styles have no effect.
        let t = parse("[foo]x[/foo]y");
        assert_eq!(t.plain_text(), "xy");
        assert!(t.spans.iter().all(|s| s.style.is_empty()));
        assert_eq!(parse("[x] done").plain_text(), " done");
        assert_eq!(parse("[foo]x[/]y").plain_text(), "xy");
        let t = parse("[b]a[/b]=[foo]x[/foo]");
        assert_eq!(t.plain_text(), "a=x");
        assert_eq!(t.spans[0].style.bold, Some(true));
        let t = parse("[bold foo]x[/bold foo]y");
        assert_eq!(t.plain_text(), "xy");
        assert_eq!(t.spans[0].style.bold, Some(true));
        assert_eq!(t.spans[1].style.bold, None);
        let t = parse("[red foo 50%]x");
        assert_eq!(t.spans[0].style.fg, Some(red().multiply_alpha(0.5)));
        assert_eq!(parse("[on $nope]x").plain_text(), "x");
        assert_eq!(parse("[$nope]x").plain_text(), "x");
        assert_eq!(parse("[on]x").plain_text(), "x");
        assert_eq!(parse("[Foo-Bar_2]x[/Foo-Bar_2]").plain_text(), "x");
        // not valid tokens: still literal text
        assert_eq!(parse("[repr.str]x").plain_text(), "[repr.str]x");
        assert_eq!(parse("list [1, 2]").plain_text(), "list [1, 2]");
    }
}

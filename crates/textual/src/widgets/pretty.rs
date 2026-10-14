//! `Pretty` (port of `textual/widgets/_pretty.py`): pretty-prints a value's `Debug`
//! representation with syntax highlighting.
//!
//! ```ignore
//! Pretty::new(&vec![("a", 1), ("b", 2)])
//! ```
//!
//! Like Rich's `Pretty`, the value is shown on one line (`{:?}`) when it fits the width and
//! expanded (`{:#?}`) otherwise. Highlighting is a hand-written port of the gist of Rich's
//! `ReprHighlighter`, adapted to Rust's `Debug` output (`true` / `false` / `None`, struct and
//! variant names, field names). Token styles come from component classes
//! (`pretty--str`, `pretty--number`, ...) whose defaults match Rich's `repr.*` styles under
//! Textual's ANSI themes (Monokai in dark mode, Alabaster in light mode).
//!
//! Deviations from Rich: expansion is all-or-nothing (Rich expands nested containers
//! individually); no indent guides; no max length / string truncation.

use crate::context::Ctx;
use crate::widget::{Content, RenderCtx, Widget};
use std::fmt::Debug;
use textual_render::{cells::cell_len, Style, Text};

const DEFAULT_CSS: &str = r#"
Pretty {
    height: auto;

    & > .pretty--str { color: #98E024; }
    & > .pretty--number { color: #58D1EB; text-style: bold; }
    & > .pretty--bool-true { color: #98E024; text-style: italic; }
    & > .pretty--bool-false { color: #F4005F; text-style: italic; }
    & > .pretty--none { color: #F4005F; text-style: italic; }
    & > .pretty--call { color: #F4005F; text-style: bold; }
    & > .pretty--attrib-name { color: #FD971F; }
    & > .pretty--brace { text-style: bold; }
    & > .pretty--ellipsis { color: #FD971F; }

    &:light {
        & > .pretty--str { color: #448C27; }
        & > .pretty--number { color: #0083B2; }
        & > .pretty--bool-true { color: #60CB00; }
        & > .pretty--bool-false { color: #F05050; }
        & > .pretty--none { color: #7A3E9D; }
        & > .pretty--call { color: #7A3E9D; }
        & > .pretty--attrib-name { color: #CB9000; }
        & > .pretty--ellipsis { color: #CB9000; }
    }
}
"#;

/// The kind of a highlighted token (Rich's `repr.*` style names).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReprToken {
    Str,
    Number,
    BoolTrue,
    BoolFalse,
    None,
    /// A struct / tuple-struct / variant name (`Foo {`, `Some(`).
    Call,
    /// A field name (`name:`).
    AttribName,
    Brace,
    Ellipsis,
}

impl ReprToken {
    /// The component class for the token.
    pub fn component_class(self) -> &'static str {
        match self {
            ReprToken::Str => "pretty--str",
            ReprToken::Number => "pretty--number",
            ReprToken::BoolTrue => "pretty--bool-true",
            ReprToken::BoolFalse => "pretty--bool-false",
            ReprToken::None => "pretty--none",
            ReprToken::Call => "pretty--call",
            ReprToken::AttribName => "pretty--attrib-name",
            ReprToken::Brace => "pretty--brace",
            ReprToken::Ellipsis => "pretty--ellipsis",
        }
    }
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Split a `Debug` representation into `(text, token)` runs (`None` = unhighlighted).
pub fn tokenize_repr(s: &str) -> Vec<(String, Option<ReprToken>)> {
    let chars: Vec<char> = s.chars().collect();
    let n = chars.len();
    let mut out: Vec<(String, Option<ReprToken>)> = Vec::new();
    let mut push = |text: String, tok: Option<ReprToken>| {
        if let Some(last) = out.last_mut() {
            if last.1 == tok && tok.is_none() {
                last.0.push_str(&text);
                return;
            }
        }
        out.push((text, tok));
    };
    let collect = |a: usize, b: usize| chars[a..b].iter().collect::<String>();
    let mut i = 0;
    while i < n {
        let c = chars[i];
        let prev_word = i > 0 && is_word(chars[i - 1]);
        // Strings: "..." with backslash escapes.
        if c == '"' {
            let mut j = i + 1;
            while j < n && chars[j] != '"' {
                if chars[j] == '\\' {
                    j += 1;
                }
                j += 1;
            }
            let end = (j + 1).min(n);
            push(collect(i, end), Some(ReprToken::Str));
            i = end;
            continue;
        }
        // Chars: 'x', '\n', '\u{1f600}'.
        if c == '\'' && !prev_word {
            let mut j = i + 1;
            if j < n && chars[j] == '\\' {
                j += 1;
                if j < n && chars[j] == 'u' {
                    while j < n && chars[j] != '}' && chars[j] != '\'' {
                        j += 1;
                    }
                }
            }
            j += 1;
            if j < n && chars[j] == '\'' {
                push(collect(i, j + 1), Some(ReprToken::Str));
                i = j + 1;
                continue;
            }
        }
        // Numbers: -?digits(.digits)?(e[-+]?digits)?
        let starts_number =
            c.is_ascii_digit() || (c == '-' && i + 1 < n && chars[i + 1].is_ascii_digit());
        if starts_number && !prev_word {
            let mut j = i + 1;
            while j < n && (chars[j].is_ascii_digit() || chars[j] == '_') {
                j += 1;
            }
            if j + 1 < n && chars[j] == '.' && chars[j + 1].is_ascii_digit() {
                j += 1;
                while j < n && chars[j].is_ascii_digit() {
                    j += 1;
                }
            }
            if j < n && (chars[j] == 'e' || chars[j] == 'E') {
                let mut k = j + 1;
                if k < n && (chars[k] == '-' || chars[k] == '+') {
                    k += 1;
                }
                if k < n && chars[k].is_ascii_digit() {
                    while k < n && chars[k].is_ascii_digit() {
                        k += 1;
                    }
                    j = k;
                }
            }
            if j >= n || !is_word(chars[j]) {
                push(collect(i, j), Some(ReprToken::Number));
                i = j;
                continue;
            }
        }
        // Identifiers.
        if (c.is_alphabetic() || c == '_') && !prev_word {
            let mut j = i + 1;
            while j < n && is_word(chars[j]) {
                j += 1;
            }
            let word = collect(i, j);
            let next = chars.get(j).copied();
            let after_space = {
                let mut k = j;
                while k < n && chars[k] == ' ' {
                    k += 1;
                }
                chars.get(k).copied()
            };
            let tok = match word.as_str() {
                "true" => Some(ReprToken::BoolTrue),
                "false" => Some(ReprToken::BoolFalse),
                "None" => Some(ReprToken::None),
                "inf" | "NaN" => Some(ReprToken::Number),
                _ if next == Some('(') => Some(ReprToken::Call),
                _ if next == Some(':') && chars.get(j + 1) != Some(&':') => {
                    Some(ReprToken::AttribName)
                }
                _ if after_space == Some('{') && word.starts_with(char::is_uppercase) => {
                    Some(ReprToken::Call)
                }
                _ => None,
            };
            push(word, tok);
            i = j;
            continue;
        }
        if matches!(c, '[' | ']' | '{' | '}' | '(' | ')') {
            push(c.to_string(), Some(ReprToken::Brace));
            i += 1;
            continue;
        }
        if c == '.' && i + 1 < n && chars[i + 1] == '.' {
            let mut j = i;
            while j < n && chars[j] == '.' {
                j += 1;
            }
            push(collect(i, j), Some(ReprToken::Ellipsis));
            i = j;
            continue;
        }
        push(c.to_string(), None);
        i += 1;
    }
    out
}

/// Highlight a `Debug` representation with `style_for` giving each token's style.
pub fn highlight_repr(s: &str, style_for: &dyn Fn(ReprToken) -> Style) -> Text {
    let mut text = Text::new();
    for (chunk, tok) in tokenize_repr(s) {
        let style = tok.map(style_for).unwrap_or_default();
        text.push(chunk, style);
    }
    text
}

/// Displays a pretty-printed, highlighted `Debug` representation of a value.
#[derive(Debug, Clone, PartialEq)]
pub struct Pretty {
    compact: String,
    expanded: String,
}

impl Pretty {
    pub fn new<T: Debug + ?Sized>(value: &T) -> Self {
        Pretty {
            compact: format!("{value:?}"),
            expanded: format!("{value:#?}"),
        }
    }

    /// Replace the displayed value.
    pub fn update<T: Debug + ?Sized>(&mut self, value: &T) {
        *self = Pretty::new(value);
    }

    /// Like [`Pretty::update`], for use from a handler of this widget (requests a repaint).
    pub fn update_with<T: Debug + ?Sized>(&mut self, value: &T, ctx: &mut Ctx) {
        self.update(value);
        ctx.refresh();
    }

    /// The representation used at `width` (compact if it fits on one line).
    pub fn repr_for_width(&self, width: i32) -> &str {
        if !self.compact.contains('\n') && cell_len(&self.compact) <= width {
            &self.compact
        } else {
            &self.expanded
        }
    }

    fn text(&self, ctx: &RenderCtx, width: i32) -> Text {
        let style_for = |tok: ReprToken| ctx.component_style(tok.component_class());
        highlight_repr(self.repr_for_width(width), &style_for)
    }
}

impl Widget for Pretty {
    fn type_names(&self) -> &'static [&'static str] {
        &["Pretty"]
    }

    fn default_css(&self) -> &'static str {
        DEFAULT_CSS
    }

    fn content_width(&self, _ctx: &RenderCtx) -> Option<i32> {
        Some(cell_len(&self.compact))
    }

    fn content_height(&self, ctx: &RenderCtx, width: i32) -> Option<i32> {
        let repr = self.repr_for_width(width);
        let text = Text::plain(repr);
        let (overflow, no_wrap) = ctx.text_overflow();
        Some(
            text.wrap(
                width.max(1),
                textual_render::Justify::Left,
                overflow,
                no_wrap,
            )
            .len() as i32,
        )
    }

    fn render(&self, ctx: &RenderCtx) -> Content {
        Content::Text(self.text(ctx, ctx.size.width))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn toks(s: &str) -> Vec<(String, Option<ReprToken>)> {
        tokenize_repr(s)
            .into_iter()
            .filter(|(_, t)| t.is_some())
            .collect()
    }

    #[test]
    fn tokenizes_rust_debug() {
        #[derive(Debug)]
        #[allow(dead_code)]
        struct P {
            name: &'static str,
            n: i32,
            f: f64,
            ok: bool,
            no: Option<u8>,
            some: Option<char>,
        }
        let s = format!(
            "{:?}",
            P {
                name: "a\"b",
                n: -12,
                f: 1.5e-7,
                ok: true,
                no: None,
                some: Some('x'),
            }
        );
        let t = toks(&s);
        use ReprToken::{AttribName, BoolTrue, Brace, Call, Number, Str};
        let expect = [
            ("P", Call),
            ("{", Brace),
            ("name", AttribName),
            ("\"a\\\"b\"", Str),
            ("n", AttribName),
            ("-12", Number),
            ("f", AttribName),
            ("1.5e-7", Number),
            ("ok", AttribName),
            ("true", BoolTrue),
            ("no", AttribName),
            ("None", ReprToken::None),
            ("some", AttribName),
            ("Some", Call),
            ("(", Brace),
            ("'x'", Str),
            (")", Brace),
            ("}", Brace),
        ];
        let got: Vec<(&str, ReprToken)> = t.iter().map(|(s, t)| (s.as_str(), t.unwrap())).collect();
        assert_eq!(got, expect.to_vec(), "{s}");
    }

    #[test]
    fn words_with_digits_are_not_numbers() {
        assert!(toks("abc123 x1").is_empty());
        assert_eq!(toks("[1, 2]").len(), 4);
        assert_eq!(toks("Foo { .. }")[2].1, Some(ReprToken::Ellipsis));
    }
}

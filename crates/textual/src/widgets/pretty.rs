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
//! [`tokenize_text`] / [`highlight_text`] port Rich's `ReprHighlighter` for arbitrary text
//! (used by [`RichLog::highlight`](crate::widgets::RichLog::highlight)), with
//! [`repr_style`] giving the same default colors without CSS.
//!
//! Deviations from Rich: expansion is all-or-nothing (Rich expands nested containers
//! individually); no indent guides; no max length / string truncation.

use crate::context::Ctx;
use crate::widget::{Content, RenderCtx, Widget};
use std::fmt::Debug;
use textual_render::{cells::cell_len, Color, Style, Text};

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
    /// The value of a `name=value` attribute (when not a string / number).
    AttribValue,
    Url,
    /// The directory part of a path (`/usr/lib/`).
    Path,
    /// The file name after a [`ReprToken::Path`].
    Filename,
    Ipv4,
    Uuid,
    /// `<` / `>` of a tag.
    Tag,
    TagName,
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
            ReprToken::AttribValue => "pretty--attrib-value",
            ReprToken::Url => "pretty--url",
            ReprToken::Path => "pretty--path",
            ReprToken::Filename => "pretty--filename",
            ReprToken::Ipv4 => "pretty--ipv4",
            ReprToken::Uuid => "pretty--uuid",
            ReprToken::Tag => "pretty--tag",
            ReprToken::TagName => "pretty--tag-name",
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

// ------------------------------------------------------------ text highlighter

/// Rich's default `repr.*` styles under Textual's ANSI themes (Monokai when `dark`, else
/// Alabaster): the styles [`highlight_text`] uses when no CSS is available (e.g. `RichLog`).
pub fn repr_style(tok: ReprToken, dark: bool) -> Style {
    // (dark, light) colors of the ANSI names Rich uses.
    let c = |d: (u8, u8, u8), l: (u8, u8, u8)| {
        let (r, g, b) = if dark { d } else { l };
        Style::new().fg(Color::rgb(r, g, b))
    };
    let green = c((152, 224, 36), (68, 140, 39));
    let bright_green = c((152, 224, 36), (96, 203, 0));
    let bright_red = c((244, 0, 95), (240, 80, 80));
    let yellow = c((253, 151, 31), (203, 144, 0));
    let bright_yellow = c((224, 213, 97), (255, 188, 93));
    let bright_blue = c((157, 101, 255), (0, 122, 204));
    let magenta = c((244, 0, 95), (122, 62, 157));
    let bright_magenta = c((244, 0, 95), (230, 76, 230));
    let cyan = c((88, 209, 235), (0, 131, 178));
    match tok {
        ReprToken::Str => green,
        ReprToken::Number => cyan.bold(),
        ReprToken::BoolTrue => bright_green.italic(),
        ReprToken::BoolFalse => bright_red.italic(),
        ReprToken::None => magenta.italic(),
        ReprToken::Call => magenta.bold(),
        ReprToken::AttribName => yellow,
        ReprToken::AttribValue => magenta,
        ReprToken::Brace | ReprToken::Tag => Style::new().bold(),
        ReprToken::TagName => bright_magenta.bold(),
        ReprToken::Ellipsis => yellow,
        ReprToken::Url => bright_blue.underline(),
        ReprToken::Path => magenta,
        ReprToken::Filename => bright_magenta,
        ReprToken::Ipv4 => bright_green.bold(),
        ReprToken::Uuid => bright_yellow,
    }
}

fn is_hex(c: char) -> bool {
    c.is_ascii_hexdigit()
}

fn is_path_char(c: char) -> bool {
    is_word(c) || matches!(c, '-' | '.' | '+')
}

fn is_url_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || "-$_+!`(),.?/;:&=%#~@".contains(c)
}

/// Length of a `8-4-4-4-12` hex UUID at `i`, if any.
fn uuid_at(chars: &[char], i: usize) -> Option<usize> {
    let mut j = i;
    for (g, n) in [8, 4, 4, 4, 12].into_iter().enumerate() {
        if g > 0 {
            if chars.get(j) != Some(&'-') {
                return None;
            }
            j += 1;
        }
        for _ in 0..n {
            if !chars.get(j).copied().is_some_and(is_hex) {
                return None;
            }
            j += 1;
        }
    }
    (!chars.get(j).copied().is_some_and(is_word)).then_some(j - i)
}

/// Length of a dotted IPv4 address at `i`, if any.
fn ipv4_at(chars: &[char], i: usize) -> Option<usize> {
    let mut j = i;
    for g in 0..4 {
        if g > 0 {
            if chars.get(j) != Some(&'.') {
                return None;
            }
            j += 1;
        }
        let start = j;
        while j < chars.len() && j - start < 3 && chars[j].is_ascii_digit() {
            j += 1;
        }
        if j == start {
            return None;
        }
    }
    (!chars
        .get(j)
        .copied()
        .is_some_and(|c| is_word(c) || c == '.'))
    .then_some(j - i)
}

/// Split arbitrary text into `(text, token)` runs like Rich's `ReprHighlighter` (used by
/// `RichLog::highlight`): strings, numbers, `True` / `False` / `None` (and Rust's `true` /
/// `false`), calls, `name=value` attributes, braces, `<tags>`, `...`, URLs, paths, IPv4
/// addresses and UUIDs.
///
/// Deviations from Rich's regexes: tags end at the first `>`; no IPv6 / EUI / complex numbers.
pub fn tokenize_text(s: &str) -> Vec<(String, Option<ReprToken>)> {
    let chars: Vec<char> = s.chars().collect();
    let n = chars.len();
    let mut out: Vec<(String, Option<ReprToken>)> = Vec::new();
    let mut push = |text: String, tok: Option<ReprToken>| {
        if let Some(last) = out.last_mut() {
            if last.1.is_none() && tok.is_none() {
                last.0.push_str(&text);
                return;
            }
        }
        out.push((text, tok));
    };
    let collect = |a: usize, b: usize| chars[a..b].iter().collect::<String>();
    let starts_with = |i: usize, p: &str| {
        p.chars()
            .enumerate()
            .all(|(k, pc)| chars.get(i + k) == Some(&pc))
    };
    let mut in_tag = false;
    let mut i = 0;
    while i < n {
        let c = chars[i];
        let prev = (i > 0).then(|| chars[i - 1]);
        let prev_word = prev.is_some_and(is_word);
        // URLs.
        if !prev_word {
            if let Some(scheme) = ["https://", "http://", "file://", "wss://", "ws://"]
                .into_iter()
                .find(|p| starts_with(i, p))
            {
                let mut j = i + scheme.len();
                while j < n && is_url_char(chars[j]) {
                    j += 1;
                }
                push(collect(i, j), Some(ReprToken::Url));
                i = j;
                continue;
            }
        }
        // Strings: '...' / "..." (optionally b-prefixed), on one line, not after a word or `\`.
        let quote_at = if c == 'b' && matches!(chars.get(i + 1), Some('\'' | '"')) {
            Some(i + 1)
        } else if c == '\'' || c == '"' {
            Some(i)
        } else {
            None
        };
        if let Some(q) = quote_at.filter(|_| !prev_word && prev != Some('\\')) {
            let quote = chars[q];
            let mut j = q + 1;
            let mut closed = false;
            while j < n && chars[j] != '\n' {
                if chars[j] == '\\' {
                    j += 2;
                    continue;
                }
                if chars[j] == quote {
                    closed = true;
                    break;
                }
                j += 1;
            }
            if closed {
                push(collect(i, j + 1), Some(ReprToken::Str));
                i = j + 1;
                continue;
            }
        }
        if !prev_word {
            if let Some(len) = uuid_at(&chars, i) {
                push(collect(i, i + len), Some(ReprToken::Uuid));
                i += len;
                continue;
            }
            if let Some(len) = ipv4_at(&chars, i).filter(|_| prev != Some('.')) {
                push(collect(i, i + len), Some(ReprToken::Ipv4));
                i += len;
                continue;
            }
        }
        // Numbers: 0x.., -?digits(.digits)?(e[-+]?digits)?
        let starts_number =
            c.is_ascii_digit() || (c == '-' && chars.get(i + 1).is_some_and(char::is_ascii_digit));
        if starts_number && !prev_word {
            let mut j = i + 1;
            if c == '0' && chars.get(j) == Some(&'x') {
                j += 1;
                while j < n && is_hex(chars[j]) {
                    j += 1;
                }
            } else {
                while j < n && chars[j].is_ascii_digit() {
                    j += 1;
                }
                if j + 1 < n && chars[j] == '.' && chars[j + 1].is_ascii_digit() {
                    j += 1;
                    while j < n && chars[j].is_ascii_digit() {
                        j += 1;
                    }
                }
                if j < n && chars[j] == 'e' {
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
            }
            if j >= n || !is_word(chars[j]) {
                push(collect(i, j), Some(ReprToken::Number));
                i = j;
                continue;
            }
        }
        // Paths: /a/b/ (path) + file.txt (filename), not after a word character.
        if c == '/' && !prev_word {
            let mut j = i;
            while j < n && (chars[j] == '/' || is_path_char(chars[j])) {
                j += 1;
            }
            let last_slash = (i..j).rev().find(|k| chars[*k] == '/').unwrap_or(i);
            push(collect(i, last_slash + 1), Some(ReprToken::Path));
            if last_slash + 1 < j {
                push(collect(last_slash + 1, j), Some(ReprToken::Filename));
            }
            i = j;
            continue;
        }
        // Tags: <name ...>.
        if c == '<' && !in_tag && chars[i + 1..].contains(&'>') {
            let mut j = i + 1;
            while j < n && (is_word(chars[j]) || matches!(chars[j], '-' | '.' | ':' | '|')) {
                j += 1;
            }
            push("<".into(), Some(ReprToken::Tag));
            if j > i + 1 {
                push(collect(i + 1, j), Some(ReprToken::TagName));
            }
            in_tag = true;
            i = j;
            continue;
        }
        if c == '>' && in_tag {
            push(">".into(), Some(ReprToken::Tag));
            in_tag = false;
            i += 1;
            continue;
        }
        // Words: constants, calls (`a.b(`), attributes (`name=value`).
        if is_word(c) && !prev_word {
            let mut j = i + 1;
            while j < n
                && (is_word(chars[j])
                    || (chars[j] == '.' && chars.get(j + 1).copied().is_some_and(is_word)))
            {
                j += 1;
            }
            let word = collect(i, j);
            let next = chars.get(j).copied();
            let tok = match word.as_str() {
                "True" | "true" => Some(ReprToken::BoolTrue),
                "False" | "false" => Some(ReprToken::BoolFalse),
                "None" => Some(ReprToken::None),
                _ if next == Some('(') => Some(ReprToken::Call),
                _ if next == Some('=') && chars.get(j + 1) != Some(&'=') && !word.contains('.') => {
                    Some(ReprToken::AttribName)
                }
                _ => None,
            };
            if tok.is_none() && word.contains('.') {
                // Not a call: only the first word is consumed (`a.b` may be a filename etc.).
                let end = (i..j).find(|k| chars[*k] == '.').unwrap_or(j);
                push(collect(i, end), None);
                i = end;
                continue;
            }
            push(word, tok);
            i = j;
            if tok == Some(ReprToken::AttribName) {
                push("=".into(), None);
                i += 1;
                // A bare word value; quoted / numeric values are tokenized normally.
                if chars
                    .get(i)
                    .copied()
                    .is_some_and(|c| is_word(c) && !c.is_ascii_digit())
                {
                    let mut k = i;
                    while k < n && is_word(chars[k]) {
                        k += 1;
                    }
                    let value = collect(i, k);
                    let constant =
                        matches!(value.as_str(), "True" | "true" | "False" | "false" | "None");
                    if !constant {
                        push(value, Some(ReprToken::AttribValue));
                        i = k;
                    }
                }
            }
            continue;
        }
        if matches!(c, '[' | ']' | '{' | '}' | '(' | ')') {
            push(c.to_string(), Some(ReprToken::Brace));
            i += 1;
            continue;
        }
        if starts_with(i, "...") {
            push("...".into(), Some(ReprToken::Ellipsis));
            i += 3;
            continue;
        }
        push(c.to_string(), None);
        i += 1;
    }
    out
}

/// Apply [`tokenize_text`] highlighting over `text` (keeping its existing styles underneath),
/// with `style_for` giving each token's style (e.g. `|t| repr_style(t, dark)`).
pub fn highlight_text(text: &Text, style_for: &dyn Fn(ReprToken) -> Style) -> Text {
    let plain = text.plain_text();
    // Token style per run of characters.
    let mut runs: Vec<(usize, Option<Style>)> = tokenize_text(&plain)
        .into_iter()
        .map(|(s, t)| (s.chars().count(), t.map(style_for)))
        .collect();
    runs.reverse();
    let mut out = Text::new();
    let mut current = runs.pop();
    for span in &text.spans {
        let mut chars = span.text.chars().peekable();
        while chars.peek().is_some() {
            let Some((left, tok_style)) = current.as_mut() else {
                out.push(chars.collect::<String>(), span.style.clone());
                break;
            };
            let chunk: String = chars.by_ref().take(*left).collect();
            *left -= chunk.chars().count();
            let style = match tok_style {
                Some(t) => span.style.combine(t),
                None => span.style.clone(),
            };
            out.push(chunk, style);
            if *left == 0 {
                current = runs.pop();
            }
        }
    }
    out
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

    fn text_toks(s: &str) -> Vec<(String, ReprToken)> {
        tokenize_text(s)
            .into_iter()
            .filter_map(|(s, t)| t.map(|t| (s, t)))
            .collect()
    }

    #[test]
    fn tokenizes_text_like_rich_repr_highlighter() {
        use ReprToken::*;
        let got = text_toks(
            "GET https://x.io/a?b=1 from 10.0.0.1 took 1.5e3 ms: ok=True err=None \
             mode=fast path=/var/log/app.log id=123e4567-e89b-12d3-a456-426614174000",
        );
        let got: Vec<(&str, ReprToken)> = got.iter().map(|(s, t)| (s.as_str(), *t)).collect();
        assert_eq!(
            got,
            vec![
                ("https://x.io/a?b=1", Url),
                ("10.0.0.1", Ipv4),
                ("1.5e3", Number),
                ("ok", AttribName),
                ("True", BoolTrue),
                ("err", AttribName),
                ("None", ReprToken::None),
                ("mode", AttribName),
                ("fast", AttribValue),
                ("path", AttribName),
                ("/var/log/", Path),
                ("app.log", Filename),
                ("id", AttribName),
                ("123e4567-e89b-12d3-a456-426614174000", Uuid),
            ]
        );
        let got = text_toks("foo.bar(1, 'it''s', \"q\\\"\") <Tag x=0x1f> ... don't");
        let got: Vec<(&str, ReprToken)> = got.iter().map(|(s, t)| (s.as_str(), *t)).collect();
        assert_eq!(
            got,
            vec![
                ("foo.bar", Call),
                ("(", Brace),
                ("1", Number),
                ("'it'", Str),
                ("'s'", Str),
                ("\"q\\\"\"", Str),
                (")", Brace),
                ("<", Tag),
                ("Tag", TagName),
                ("x", AttribName),
                ("0x1f", Number),
                (">", Tag),
                ("...", Ellipsis),
            ]
        );
        assert!(text_toks("abc123 x1 a.b don't").is_empty());
        // Round trip: tokens cover the whole input.
        let s = "a=1 /x/ \"unterminated";
        let joined: String = tokenize_text(s).into_iter().map(|(s, _)| s).collect();
        assert_eq!(joined, s);
    }

    #[test]
    fn highlight_text_keeps_existing_styles() {
        let mut t = Text::new();
        t.push("x=", Style::new().italic());
        t.push("12 y", Style::new().underline());
        let h = highlight_text(&t, &|tok| repr_style(tok, true));
        assert_eq!(h.plain_text(), "x=12 y");
        let num = h.spans.iter().find(|s| s.text == "12").unwrap();
        assert!(
            num.style.underline == Some(true) && num.style.bold == Some(true),
            "{:?}",
            num.style
        );
        let name = h.spans.iter().find(|s| s.text == "x").unwrap();
        assert_eq!(name.style.italic, Some(true));
        assert_eq!(name.style.fg, repr_style(ReprToken::AttribName, true).fg);
    }
}

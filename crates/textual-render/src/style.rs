//! Span styles (like `rich.style.Style`): optional colors and attributes that combine.

use crate::color::Color;
use std::fmt;
use std::sync::Arc;

/// Resolved text attribute bit flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Attrs(pub u8);

impl Attrs {
    pub const NONE: Attrs = Attrs(0);
    pub const BOLD: Attrs = Attrs(1);
    pub const DIM: Attrs = Attrs(2);
    pub const ITALIC: Attrs = Attrs(4);
    pub const UNDERLINE: Attrs = Attrs(8);
    pub const STRIKE: Attrs = Attrs(16);
    pub const REVERSE: Attrs = Attrs(32);
    pub const BLINK: Attrs = Attrs(64);

    pub fn contains(&self, o: Attrs) -> bool {
        self.0 & o.0 == o.0
    }
    pub fn insert(&mut self, o: Attrs) {
        self.0 |= o.0
    }
    pub fn remove(&mut self, o: Attrs) {
        self.0 &= !o.0
    }
    pub fn union(self, o: Attrs) -> Attrs {
        Attrs(self.0 | o.0)
    }
    pub fn is_empty(&self) -> bool {
        self.0 == 0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StyleParseError(pub String);

impl fmt::Display for StyleParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid style: {}", self.0)
    }
}

impl std::error::Error for StyleParseError {}

/// A style where every field is optional; `None` means "inherit from whatever is below".
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Style {
    pub fg: Option<Color>,
    pub bg: Option<Color>,
    pub bold: Option<bool>,
    pub dim: Option<bool>,
    pub italic: Option<bool>,
    pub underline: Option<bool>,
    pub strike: Option<bool>,
    pub reverse: Option<bool>,
    pub blink: Option<bool>,
    /// Link / action metadata (e.g. from markup `[@click=app.bell]`).
    pub link: Option<Arc<str>>,
}

const ATTR_NAMES: [&str; 7] = [
    "bold",
    "dim",
    "italic",
    "underline",
    "strike",
    "reverse",
    "blink",
];

fn is_percent(word: &str) -> bool {
    word.len() > 1
        && word.ends_with('%')
        && word[..word.len() - 1]
            .chars()
            .all(|c| c.is_ascii_digit() || c == '.')
}

/// Parse a color word: `$var` (looked up without the `$`) or anything [`Color::parse`] accepts.
fn parse_color_word(word: &str, vars: &dyn Fn(&str) -> Option<Color>) -> Result<Color, String> {
    if let Some(name) = word.strip_prefix('$') {
        vars(name).ok_or_else(|| format!("unknown variable ${name}"))
    } else {
        Color::parse(word).map_err(|_| format!("unknown style word {word:?}"))
    }
}

/// Split a style string on whitespace, keeping bracketed groups (`rgb(1, 2, 3)`) and
/// quoted strings together.
pub(crate) fn split_style_words(s: &str) -> Vec<&str> {
    let mut words = Vec::new();
    let mut depth = 0i32;
    let mut quote: Option<char> = None;
    let mut start: Option<usize> = None;
    for (i, c) in s.char_indices() {
        if let Some(q) = quote {
            if c == q {
                quote = None;
            }
            continue;
        }
        match c {
            '\'' | '"' => {
                quote = Some(c);
                start.get_or_insert(i);
            }
            '(' | '[' | '{' => {
                depth += 1;
                start.get_or_insert(i);
            }
            ')' | ']' | '}' => {
                depth = (depth - 1).max(0);
                start.get_or_insert(i);
            }
            c if c.is_whitespace() && depth == 0 => {
                if let Some(st) = start.take() {
                    words.push(&s[st..i]);
                }
            }
            _ => {
                start.get_or_insert(i);
            }
        }
    }
    if let Some(st) = start {
        words.push(&s[st..]);
    }
    words
}

impl Style {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn fg(mut self, c: Color) -> Self {
        self.fg = Some(c);
        self
    }
    pub fn bg(mut self, c: Color) -> Self {
        self.bg = Some(c);
        self
    }
    pub fn bold(mut self) -> Self {
        self.bold = Some(true);
        self
    }
    pub fn dim(mut self) -> Self {
        self.dim = Some(true);
        self
    }
    pub fn italic(mut self) -> Self {
        self.italic = Some(true);
        self
    }
    pub fn underline(mut self) -> Self {
        self.underline = Some(true);
        self
    }
    pub fn strike(mut self) -> Self {
        self.strike = Some(true);
        self
    }
    pub fn reverse(mut self) -> Self {
        self.reverse = Some(true);
        self
    }
    pub fn blink(mut self) -> Self {
        self.blink = Some(true);
        self
    }
    pub fn link(mut self, link: impl Into<Arc<str>>) -> Self {
        self.link = Some(link.into());
        self
    }

    /// Parse a rich-like style string: `"bold red on #000033 not italic underline"`.
    /// Words: attribute names (b/bold, d/dim, i/italic, u/underline, s/strike, r/reverse, blink),
    /// `not <attr>`, a color, `on <color>`, `none`. Colors use [`Color::parse`] and `$name`
    /// is looked up with `vars` (the name is passed without the `$`).
    /// A percentage after a color (`red 50%`, `on $panel 30%`) multiplies that color's alpha.
    /// An empty string parses to the empty style.
    pub fn parse_with(
        s: &str,
        vars: &dyn Fn(&str) -> Option<Color>,
    ) -> Result<Style, StyleParseError> {
        let err = |msg: String| StyleParseError(format!("{s:?}: {msg}"));
        let mut style = Style::default();
        // Which color a trailing `NN%` applies to: Some(false) = fg, Some(true) = bg.
        let mut last_color_is_bg: Option<bool> = None;
        let mut iter = split_style_words(s).into_iter();
        while let Some(word) = iter.next() {
            let lower = word.to_ascii_lowercase();
            match lower.as_str() {
                "not" => {
                    let next = iter
                        .next()
                        .ok_or_else(|| err("expected attribute after 'not'".into()))?;
                    if !style.set_attr(&next.to_ascii_lowercase(), false) {
                        return Err(err(format!("unknown attribute {next:?}")));
                    }
                }
                "on" => {
                    let next = iter
                        .next()
                        .ok_or_else(|| err("expected color after 'on'".into()))?;
                    style.bg = Some(parse_color_word(next, vars).map_err(err)?);
                    last_color_is_bg = Some(true);
                }
                "none" => {
                    for attr in ATTR_NAMES {
                        style.set_attr(attr, false);
                    }
                }
                _ if is_percent(&lower) => {
                    let pct: f32 = lower[..lower.len() - 1]
                        .parse()
                        .map_err(|_| err(format!("bad percentage {word:?}")))?;
                    let factor = (pct / 100.0).clamp(0.0, 1.0);
                    match last_color_is_bg {
                        Some(true) => style.bg = style.bg.map(|c| c.multiply_alpha(factor)),
                        Some(false) => style.fg = style.fg.map(|c| c.multiply_alpha(factor)),
                        None => {}
                    }
                }
                _ => {
                    if !style.set_attr(&lower, true) {
                        style.fg = Some(parse_color_word(word, vars).map_err(err)?);
                        last_color_is_bg = Some(false);
                    }
                }
            }
        }
        Ok(style)
    }

    pub fn parse(s: &str) -> Result<Style, StyleParseError> {
        Self::parse_with(s, &|_| None)
    }

    /// Combine with `over` taking precedence. Colors in `over` with alpha < 1 are
    /// composited over the corresponding color in `self` (when present).
    pub fn combine(&self, over: &Style) -> Style {
        fn color(under: Option<Color>, over: Option<Color>) -> Option<Color> {
            match (under, over) {
                (Some(u), Some(o)) if o.a < 1.0 => Some(u.composite(o)),
                (_, Some(o)) => Some(o),
                (u, None) => u,
            }
        }
        Style {
            fg: color(self.fg, over.fg),
            bg: color(self.bg, over.bg),
            bold: over.bold.or(self.bold),
            dim: over.dim.or(self.dim),
            italic: over.italic.or(self.italic),
            underline: over.underline.or(self.underline),
            strike: over.strike.or(self.strike),
            reverse: over.reverse.or(self.reverse),
            blink: over.blink.or(self.blink),
            link: over.link.clone().or_else(|| self.link.clone()),
        }
    }

    /// Resolve attribute flags (`None` → off).
    pub fn attrs(&self) -> Attrs {
        let mut a = Attrs::NONE;
        for (flag, value) in [
            (Attrs::BOLD, self.bold),
            (Attrs::DIM, self.dim),
            (Attrs::ITALIC, self.italic),
            (Attrs::UNDERLINE, self.underline),
            (Attrs::STRIKE, self.strike),
            (Attrs::REVERSE, self.reverse),
            (Attrs::BLINK, self.blink),
        ] {
            if value == Some(true) {
                a.insert(flag);
            }
        }
        a
    }

    pub fn is_empty(&self) -> bool {
        *self == Style::default()
    }

    /// Set an attribute by (lowercase) name or abbreviation. Returns false if unknown.
    fn set_attr(&mut self, name: &str, value: bool) -> bool {
        let slot = match name {
            "b" | "bold" => &mut self.bold,
            "d" | "dim" => &mut self.dim,
            "i" | "italic" => &mut self.italic,
            // Textual's `underline2`/`uu` (double underline) maps to plain underline here.
            "u" | "underline" | "uu" | "underline2" => &mut self.underline,
            "s" | "strike" | "strikethrough" => &mut self.strike,
            "r" | "reverse" => &mut self.reverse,
            "blink" => &mut self.blink,
            _ => return false,
        };
        *slot = Some(value);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_attrs() {
        let s = Style::parse("bold i u s r d blink").unwrap();
        assert_eq!(s.bold, Some(true));
        assert_eq!(s.italic, Some(true));
        assert_eq!(s.underline, Some(true));
        assert_eq!(s.strike, Some(true));
        assert_eq!(s.reverse, Some(true));
        assert_eq!(s.dim, Some(true));
        assert_eq!(s.blink, Some(true));
        assert_eq!(s.fg, None);
        assert_eq!(s.bg, None);
    }

    #[test]
    fn parse_colors() {
        let s = Style::parse("bold red on #000033 not italic underline").unwrap();
        assert_eq!(s.fg, Some(Color::parse("red").unwrap()));
        assert_eq!(s.bg, Some(Color::rgb(0, 0, 0x33)));
        assert_eq!(s.bold, Some(true));
        assert_eq!(s.italic, Some(false));
        assert_eq!(s.underline, Some(true));
    }

    #[test]
    fn parse_rgb_with_spaces() {
        let s = Style::parse("rgb(1, 2, 3) on rgba(4, 5, 6, 0.5)").unwrap();
        assert_eq!(s.fg, Some(Color::rgb(1, 2, 3)));
        assert_eq!(s.bg, Some(Color::rgba(4, 5, 6, 0.5)));
    }

    #[test]
    fn parse_none_and_empty() {
        let s = Style::parse("none").unwrap();
        assert_eq!(s.bold, Some(false));
        assert_eq!(s.blink, Some(false));
        assert!(s.attrs().is_empty());
        assert!(Style::parse("").unwrap().is_empty());
        assert!(Style::parse("   ").unwrap().is_empty());
    }

    #[test]
    fn parse_vars_and_alpha() {
        let vars = |n: &str| (n == "panel").then_some(Color::rgb(10, 20, 30));
        let s = Style::parse_with("$panel 50% on $panel 25%", &vars).unwrap();
        assert_eq!(s.fg, Some(Color::rgba(10, 20, 30, 0.5)));
        assert_eq!(s.bg, Some(Color::rgba(10, 20, 30, 0.25)));
        assert!(Style::parse_with("$nope", &vars).is_err());
        assert!(Style::parse("$panel").is_err());
        // percentage with no color is ignored
        assert!(Style::parse("bold 50%").unwrap().fg.is_none());
    }

    #[test]
    fn parse_errors() {
        assert!(Style::parse("frobnicate").is_err());
        assert!(Style::parse("on").is_err());
        assert!(Style::parse("not").is_err());
        assert!(Style::parse("not red").is_err());
        assert!(Style::parse("on bold").is_err());
    }

    #[test]
    fn parse_case_insensitive() {
        let s = Style::parse("BOLD Red").unwrap();
        assert_eq!(s.bold, Some(true));
        assert_eq!(s.fg, Some(Color::parse("red").unwrap()));
    }

    #[test]
    fn combine_precedence() {
        let a = Style::new().fg(Color::rgb(1, 1, 1)).bold();
        let b = Style {
            bold: Some(false),
            italic: Some(true),
            ..Style::default()
        };
        let c = a.combine(&b);
        assert_eq!(c.fg, Some(Color::rgb(1, 1, 1)));
        assert_eq!(c.bold, Some(false));
        assert_eq!(c.italic, Some(true));
        let d = a.combine(&Style::new().fg(Color::rgb(9, 9, 9)));
        assert_eq!(d.fg, Some(Color::rgb(9, 9, 9)));
    }

    #[test]
    fn combine_alpha_composites() {
        let under = Style::new().bg(Color::rgb(0, 0, 0)).fg(Color::rgb(0, 0, 0));
        let over = Style::new()
            .bg(Color::rgba(200, 100, 0, 0.5))
            .fg(Color::rgba(255, 255, 255, 0.5));
        let c = under.combine(&over);
        assert_eq!(c.bg, Some(Color::rgb(100, 50, 0)));
        assert_eq!(c.fg.unwrap().a, 1.0);
        // nothing underneath: taken as-is
        let c2 = Style::new().combine(&over);
        assert_eq!(c2.bg, Some(Color::rgba(200, 100, 0, 0.5)));
    }

    #[test]
    fn combine_link() {
        let a = Style::new().link("a");
        assert_eq!(a.combine(&Style::new()).link.as_deref(), Some("a"));
        assert_eq!(
            a.combine(&Style::new().link("b")).link.as_deref(),
            Some("b")
        );
        assert_eq!(Style::new().combine(&a).link.as_deref(), Some("a"));
    }

    #[test]
    fn attrs_resolve() {
        let s = Style {
            bold: Some(true),
            italic: Some(false),
            reverse: Some(true),
            ..Style::default()
        };
        let a = s.attrs();
        assert!(a.contains(Attrs::BOLD));
        assert!(a.contains(Attrs::REVERSE));
        assert!(!a.contains(Attrs::ITALIC));
        assert_eq!(a, Attrs::BOLD.union(Attrs::REVERSE));
    }

    #[test]
    fn split_words() {
        assert_eq!(
            split_style_words("  a  rgb(1, 2) 'x y' b"),
            vec!["a", "rgb(1, 2)", "'x y'", "b"]
        );
    }
}

//! `TextAreaTheme` (port of `textual/_text_area_theme.py`).
//!
//! A theme supplies styles for the editor chrome (cursor, gutter, selection, ...) and for
//! syntax highlight names. Any chrome style left as `None` falls back to the corresponding
//! component class (`text-area--cursor`, ...), which is how the default `css` theme works.

use std::collections::HashMap;
use textual_render::{Color, Style};

/// Styles for a [`super::TextArea`].
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TextAreaTheme {
    pub name: String,
    /// Foreground / background of the whole widget (`None` = use CSS).
    pub base_style: Option<Style>,
    pub gutter_style: Option<Style>,
    pub cursor_style: Option<Style>,
    pub cursor_line_style: Option<Style>,
    pub cursor_line_gutter_style: Option<Style>,
    pub bracket_matching_style: Option<Style>,
    pub selection_style: Option<Style>,
    /// Highlight name (e.g. `"keyword"`, `"string"`) → style.
    pub syntax_styles: HashMap<String, Style>,
}

fn hex(s: &str) -> Color {
    Color::parse(s).unwrap_or(Color::WHITE)
}

fn fg(c: &str) -> Style {
    Style::new().fg(hex(c))
}

fn bg(c: &str) -> Style {
    Style::new().bg(hex(c))
}

fn fg_bg(f: &str, b: &str) -> Style {
    Style::new().fg(hex(f)).bg(hex(b))
}

fn syntax(entries: &[(&str, Style)]) -> HashMap<String, Style> {
    entries
        .iter()
        .map(|(k, v)| (k.to_string(), v.clone()))
        .collect()
}

impl TextAreaTheme {
    /// A theme with only a name (everything from CSS, no syntax styles).
    pub fn new(name: impl Into<String>) -> Self {
        TextAreaTheme {
            name: name.into(),
            ..Default::default()
        }
    }

    /// Style for a highlight name, falling back to its parent name (`"keyword.function"` →
    /// `"keyword"`).
    pub fn get_highlight(&self, name: &str) -> Option<&Style> {
        let mut name = name;
        loop {
            if let Some(s) = self.syntax_styles.get(name) {
                return Some(s);
            }
            name = &name[..name.rfind('.')?];
        }
    }

    /// Names of the built-in themes.
    pub fn builtin_names() -> &'static [&'static str] {
        &["css", "monokai", "dracula", "vscode_dark", "github_light"]
    }

    /// All built-in themes.
    pub fn builtin_themes() -> Vec<TextAreaTheme> {
        Self::builtin_names()
            .iter()
            .filter_map(|n| Self::get_builtin_theme(n))
            .collect()
    }

    /// A built-in theme by name.
    pub fn get_builtin_theme(name: &str) -> Option<TextAreaTheme> {
        Some(match name {
            "css" => TextAreaTheme {
                name: "css".into(),
                syntax_styles: vscode_dark_syntax(),
                ..Default::default()
            },
            "monokai" => TextAreaTheme {
                name: "monokai".into(),
                base_style: Some(fg_bg("#f8f8f2", "#272822")),
                gutter_style: Some(fg_bg("#90908a", "#272822")),
                cursor_style: Some(fg_bg("#272822", "#f8f8f0")),
                cursor_line_style: Some(bg("#3e3d32")),
                cursor_line_gutter_style: Some(fg_bg("#c2c2bf", "#3e3d32")),
                bracket_matching_style: Some(bg("#838889").bold()),
                selection_style: Some(bg("#65686a")),
                syntax_styles: syntax(&[
                    ("string", fg("#E6DB74")),
                    ("comment", fg("#75715E")),
                    ("keyword", fg("#F92672")),
                    ("operator", fg("#f8f8f2")),
                    ("number", fg("#AE81FF")),
                    ("type", fg("#A6E22E")),
                    ("type.builtin", fg("#F92672")),
                    ("function", fg("#A6E22E")),
                    ("boolean", fg("#66D9EF").italic()),
                    ("constant.builtin", fg("#66D9EF").italic()),
                    ("json.label", fg("#F92672").bold()),
                    ("punctuation.bracket", fg("#f8f8f2")),
                ]),
            },
            "dracula" => TextAreaTheme {
                name: "dracula".into(),
                base_style: Some(fg_bg("#f8f8f2", "#1E1F35")),
                gutter_style: Some(fg("#6272a4")),
                cursor_style: Some(fg_bg("#282a36", "#f8f8f0")),
                cursor_line_style: Some(bg("#282b45")),
                cursor_line_gutter_style: Some(fg_bg("#c2c2bf", "#282b45").bold()),
                bracket_matching_style: Some(bg("#99999d").bold().underline()),
                selection_style: Some(bg("#44475A")),
                syntax_styles: syntax(&[
                    ("string", fg("#f1fa8c")),
                    ("comment", fg("#6272a4")),
                    ("keyword", fg("#ff79c6")),
                    ("operator", fg("#f8f8f2")),
                    ("number", fg("#bd93f9")),
                    ("type", fg("#ff79c6")),
                    ("type.builtin", fg("#bd93f9")),
                    ("function", fg("#50fa7b")),
                    ("boolean", fg("#50fa7b")),
                    ("constant.builtin", fg("#bd93f9")),
                    ("json.label", fg("#ff79c6").bold()),
                    ("punctuation.bracket", fg("#f8f8f2")),
                ]),
            },
            "vscode_dark" => TextAreaTheme {
                name: "vscode_dark".into(),
                base_style: Some(fg_bg("#CCCCCC", "#1F1F1F")),
                gutter_style: Some(fg_bg("#6E7681", "#1F1F1F")),
                cursor_style: Some(fg_bg("#1e1e1e", "#f0f0f0")),
                cursor_line_style: Some(bg("#2b2b2b")),
                cursor_line_gutter_style: Some(fg_bg("#CCCCCC", "#2b2b2b")),
                bracket_matching_style: Some(bg("#3a3a3a").bold()),
                selection_style: Some(bg("#264F78")),
                syntax_styles: vscode_dark_syntax(),
            },
            "github_light" => TextAreaTheme {
                name: "github_light".into(),
                base_style: Some(fg_bg("#24292e", "#f0f0f0")),
                gutter_style: Some(fg_bg("#BBBBBB", "#f0f0f0")),
                cursor_style: Some(fg_bg("#fafbfc", "#24292e")),
                cursor_line_style: Some(bg("#ebebeb")),
                cursor_line_gutter_style: Some(fg_bg("#A4A4A4", "#ebebeb")),
                bracket_matching_style: Some(fg("#24292e").underline()),
                selection_style: Some(bg("#c8c8fa")),
                syntax_styles: syntax(&[
                    ("string", fg("#093069")),
                    ("comment", fg("#6a737d")),
                    ("keyword", fg("#d73a49")),
                    ("operator", fg("#0450AE")),
                    ("number", fg("#d73a49")),
                    ("type", fg("#A4A4A4")),
                    ("type.builtin", fg("#7DAF9C")),
                    ("function", fg("#6639BB")),
                    ("boolean", fg("#7DAF9C")),
                    ("constant.builtin", fg("#7DAF9C")),
                    ("json.label", fg("#6639BB")),
                    ("punctuation.bracket", fg("#24292e")),
                ]),
            },
            _ => return None,
        })
    }
}

fn vscode_dark_syntax() -> HashMap<String, Style> {
    syntax(&[
        ("string", fg("#ce9178")),
        ("comment", fg("#6A9955")),
        ("keyword", fg("#C586C0")),
        ("operator", fg("#CCCCCC")),
        ("number", fg("#b5cea8")),
        ("type", fg("#EFCB43")),
        ("type.builtin", fg("#9CDCFE")),
        ("function", fg("#DCDCAA")),
        ("boolean", fg("#7DAF9C")),
        ("constant.builtin", fg("#7DAF9C")),
        ("json.label", fg("#569cd6").bold()),
        ("punctuation.bracket", fg("#CCCCCC")),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtins_and_fallback_names() {
        assert_eq!(TextAreaTheme::builtin_themes().len(), 5);
        let t = TextAreaTheme::get_builtin_theme("monokai").unwrap();
        assert!(t.get_highlight("keyword.function").is_some());
        assert!(t.get_highlight("nope").is_none());
        assert!(TextAreaTheme::get_builtin_theme("css")
            .unwrap()
            .cursor_style
            .is_none());
    }
}

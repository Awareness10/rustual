//! Pluggable syntax highlighting.
//!
//! Textual highlights with tree-sitter; this port does not depend on tree-sitter. Instead a
//! [`Highlighter`] returns highlight spans for a single line, and a small built-in lexical
//! highlighter ([`KeywordHighlighter`]) covers keywords, strings, numbers, comments, constants,
//! builtin types and function calls for a few languages. Constructs spanning several lines
//! (block comments, multi-line strings) are not tracked.

use super::document::is_word_char;
use std::rc::Rc;

/// A highlighted character range within a line, with a highlight name such as `"keyword"`
/// (looked up in the [`super::TextAreaTheme`]'s `syntax_styles`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Highlight {
    /// Start column (character index).
    pub start: usize,
    /// End column (exclusive).
    pub end: usize,
    pub name: String,
}

impl Highlight {
    pub fn new(start: usize, end: usize, name: impl Into<String>) -> Self {
        Highlight {
            start,
            end,
            name: name.into(),
        }
    }
}

/// Produces highlights for one line at a time. Only visible lines are highlighted.
pub trait Highlighter {
    fn highlight_line(&self, line_index: usize, line: &str) -> Vec<Highlight>;
}

/// A simple lexical highlighter driven by word lists.
#[derive(Debug, Clone, Default)]
pub struct KeywordHighlighter {
    pub keywords: Vec<&'static str>,
    /// `true`, `None`, ... (`constant.builtin`).
    pub constants: Vec<&'static str>,
    /// Builtin types (`type.builtin`).
    pub types: Vec<&'static str>,
    /// Line comment prefixes (`#`, `//`).
    pub line_comments: Vec<&'static str>,
    /// String delimiters.
    pub quotes: Vec<char>,
    /// Strings immediately followed by `:` are `json.label`.
    pub json_labels: bool,
}

/// Names of languages with a built-in highlighter.
pub const BUILTIN_LANGUAGES: &[&str] = &["python", "rust", "javascript", "json", "toml"];

impl KeywordHighlighter {
    /// The built-in highlighter for `language`, if any.
    pub fn for_language(language: &str) -> Option<KeywordHighlighter> {
        let h = match language {
            "python" => KeywordHighlighter {
                keywords: vec![
                    "and", "as", "assert", "async", "await", "break", "class", "continue", "def",
                    "del", "elif", "else", "except", "finally", "for", "from", "global", "if",
                    "import", "in", "is", "lambda", "nonlocal", "not", "or", "pass", "raise",
                    "return", "try", "while", "with", "yield", "match", "case",
                ],
                constants: vec!["True", "False", "None", "self"],
                types: vec![
                    "int", "str", "float", "bool", "list", "dict", "set", "tuple", "bytes",
                    "object",
                ],
                line_comments: vec!["#"],
                quotes: vec!['"', '\''],
                json_labels: false,
            },
            "rust" => KeywordHighlighter {
                keywords: vec![
                    "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else",
                    "enum", "extern", "fn", "for", "if", "impl", "in", "let", "loop", "match",
                    "mod", "move", "mut", "pub", "ref", "return", "static", "struct", "super",
                    "trait", "type", "unsafe", "use", "where", "while",
                ],
                constants: vec!["true", "false", "self", "Self", "None", "Some", "Ok", "Err"],
                types: vec![
                    "i8", "i16", "i32", "i64", "i128", "isize", "u8", "u16", "u32", "u64", "u128",
                    "usize", "f32", "f64", "bool", "char", "str", "String", "Vec", "Option",
                    "Result", "Box",
                ],
                line_comments: vec!["//"],
                quotes: vec!['"'],
                json_labels: false,
            },
            "javascript" => KeywordHighlighter {
                keywords: vec![
                    "async",
                    "await",
                    "break",
                    "case",
                    "catch",
                    "class",
                    "const",
                    "continue",
                    "default",
                    "delete",
                    "do",
                    "else",
                    "export",
                    "extends",
                    "finally",
                    "for",
                    "function",
                    "if",
                    "import",
                    "in",
                    "instanceof",
                    "let",
                    "new",
                    "of",
                    "return",
                    "switch",
                    "throw",
                    "try",
                    "typeof",
                    "var",
                    "while",
                    "yield",
                ],
                constants: vec!["true", "false", "null", "undefined", "this"],
                types: vec![],
                line_comments: vec!["//"],
                quotes: vec!['"', '\'', '`'],
                json_labels: false,
            },
            "json" => KeywordHighlighter {
                constants: vec!["true", "false", "null"],
                quotes: vec!['"'],
                json_labels: true,
                ..Default::default()
            },
            "toml" => KeywordHighlighter {
                constants: vec!["true", "false"],
                line_comments: vec!["#"],
                quotes: vec!['"', '\''],
                ..Default::default()
            },
            _ => return None,
        };
        Some(h)
    }

    /// Shared handle to the built-in highlighter for `language`.
    pub fn shared(language: &str) -> Option<Rc<dyn Highlighter>> {
        Self::for_language(language).map(|h| Rc::new(h) as Rc<dyn Highlighter>)
    }
}

impl Highlighter for KeywordHighlighter {
    fn highlight_line(&self, _line_index: usize, line: &str) -> Vec<Highlight> {
        let chars: Vec<char> = line.chars().collect();
        let n = chars.len();
        let mut out = Vec::new();
        let mut i = 0;
        let starts_with = |i: usize, p: &str| {
            p.chars()
                .enumerate()
                .all(|(j, pc)| i + j < n && chars[i + j] == pc)
        };
        while i < n {
            let c = chars[i];
            if self.line_comments.iter().any(|p| starts_with(i, p)) {
                out.push(Highlight::new(i, n, "comment"));
                break;
            }
            if self.quotes.contains(&c) {
                let start = i;
                i += 1;
                while i < n && chars[i] != c {
                    if chars[i] == '\\' {
                        i += 1;
                    }
                    i += 1;
                }
                i = (i + 1).min(n);
                let mut j = i;
                while j < n && chars[j] == ' ' {
                    j += 1;
                }
                let name = if self.json_labels && j < n && chars[j] == ':' {
                    "json.label"
                } else {
                    "string"
                };
                out.push(Highlight::new(start, i, name));
                continue;
            }
            if c.is_ascii_digit() && (i == 0 || !is_word_char(chars[i - 1])) {
                let start = i;
                while i < n && (is_word_char(chars[i]) || chars[i] == '.') {
                    i += 1;
                }
                out.push(Highlight::new(start, i, "number"));
                continue;
            }
            if is_word_char(c) {
                let start = i;
                while i < n && is_word_char(chars[i]) {
                    i += 1;
                }
                let word: String = chars[start..i].iter().collect();
                let w = word.as_str();
                let name = if self.keywords.contains(&w) {
                    Some("keyword")
                } else if self.constants.contains(&w) {
                    Some("constant.builtin")
                } else if self.types.contains(&w) {
                    Some("type.builtin")
                } else if i < n && chars[i] == '(' {
                    Some("function")
                } else {
                    None
                };
                if let Some(name) = name {
                    out.push(Highlight::new(start, i, name));
                }
                continue;
            }
            i += 1;
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn python_line() {
        let h = KeywordHighlighter::for_language("python").unwrap();
        let hl = h.highlight_line(0, "def f(x): return 'a#b' # done");
        let names: Vec<(usize, usize, &str)> = hl
            .iter()
            .map(|h| (h.start, h.end, h.name.as_str()))
            .collect();
        assert_eq!(
            names,
            vec![
                (0, 3, "keyword"),
                (4, 5, "function"),
                (10, 16, "keyword"),
                (17, 22, "string"),
                (23, 29, "comment"),
            ]
        );
    }

    #[test]
    fn json_labels_and_numbers() {
        let h = KeywordHighlighter::for_language("json").unwrap();
        let hl = h.highlight_line(0, r#"{"a": 12, "b": true}"#);
        let names: Vec<&str> = hl.iter().map(|h| h.name.as_str()).collect();
        assert_eq!(
            names,
            vec!["json.label", "number", "json.label", "constant.builtin"]
        );
        assert!(KeywordHighlighter::for_language("cobol").is_none());
    }
}

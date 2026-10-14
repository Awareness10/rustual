//! TCSS tokenizer. Handles comments (`/* */`), selectors, `{` `}`, nesting with `&`,
//! declarations, `$var: value;` definitions and `$var` references.
//! Internal API: the parser is the only consumer; shape it as needed.
//!
//! The tokenizer is statement oriented: it produces one token per variable definition,
//! selector (block start), declaration and block end. Values are returned as raw text with
//! comments stripped; `$variable` substitution and value parsing happen in the parser.
//!
//! Like Textual (`textual/css/tokenize.py`), declarations are distinguished from nested
//! selectors by their shape: a declaration name starts with a lowercase letter and is
//! followed by `:`, whereas selectors start with an uppercase type name, `.`, `#`, `*` or `&`.
//! A `#` followed by whitespace at the start of a statement is a line comment (`# like this`).

use crate::model::CssError;

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    /// `$name: value;` (only valid at the top level). Value ends at `;` or newline.
    VariableDef { name: String, value: String },
    /// Selector text before a `{` (the `{` is consumed; a block is now open).
    BlockStart(String),
    /// `name: value` inside a block, terminated by `;` or `}`. Value includes `!important`.
    Declaration { name: String, value: String },
    /// `}` closing the innermost open block.
    BlockEnd,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    /// 1-based line of the first character of the token.
    pub line: usize,
    /// 1-based column of the first character of the token.
    pub column: usize,
}

struct Scanner<'a> {
    chars: Vec<char>,
    pos: usize,
    line: usize,
    column: usize,
    errors: &'a mut Vec<CssError>,
}

impl Scanner<'_> {
    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn peek_at(&self, offset: usize) -> Option<char> {
        self.chars.get(self.pos + offset).copied()
    }

    fn bump(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.pos += 1;
        if c == '\n' {
            self.line += 1;
            self.column = 1;
        } else {
            self.column += 1;
        }
        Some(c)
    }

    fn error(&mut self, message: impl Into<String>, line: usize, column: usize) {
        self.errors.push(CssError {
            message: message.into(),
            line,
            column,
        });
    }

    fn at_block_comment(&self) -> bool {
        self.peek() == Some('/') && self.peek_at(1) == Some('*')
    }

    /// Skip a `/* ... */` comment (cursor at `/`).
    fn skip_block_comment(&mut self) {
        let (line, column) = (self.line, self.column);
        self.bump();
        self.bump();
        loop {
            match self.peek() {
                None => {
                    self.error("unterminated comment", line, column);
                    return;
                }
                Some('*') if self.peek_at(1) == Some('/') => {
                    self.bump();
                    self.bump();
                    return;
                }
                _ => {
                    self.bump();
                }
            }
        }
    }

    fn skip_line(&mut self) {
        while let Some(c) = self.peek() {
            if c == '\n' {
                break;
            }
            self.bump();
        }
    }

    /// Skip whitespace, block comments and `# ` line comments between statements.
    fn skip_trivia(&mut self) {
        loop {
            match self.peek() {
                Some(c) if c.is_whitespace() => {
                    self.bump();
                }
                Some('/') if self.at_block_comment() => self.skip_block_comment(),
                Some('#') if self.peek_at(1).is_none_or(|c| c.is_whitespace()) => self.skip_line(),
                _ => return,
            }
        }
    }

    /// Does the text at the cursor look like `name:` (a declaration)?
    fn at_declaration(&self) -> bool {
        match self.peek() {
            Some(c) if c.is_ascii_lowercase() => {}
            _ => return false,
        }
        let mut i = 1;
        while let Some(c) = self.peek_at(i) {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                i += 1;
            } else {
                break;
            }
        }
        while let Some(c) = self.peek_at(i) {
            if c == ' ' || c == '\t' {
                i += 1;
            } else {
                break;
            }
        }
        self.peek_at(i) == Some(':')
    }

    /// Does the text at the cursor look like `$name:` (a variable definition)?
    fn at_variable_def(&self) -> bool {
        if self.peek() != Some('$') {
            return false;
        }
        let mut i = 1;
        while let Some(c) = self.peek_at(i) {
            if is_var_char(c) {
                i += 1;
            } else {
                break;
            }
        }
        if i == 1 {
            return false;
        }
        while let Some(c) = self.peek_at(i) {
            if c == ' ' || c == '\t' {
                i += 1;
            } else {
                break;
            }
        }
        self.peek_at(i) == Some(':')
    }

    /// Read an identifier-ish name (letters, digits, `_`, `-`).
    fn read_name(&mut self) -> String {
        let mut s = String::new();
        while let Some(c) = self.peek() {
            if is_var_char(c) {
                s.push(c);
                self.bump();
            } else {
                break;
            }
        }
        s
    }

    /// Skip spaces/tabs then the `:` following a declaration or variable name.
    fn skip_colon(&mut self) {
        while matches!(self.peek(), Some(' ') | Some('\t')) {
            self.bump();
        }
        if self.peek() == Some(':') {
            self.bump();
        }
    }

    /// Read a value until a terminator. `stop_newline` ends at a newline too (variables).
    /// `;` is consumed; `}` is not. Comments are removed; parentheses and strings are respected.
    fn read_value(&mut self, stop_newline: bool) -> String {
        let mut out = String::new();
        let mut depth = 0usize;
        while let Some(c) = self.peek() {
            match c {
                '/' if self.at_block_comment() => {
                    self.skip_block_comment();
                    out.push(' ');
                    continue;
                }
                '"' | '\'' => {
                    out.push(c);
                    self.bump();
                    while let Some(d) = self.peek() {
                        if d == '\n' {
                            break;
                        }
                        out.push(d);
                        self.bump();
                        if d == c {
                            break;
                        }
                    }
                    continue;
                }
                '(' => depth += 1,
                ')' => depth = depth.saturating_sub(1),
                ';' if depth == 0 => {
                    self.bump();
                    break;
                }
                '}' if depth == 0 => break,
                '\n' if stop_newline => {
                    self.bump();
                    break;
                }
                _ => {}
            }
            out.push(c);
            self.bump();
        }
        out.trim().to_string()
    }

    /// Read selector text up to `{`. Returns `None` (after reporting) if a `;`, `}` or EOF
    /// is found first; in that case the offending `;` is consumed, `}` is not.
    fn read_selector(&mut self, line: usize, column: usize) -> Option<String> {
        let mut out = String::new();
        loop {
            match self.peek() {
                None => {
                    self.error(
                        format!("expected '{{' after selector {:?}", out.trim()),
                        line,
                        column,
                    );
                    return None;
                }
                Some('{') => {
                    self.bump();
                    return Some(out.trim().to_string());
                }
                Some(';') => {
                    self.error(
                        format!("unexpected ';' after {:?}", out.trim()),
                        line,
                        column,
                    );
                    self.bump();
                    return None;
                }
                Some('}') => {
                    self.error(
                        format!("unexpected '}}' after {:?}", out.trim()),
                        line,
                        column,
                    );
                    return None;
                }
                Some('/') if self.at_block_comment() => {
                    self.skip_block_comment();
                    out.push(' ');
                }
                Some(c) => {
                    out.push(c);
                    self.bump();
                }
            }
        }
    }
}

pub(crate) fn is_var_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '-'
}

/// Tokenize a TCSS source. Errors are collected; tokenizing always continues to the end.
pub fn tokenize(css: &str) -> (Vec<Token>, Vec<CssError>) {
    let mut errors = Vec::new();
    let mut tokens = Vec::new();
    let mut s = Scanner {
        chars: css.chars().collect(),
        pos: 0,
        line: 1,
        column: 1,
        errors: &mut errors,
    };
    let mut depth = 0usize;
    let mut open_blocks: Vec<(usize, usize)> = Vec::new();
    loop {
        s.skip_trivia();
        let (line, column) = (s.line, s.column);
        let Some(c) = s.peek() else { break };
        let push = |tokens: &mut Vec<Token>, kind| {
            tokens.push(Token { kind, line, column });
        };
        if c == '}' {
            s.bump();
            if depth == 0 {
                s.error("unexpected '}'", line, column);
            } else {
                depth -= 1;
                open_blocks.pop();
                push(&mut tokens, TokenKind::BlockEnd);
            }
        } else if s.at_variable_def() {
            s.bump(); // `$`
            let name = s.read_name();
            s.skip_colon();
            let value = s.read_value(true);
            if depth > 0 {
                s.error(
                    format!("variable ${name} must be defined at the top level"),
                    line,
                    column,
                );
            } else {
                push(&mut tokens, TokenKind::VariableDef { name, value });
            }
        } else if s.at_declaration() {
            let name = s.read_name();
            s.skip_colon();
            let value = s.read_value(false);
            if depth == 0 {
                s.error(
                    format!("declaration '{name}' outside of a rule set"),
                    line,
                    column,
                );
            } else {
                push(&mut tokens, TokenKind::Declaration { name, value });
            }
        } else if let Some(sel) = s.read_selector(line, column) {
            depth += 1;
            open_blocks.push((line, column));
            push(&mut tokens, TokenKind::BlockStart(sel));
        }
    }
    for (line, column) in open_blocks.into_iter().rev() {
        errors.push(CssError {
            message: "unexpected end of file; expected '}'".into(),
            line,
            column,
        });
        tokens.push(Token {
            kind: TokenKind::BlockEnd,
            line,
            column,
        });
    }
    (tokens, errors)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(css: &str) -> Vec<TokenKind> {
        let (t, e) = tokenize(css);
        assert!(e.is_empty(), "{e:?}");
        t.into_iter().map(|t| t.kind).collect()
    }

    fn decl(name: &str, value: &str) -> TokenKind {
        TokenKind::Declaration {
            name: name.into(),
            value: value.into(),
        }
    }

    #[test]
    fn basic_rule() {
        assert_eq!(
            kinds("Button { color: red; background: blue }"),
            vec![
                TokenKind::BlockStart("Button".into()),
                decl("color", "red"),
                decl("background", "blue"),
                TokenKind::BlockEnd
            ]
        );
    }

    #[test]
    fn comments_are_skipped() {
        let css =
            "/* a */ Button /* b */ { /* c */ color: red /* d */; \n # line comment\n width: 1; }";
        assert_eq!(
            kinds(css),
            vec![
                TokenKind::BlockStart("Button".into()),
                decl("color", "red"),
                decl("width", "1"),
                TokenKind::BlockEnd
            ]
        );
    }

    #[test]
    fn variables() {
        assert_eq!(
            kinds("$accent: #ff0000;\n$x: 1 2\nA { width: $x; }"),
            vec![
                TokenKind::VariableDef {
                    name: "accent".into(),
                    value: "#ff0000".into()
                },
                TokenKind::VariableDef {
                    name: "x".into(),
                    value: "1 2".into()
                },
                TokenKind::BlockStart("A".into()),
                decl("width", "$x"),
                TokenKind::BlockEnd
            ]
        );
    }

    #[test]
    fn nested_and_pseudo() {
        assert_eq!(
            kinds("Button { &:hover { color: red; } Label{width:1} .x:focus { } }"),
            vec![
                TokenKind::BlockStart("Button".into()),
                TokenKind::BlockStart("&:hover".into()),
                decl("color", "red"),
                TokenKind::BlockEnd,
                TokenKind::BlockStart("Label".into()),
                decl("width", "1"),
                TokenKind::BlockEnd,
                TokenKind::BlockStart(".x:focus".into()),
                TokenKind::BlockEnd,
                TokenKind::BlockEnd
            ]
        );
    }

    #[test]
    fn hex_colors_and_ids_are_not_comments() {
        assert_eq!(
            kinds("#main { color: #fff; }"),
            vec![
                TokenKind::BlockStart("#main".into()),
                decl("color", "#fff"),
                TokenKind::BlockEnd
            ]
        );
    }

    #[test]
    fn parens_in_values() {
        assert_eq!(
            kinds("A { color: rgb(1, 2, 3) !important }"),
            vec![
                TokenKind::BlockStart("A".into()),
                decl("color", "rgb(1, 2, 3) !important"),
                TokenKind::BlockEnd
            ]
        );
    }

    #[test]
    fn errors_have_positions_and_recover() {
        let (tokens, errors) = tokenize("A { color: red; }\n}\nB { width: 1;");
        assert_eq!(errors.len(), 2);
        assert_eq!((errors[0].line, errors[0].column), (2, 1));
        assert_eq!((errors[1].line, errors[1].column), (3, 1));
        // B block is auto-closed.
        assert_eq!(tokens.last().unwrap().kind, TokenKind::BlockEnd);
        assert_eq!(tokens.len(), 6);
    }

    #[test]
    fn declaration_outside_rule_is_error() {
        let (tokens, errors) = tokenize("color: red;\nA {}");
        assert_eq!(errors.len(), 1);
        assert_eq!(tokens.len(), 2);
    }
}

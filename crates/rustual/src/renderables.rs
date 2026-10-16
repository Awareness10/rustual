//! Rich-style renderables for widgets: everything in [`rustual_render::renderables`]
//! ([`Table`], [`Panel`], [`Columns`], [`Padding`], [`Group`]) plus [`Syntax`], which uses the
//! TextArea highlighter. Show one with [`Static::renderable`](crate::widgets::Static::renderable)
//! or [`RichLog::write_renderable`](crate::widgets::RichLog::write_renderable).
//!
//! ```
//! use rustual::renderables::{Panel, Syntax};
//!
//! let code = Syntax::new("fn main() {}\n", "rust").line_numbers(true);
//! let panel = Panel::fit(code).title("main.rs");
//! # let _ = panel;
//! ```

pub use rustual_render::renderables::*;

use crate::widgets::markdown::highlight_code;
use crate::widgets::TextAreaTheme;
use rustual_render::cells::cell_len;
use rustual_render::text::Span;
use rustual_render::{Color, Justify, Spacing, Strip, Style, Text, TextOverflow};
use std::collections::HashSet;

/// Width of the line-number gutter beyond the digits (`NUMBERS_COLUMN_DEFAULT_PADDING`).
const NUMBERS_COLUMN_PADDING: i32 = 2;

/// Syntax-highlighted code (`rich.syntax.Syntax`), with optional line numbers, highlighted
/// lines and word wrapping. Highlighting uses the TextArea's built-in keyword highlighter
/// (python, rust, javascript, json, toml; other languages are shown plain) and a
/// [`TextAreaTheme`] (default `monokai`, like `rich`).
#[derive(Debug, Clone)]
pub struct Syntax {
    code: String,
    language: String,
    theme: TextAreaTheme,
    dedent: bool,
    line_numbers: bool,
    start_line: usize,
    highlight_lines: HashSet<usize>,
    code_width: Option<i32>,
    tab_size: usize,
    word_wrap: bool,
    background_color: Option<Color>,
    padding: Spacing,
}

impl Syntax {
    pub fn new(code: impl Into<String>, language: &str) -> Self {
        Syntax {
            code: code.into(),
            language: language.to_string(),
            theme: TextAreaTheme::get_builtin_theme("monokai").unwrap_or_default(),
            dedent: false,
            line_numbers: false,
            start_line: 1,
            highlight_lines: HashSet::new(),
            code_width: None,
            tab_size: 4,
            word_wrap: false,
            background_color: None,
            padding: Spacing::ZERO,
        }
    }

    /// A built-in TextArea theme by name (`monokai`, `dracula`, `vscode_dark`, `github_light`,
    /// `css`); unknown names keep the current theme.
    pub fn theme(mut self, name: &str) -> Self {
        if let Some(theme) = TextAreaTheme::get_builtin_theme(name) {
            self.theme = theme;
        }
        self
    }
    /// A custom theme.
    pub fn with_theme(mut self, theme: TextAreaTheme) -> Self {
        self.theme = theme;
        self
    }
    /// Remove common leading whitespace.
    pub fn dedent(mut self, dedent: bool) -> Self {
        self.dedent = dedent;
        self
    }
    pub fn line_numbers(mut self, line_numbers: bool) -> Self {
        self.line_numbers = line_numbers;
        self
    }
    /// Number of the first line (default 1).
    pub fn start_line(mut self, start_line: usize) -> Self {
        self.start_line = start_line;
        self
    }
    /// Line numbers to mark with `❱` (needs `line_numbers`).
    pub fn highlight_lines(mut self, lines: impl IntoIterator<Item = usize>) -> Self {
        self.highlight_lines = lines.into_iter().collect();
        self
    }
    /// Fixed width of the code (default: the available width).
    pub fn code_width(mut self, width: i32) -> Self {
        self.code_width = Some(width);
        self
    }
    pub fn tab_size(mut self, tab_size: usize) -> Self {
        self.tab_size = tab_size.max(1);
        self
    }
    /// Wrap long lines instead of cropping them.
    pub fn word_wrap(mut self, word_wrap: bool) -> Self {
        self.word_wrap = word_wrap;
        self
    }
    /// Override the theme's background.
    pub fn background_color(mut self, color: Color) -> Self {
        self.background_color = Some(color);
        self
    }
    pub fn padding(mut self, padding: Spacing) -> Self {
        self.padding = padding;
        self
    }

    fn base_style(&self) -> Style {
        let mut style = self.theme.base_style.clone().unwrap_or_default();
        if let Some(bg) = self.background_color {
            style.bg = Some(bg);
        }
        style
    }

    fn numbers_column_width(&self) -> i32 {
        if !self.line_numbers {
            return 0;
        }
        let last = self.start_line + self.code.matches('\n').count();
        last.to_string().len() as i32 + NUMBERS_COLUMN_PADDING
    }

    /// Dedented, tab-expanded code ending with a newline, and whether the original did.
    fn processed_code(&self) -> (bool, String) {
        let ends_on_nl = self.code.ends_with('\n');
        let mut code = if self.dedent {
            dedent(&self.code)
        } else {
            self.code.clone()
        };
        code = expand_tabs(&code, self.tab_size);
        if !code.ends_with('\n') {
            code.push('\n');
        }
        (ends_on_nl, code)
    }

    fn highlight(&self, code: &str) -> Text {
        let mut text =
            highlight_code(code, &self.language, &self.theme).unwrap_or_else(|| Text::plain(code));
        text.stylize_base(&self.base_style());
        text
    }

    /// Styles of the gutter: (background, number, highlighted number).
    fn number_styles(&self) -> (Style, Style, Style) {
        let background = self.base_style();
        let Some(bg) = background.bg else {
            return (Style::default(), Style::new().dim(), Style::default());
        };
        let fg = background.fg.unwrap_or(Color::WHITE);
        let number = Style {
            fg: Some(bg.blend(fg, 0.3)),
            ..background.clone()
        };
        let highlighted = Style {
            fg: Some(bg.blend(fg, 0.9)),
            ..background.clone()
        }
        .bold();
        (background, number, highlighted)
    }

    fn render_code(&self, options: &RenderOptions) -> Vec<Strip> {
        let background = self.base_style();
        let horizontal_padding = self.padding.left + self.padding.right;
        let numbers_width = self.numbers_column_width();
        let code_width = match self.code_width {
            Some(w) => w,
            None if self.line_numbers => options.max_width - numbers_width - 1 - horizontal_padding,
            None => options.max_width - horizontal_padding,
        }
        .max(0);
        let (ends_on_nl, code) = self.processed_code();
        let mut text = self.highlight(&code);
        let crop = |line: &Text| {
            line.wrap(code_width, Justify::Left, TextOverflow::Clip, true)
                .into_iter()
                .next()
                .unwrap_or_default()
                .set_cell_len(code_width, &background)
        };

        if !self.line_numbers && !self.word_wrap {
            if !ends_on_nl {
                text = text_remove_suffix(&text, "\n");
            }
            return text
                .wrap(code_width, Justify::Left, TextOverflow::Clip, true)
                .into_iter()
                .map(|l| l.set_cell_len(code_width, &background))
                .collect();
        }

        let mut lines = split_lines(&text);
        if !ends_on_nl {
            lines.pop();
        }
        let (gutter_style, number_style, highlight_style) = self.number_styles();
        let mut out = Vec::new();
        for (i, line) in lines.iter().enumerate() {
            let line_no = self.start_line + i;
            let wrapped: Vec<Strip> = if self.word_wrap {
                line.wrap(code_width, Justify::Left, TextOverflow::Fold, false)
                    .into_iter()
                    .map(|l| l.set_cell_len(code_width, &background))
                    .collect()
            } else {
                vec![crop(line)]
            };
            for (j, wrapped_line) in wrapped.into_iter().enumerate() {
                let mut strip = Strip::default();
                if self.line_numbers {
                    if j == 0 {
                        let column = format!(
                            "{:>width$} ",
                            line_no,
                            width = (numbers_width - 2).max(0) as usize
                        );
                        if self.highlight_lines.contains(&line_no) {
                            strip.push(Span::new("❱ ", Style::parse("red").unwrap_or_default()));
                            strip.push(Span::new(column, highlight_style.clone()));
                        } else {
                            strip.push(Span::new("  ", highlight_style.clone()));
                            strip.push(Span::new(column, number_style.clone()));
                        }
                    } else {
                        strip.push(Span::new(
                            " ".repeat(numbers_width as usize + 1),
                            gutter_style.clone(),
                        ));
                    }
                }
                strip.spans.extend(wrapped_line.spans);
                out.push(strip);
            }
        }
        out
    }
}

impl Renderable for Syntax {
    fn render(&self, options: &RenderOptions) -> Vec<Strip> {
        if self.padding == Spacing::ZERO {
            return self.render_code(options);
        }
        // `render_code` already leaves room for the horizontal padding.
        let code = Prerendered(self.render_code(options));
        Padding::new(code, self.padding)
            .style(self.base_style())
            .render(options)
    }

    fn measure(&self, _options: &RenderOptions) -> Measurement {
        let padding = self.padding.left + self.padding.right;
        let numbers = self.numbers_column_width();
        if let Some(w) = self.code_width {
            return Measurement::new(numbers, w + numbers + padding + 1);
        }
        let widest = self.code.lines().map(cell_len).max().unwrap_or(0);
        let mut width = numbers + padding + widest;
        if self.line_numbers {
            width += 1;
        }
        Measurement::new(numbers, width)
    }
}

/// Lines that are already rendered (`rich.segment.Segments`).
#[derive(Debug, Clone)]
struct Prerendered(Vec<Strip>);

impl Renderable for Prerendered {
    fn render(&self, _options: &RenderOptions) -> Vec<Strip> {
        self.0.clone()
    }
}

/// Split on `\n` keeping a trailing empty line (`Text.split("\n", allow_blank=True)`).
fn split_lines(text: &Text) -> Vec<Text> {
    let mut lines = vec![Text::new()];
    for span in &text.spans {
        for (i, part) in span.text.split('\n').enumerate() {
            if i > 0 {
                lines.push(Text::new());
            }
            if !part.is_empty() {
                lines
                    .last_mut()
                    .expect("at least one line")
                    .push(part, span.style.clone());
            }
        }
    }
    lines
}

fn text_remove_suffix(text: &Text, suffix: &str) -> Text {
    let mut out = text.clone();
    if let Some(last) = out.spans.last_mut() {
        if let Some(stripped) = last.text.strip_suffix(suffix) {
            last.text = stripped.to_string();
        }
    }
    out
}

/// `str.expandtabs(tab_size)`.
fn expand_tabs(code: &str, tab_size: usize) -> String {
    if !code.contains('\t') {
        return code.to_string();
    }
    let mut out = String::with_capacity(code.len());
    let mut col = 0;
    for c in code.chars() {
        match c {
            '\t' => {
                let n = tab_size - col % tab_size;
                out.extend(std::iter::repeat_n(' ', n));
                col += n;
            }
            '\n' => {
                out.push(c);
                col = 0;
            }
            _ => {
                out.push(c);
                col += 1;
            }
        }
    }
    out
}

/// `textwrap.dedent`: remove the longest common leading whitespace of non-blank lines.
fn dedent(code: &str) -> String {
    let indent = code
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| &l[..l.len() - l.trim_start().len()])
        .reduce(|a, b| {
            let n = a.chars().zip(b.chars()).take_while(|(x, y)| x == y).count();
            &a[..a.char_indices().nth(n).map_or(a.len(), |(i, _)| i)]
        })
        .unwrap_or("");
    code.split('\n')
        .map(|l| {
            l.strip_prefix(indent)
                .unwrap_or(l.trim_start_matches([' ', '\t']))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

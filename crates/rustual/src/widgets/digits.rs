//! `Digits` (port of `textual/widgets/_digits.py` and `textual/renderables/digits.py`):
//! numbers drawn in a 3x3 unicode "font". Bold text (`text-style: bold`) uses heavy glyphs.

use crate::widget::{Content, RenderCtx, Widget};
use rustual_render::{Strip, Style};

const DEFAULT_CSS: &str = "
Digits {
    width: 1fr;
    height: auto;
    text-align: left;
    box-sizing: border-box;
}
";

/// Characters with a 3x3 glyph (each is 3 cells wide; anything else is 1 cell, bottom row).
pub const DIGITS: &str = " 0123456789+-^x:ABCDEF$£€()";

#[rustfmt::skip]
const DIGITS3X3: [&str; 81] = [
    "", "", "",
    "╭─╮", "│ │", "╰─╯",
    "╶╮", " │", "╶┴╴",
    "╶─╮", "┌─┘", "╰─╴",
    "╶─╮", " ─┤", "╶─╯",
    "╷ ╷", "╰─┤", "  ╵",
    "╭─╴", "╰─╮", "╶─╯",
    "╭─╴", "├─╮", "╰─╯",
    "╶─┐", "  │", "  ╵",
    "╭─╮", "├─┤", "╰─╯",
    "╭─╮", "╰─┤", "╶─╯",
    "", "╶┼╴", "",
    "", "╶─╴", "",
    " ^", "", "",
    "", " ×", "",
    "", " :", "",
    "╭─╮", "├─┤", "╵ ╵",
    "┌─╮", "├─┤", "└─╯",
    "╭─╮", "│", "╰─╯",
    "┌─╮", "│ │", "└─╯",
    "╭─╴", "├─", "╰─╴",
    "╭─╴", "├─", "╵",
    "╭╫╮", "╰╫╮", "╰╫╯",
    "╭─╮", "╪═", "┷━╸",
    "╭─╮", "╪═", "╰─╯",
    "╭╴ ", "│  ", "╰╴ ",
    " ╶╮ ", "  │ ", " ╶╯ ",
];

#[rustfmt::skip]
const DIGITS3X3_BOLD: [&str; 81] = [
    "", "", "",
    "┏━┓", "┃ ┃", "┗━┛",
    "╺┓", " ┃", "╺┻╸",
    "╺━┓", "┏━┛", "┗━╸",
    "╺━┓", " ━┫", "╺━┛",
    "╻ ╻", "┗━┫", "  ╹",
    "┏━╸", "┗━┓", "╺━┛",
    "┏━╸", "┣━┓", "┗━┛",
    "╺━┓", "  ┃", "  ╹",
    "┏━┓", "┣━┫", "┗━┛",
    "┏━┓", "┗━┫", "╺━┛",
    "", "╺╋╸", "",
    "", "╺━╸", "",
    " ^", "", "",
    "", " ×", "",
    "", " :", "",
    "╭─╮", "├─┤", "╵ ╵",
    "┌─╮", "├─┤", "└─╯",
    "╭─╮", "│", "╰─╯",
    "┌─╮", "│ │", "└─╯",
    "╭─╴", "├─", "╰─╴",
    "╭─╴", "├─", "╵",
    "╭╫╮", "╰╫╮", "╰╫╯",
    "╭─╮", "╪═", "┷━╸",
    "╭─╮", "╪═", "╰─╯",
    "╭╴ ", "│  ", "╰╴ ",
    " ╶╮ ", "  │ ", " ╶╯ ",
];

fn ljust3(s: &str) -> String {
    format!("{s:<3}")
}

/// Render `text` as three rows of glyphs (port of `renderables.Digits.render`).
pub fn render_digits(text: &str, bold: bool) -> [String; 3] {
    let table = if bold { &DIGITS3X3_BOLD } else { &DIGITS3X3 };
    let mut rows = [String::new(), String::new(), String::new()];
    for c in text.chars().map(|c| if c == '.' { '•' } else { c }) {
        match DIGITS.chars().position(|d| d == c) {
            Some(i) => {
                for (r, row) in rows.iter_mut().enumerate() {
                    row.push_str(&ljust3(table[i * 3 + r]));
                }
            }
            None => {
                rows[0].push(' ');
                rows[1].push(' ');
                rows[2].push(c);
            }
        }
    }
    rows
}

/// Width in cells of `text` rendered as digits.
pub fn digits_width(text: &str) -> i32 {
    text.chars()
        .map(|c| if DIGITS.contains(c) { 3 } else { 1 })
        .sum()
}

/// Displays a value using large 3x3 digits. Supports `0-9 + - ^ x : A-F $ £ € ( ) .`.
#[derive(Debug, Clone, Default)]
pub struct Digits {
    value: String,
}

impl Digits {
    pub fn new(value: impl Into<String>) -> Self {
        Digits {
            value: value.into(),
        }
    }

    pub fn value(&self) -> &str {
        &self.value
    }

    pub fn update(&mut self, value: impl Into<String>) {
        self.value = value.into();
    }
}

impl Widget for Digits {
    fn type_names(&self) -> &'static [&'static str] {
        &["Digits"]
    }

    fn default_css(&self) -> &'static str {
        DEFAULT_CSS
    }

    fn content_width(&self, _ctx: &RenderCtx) -> Option<i32> {
        Some(digits_width(&self.value))
    }

    fn content_height(&self, _ctx: &RenderCtx, _width: i32) -> Option<i32> {
        Some(3)
    }

    fn render(&self, ctx: &RenderCtx) -> Content {
        let bold = ctx.base_style().bold.unwrap_or(false);
        let justify = match ctx.justify() {
            rustual_render::Justify::Center => rustual_render::Justify::Center,
            rustual_render::Justify::Right => rustual_render::Justify::Right,
            _ => rustual_render::Justify::Left,
        };
        let lines = render_digits(&self.value, bold)
            .iter()
            .map(|row| {
                Strip::from_str(row, Style::new()).align(ctx.size.width, justify, &Style::new())
            })
            .collect();
        Content::Lines(lines)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_glyphs() {
        let rows = render_digits("12", false);
        assert_eq!(rows[0], "╶╮ ╶─╮");
        assert_eq!(rows[1], " │ ┌─┘");
        assert_eq!(rows[2], "╶┴╴╰─╴");
        let rows = render_digits("1.5", true);
        assert_eq!(rows[2], "╺┻╸•╺━┛");
        assert_eq!(digits_width("1.5"), 7);
    }
}

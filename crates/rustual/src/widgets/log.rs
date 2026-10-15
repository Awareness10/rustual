//! `Log` (port of `textual/widgets/_log.py`): a self-scrolling widget of plain text lines.
//!
//! Writing needs the widget's [`Ctx`] (for auto-scrolling), so from elsewhere use
//! `ctx.with::<Log, _>("#log", |log, ctx| log.write_line("hello", ctx))`.

use crate::binding::Binding;
use crate::context::Ctx;
use crate::widget::{Content, RenderCtx, Widget};
use crate::widgets::scroll_bindings;
use rustual_render::{cells::cell_len, Size, Strip, Style};
use std::collections::VecDeque;

const DEFAULT_CSS: &str = r#"
Log {
    width: 1fr;
    height: 1fr;
    background: $surface;
    color: $text;
    overflow: scroll;
    &:focus {
        background-tint: $foreground 5%;
    }
}
"#;

/// Expand tabs and replace control codes (`\x00`-`\x14`) with `�` (`Log._process_line`).
pub(crate) fn process_line(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut col = 0usize;
    for c in line.chars() {
        if c == '\t' {
            let n = 8 - (col % 8);
            out.extend(std::iter::repeat_n(' ', n));
            col += n;
        } else if (c as u32) <= 0x14 {
            out.push('\u{fffd}');
            col += 1;
        } else {
            out.push(c);
            col += unicode_width::UnicodeWidthChar::width(c).unwrap_or(0);
        }
    }
    out
}

/// Split into (line, has_ending) pairs on `\r\n`, `\n` or `\r` (`textual._line_split`).
fn line_split(data: &str) -> Vec<(&str, bool)> {
    let mut out = Vec::new();
    let bytes = data.as_bytes();
    let mut start = 0;
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'\n' => {
                out.push((&data[start..i], true));
                start = i + 1;
            }
            b'\r' => {
                out.push((&data[start..i], true));
                if bytes.get(i + 1) == Some(&b'\n') {
                    i += 1;
                }
                start = i + 1;
            }
            _ => {}
        }
        i += 1;
    }
    if start < data.len() {
        out.push((&data[start..], false));
    }
    out
}

/// Whether the widget is currently scrolled to the bottom (`is_vertical_scroll_end`).
pub(crate) fn is_vertical_scroll_end(ctx: &Ctx) -> bool {
    ctx.scroll_offset().y >= ctx.max_scroll().y
}

/// Scroll to the last line (the offset is clamped at the next layout).
pub(crate) fn scroll_end(ctx: &mut Ctx) {
    let x = ctx.scroll_offset().x;
    ctx.scroll_to(x, i32::MAX / 2);
}

/// A widget that displays lines of plain text, scrolling to new lines as they are written.
#[derive(Debug, Clone)]
pub struct Log {
    lines: VecDeque<String>,
    width: i32,
    max_lines: Option<usize>,
    auto_scroll: bool,
    highlight: bool,
}

impl Default for Log {
    fn default() -> Self {
        Self::new()
    }
}

impl Log {
    pub fn new() -> Self {
        Log {
            lines: VecDeque::new(),
            width: 0,
            max_lines: None,
            auto_scroll: true,
            highlight: false,
        }
    }

    /// Maximum number of lines to keep (oldest are discarded).
    pub fn max_lines(mut self, max_lines: Option<usize>) -> Self {
        self.max_lines = max_lines;
        self
    }

    /// Scroll to the end when new lines are written (default `true`).
    pub fn auto_scroll(mut self, auto_scroll: bool) -> Self {
        self.auto_scroll = auto_scroll;
        self
    }

    /// Accepted for API parity with Textual; syntax highlighting is not implemented.
    pub fn highlight(mut self, highlight: bool) -> Self {
        self.highlight = highlight;
        self
    }

    pub fn set_max_lines(&mut self, max_lines: Option<usize>) {
        self.max_lines = max_lines;
        self.prune();
    }

    pub fn set_auto_scroll(&mut self, auto_scroll: bool) {
        self.auto_scroll = auto_scroll;
    }

    pub fn get_auto_scroll(&self) -> bool {
        self.auto_scroll
    }

    pub fn get_max_lines(&self) -> Option<usize> {
        self.max_lines
    }

    pub fn is_highlight(&self) -> bool {
        self.highlight
    }

    pub fn set_highlight(&mut self, highlight: bool) {
        self.highlight = highlight;
    }

    /// The raw lines (a trailing partial line from [`Log::write`] is included).
    pub fn lines(&self) -> impl Iterator<Item = &str> {
        self.lines.iter().map(String::as_str)
    }

    /// Number of lines of content.
    pub fn line_count(&self) -> usize {
        match self.lines.back() {
            Some(last) => self.lines.len() - usize::from(last.is_empty()),
            None => 0,
        }
    }

    fn prune(&mut self) {
        if let Some(max) = self.max_lines {
            while self.lines.len() > max {
                self.lines.pop_front();
            }
        }
    }

    /// Write raw data; newlines start new lines, a trailing partial line is continued by the next write.
    pub fn write(&mut self, data: &str, ctx: &mut Ctx) {
        self.write_with(data, None, ctx)
    }

    /// Like [`Log::write`]; `scroll_end` overrides `auto_scroll` when `Some`.
    pub fn write_with(&mut self, data: &str, scroll_end: Option<bool>, ctx: &mut Ctx) {
        self.push(data);
        if scroll_end.unwrap_or(self.auto_scroll) {
            self::scroll_end(ctx);
        }
    }

    /// Append raw data like [`Log::write`] without a ctx (no auto-scroll), e.g. before the
    /// widget is mounted.
    pub fn push(&mut self, data: &str) {
        if !data.is_empty() {
            if self.lines.is_empty() {
                self.lines.push_back(String::new());
            }
            for (line, ending) in line_split(data) {
                let last = self.lines.back_mut().expect("non-empty");
                last.push_str(line);
                self.width = self.width.max(cell_len(&process_line(last)));
                if ending {
                    self.lines.push_back(String::new());
                }
            }
        }
        self.prune();
    }

    /// Write a single line.
    pub fn write_line(&mut self, line: &str, ctx: &mut Ctx) {
        self.write_lines([line], ctx)
    }

    /// Write several lines. Auto-scrolls only if the log was already scrolled to the end.
    pub fn write_lines<'s>(&mut self, lines: impl IntoIterator<Item = &'s str>, ctx: &mut Ctx) {
        self.write_lines_with(lines, None, ctx)
    }

    /// Like [`Log::write_lines`]; `scroll_end` overrides `auto_scroll` when `Some`.
    pub fn write_lines_with<'s>(
        &mut self,
        lines: impl IntoIterator<Item = &'s str>,
        scroll_end: Option<bool>,
        ctx: &mut Ctx,
    ) {
        let at_end = is_vertical_scroll_end(ctx);
        self.push_lines(lines);
        if scroll_end.unwrap_or(self.auto_scroll) && at_end {
            self::scroll_end(ctx);
        }
    }

    /// Append lines without a ctx (no auto-scroll), e.g. before the widget is mounted.
    pub fn push_lines<'s>(&mut self, lines: impl IntoIterator<Item = &'s str>) {
        for line in lines {
            if line.is_empty() {
                self.lines.push_back(String::new());
                continue;
            }
            for l in line.lines() {
                self.width = self.width.max(cell_len(&process_line(l)));
                self.lines.push_back(l.to_string());
            }
        }
        self.prune();
    }

    /// Remove all lines.
    pub fn clear(&mut self, ctx: &mut Ctx) {
        self.lines.clear();
        self.width = 0;
        ctx.scroll_to(0, 0);
    }
}

impl Widget for Log {
    fn type_names(&self) -> &'static [&'static str] {
        &["Log", "ScrollView"]
    }

    fn default_css(&self) -> &'static str {
        DEFAULT_CSS
    }

    fn can_focus(&self) -> bool {
        true
    }

    fn bindings(&self) -> Vec<Binding> {
        scroll_bindings()
    }

    fn virtual_size(&self, _ctx: &RenderCtx) -> Option<Size> {
        Some(Size::new(self.width, self.line_count() as i32))
    }

    fn content_width(&self, _ctx: &RenderCtx) -> Option<i32> {
        Some(self.width)
    }

    fn content_height(&self, _ctx: &RenderCtx, _width: i32) -> Option<i32> {
        Some(self.line_count() as i32)
    }

    fn render(&self, ctx: &RenderCtx) -> Content {
        let Size { width, height } = ctx.size;
        let off = ctx.scroll_offset;
        let count = self.line_count();
        let mut out = Vec::with_capacity(height.max(0) as usize);
        for y in 0..height.max(0) {
            let idx = (off.y + y) as usize;
            if idx >= count {
                break;
            }
            let line = process_line(&self.lines[idx]);
            let strip = Strip::from_str(&line, Style::default());
            out.push(strip.crop(off.x, off.x + width));
        }
        Content::Lines(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_lines() {
        assert_eq!(
            line_split("a\nb\r\nc\rd"),
            vec![("a", true), ("b", true), ("c", true), ("d", false)]
        );
        assert_eq!(line_split("a\n"), vec![("a", true)]);
    }

    #[test]
    fn processes_control_codes() {
        assert_eq!(process_line("a\tb"), "a       b");
        assert_eq!(process_line("x\x01"), "x\u{fffd}");
    }
}

//! `RichLog` (port of `textual/widgets/_rich_log.py`): a self-scrolling log of styled text.
//!
//! Lines are [`Text`] (or markup when `markup(true)`); with `wrap(true)` long lines are
//! word-wrapped to the widget width (at least `min_width`). Writing needs the widget's [`Ctx`]:
//! `ctx.with::<RichLog, _>("#log", |log, ctx| log.write("[b]hi[/]", ctx))`.
//!
//! [`RichLog::write_renderable`] writes a Rich-style renderable (a table, panel, syntax, ...):
//! it is rendered once, at write time, like Textual's `RichLog.write`.

use crate::binding::Binding;
use crate::context::Ctx;
use crate::renderables::{measure, render_lines, IntoRenderable, RenderOptions};
use crate::widget::{Content, RenderCtx, Widget};
use crate::widgets::log::{is_vertical_scroll_end, process_line, scroll_end};
use crate::widgets::pretty;
use crate::widgets::scroll_bindings;
use rustual_css::Overflow;
use rustual_render::{Justify, Size, Span, Strip, Text, TextOverflow};
use std::cell::RefCell;
use std::collections::VecDeque;

const DEFAULT_CSS: &str = r#"
RichLog {
    width: 1fr;
    height: 1fr;
    overflow-y: auto;
    overflow-x: auto;
}
RichLog{
    background: $surface;
    color: $foreground;
    overflow-y: scroll;
    &:focus {
        background-tint: $foreground 5%;
    }
}
"#;

/// Wrapped display lines for one width: `(entry absolute index, strip)`.
#[derive(Debug, Default, Clone)]
struct WrapCache {
    width: i32,
    /// Absolute index of the next entry to wrap.
    next_entry: usize,
    lines: VecDeque<(usize, Strip)>,
    widest: i32,
}

/// A logical line of the log.
#[derive(Debug, Clone)]
struct Entry {
    line: Strip,
    /// Re-wrapped to the widget width when wrapping is on (`false` for pre-rendered lines).
    wrap: bool,
}

/// A log of styled text lines.
#[derive(Debug, Clone)]
pub struct RichLog {
    /// Logical lines (already split on newlines, tabs expanded).
    entries: VecDeque<Entry>,
    /// Absolute index of `entries[0]` (grows as old lines are pruned).
    start_entry: usize,
    widest: i32,
    max_lines: Option<usize>,
    min_width: i32,
    wrap: bool,
    markup: bool,
    highlight: bool,
    auto_scroll: bool,
    cache: RefCell<WrapCache>,
}

impl Default for RichLog {
    fn default() -> Self {
        Self::new()
    }
}

impl RichLog {
    pub fn new() -> Self {
        RichLog {
            entries: VecDeque::new(),
            start_entry: 0,
            widest: 0,
            max_lines: None,
            min_width: 78,
            wrap: false,
            markup: false,
            highlight: false,
            auto_scroll: true,
            cache: RefCell::new(WrapCache::default()),
        }
    }

    /// Maximum number of (logical) lines to keep.
    pub fn max_lines(mut self, max_lines: Option<usize>) -> Self {
        self.max_lines = max_lines;
        self
    }

    /// Minimum width used when wrapping (default 78, as in Textual).
    pub fn min_width(mut self, min_width: i32) -> Self {
        self.min_width = min_width.max(1);
        self
    }

    /// Word-wrap long lines (default off).
    pub fn wrap(mut self, wrap: bool) -> Self {
        self.wrap = wrap;
        self
    }

    /// Parse strings passed to [`RichLog::write`] as markup (default off).
    pub fn markup(mut self, markup: bool) -> Self {
        self.markup = markup;
        self
    }

    /// Highlight strings passed to [`RichLog::write`] with a port of Rich's `ReprHighlighter`
    /// (numbers, strings, `True` / `None`, calls, URLs, paths, ...; see
    /// [`pretty::tokenize_text`]). Default off. Colors are Rich's `repr.*` styles under
    /// Textual's ANSI theme for the current dark / light mode at write time.
    pub fn highlight(mut self, highlight: bool) -> Self {
        self.highlight = highlight;
        self
    }

    pub fn set_highlight(&mut self, highlight: bool) {
        self.highlight = highlight;
    }

    pub fn is_highlighting(&self) -> bool {
        self.highlight
    }

    /// Scroll to the end on write (default `true`).
    pub fn auto_scroll(mut self, auto_scroll: bool) -> Self {
        self.auto_scroll = auto_scroll;
        self
    }

    pub fn set_wrap(&mut self, wrap: bool) {
        self.wrap = wrap;
        *self.cache.borrow_mut() = WrapCache::default();
    }

    pub fn set_auto_scroll(&mut self, auto_scroll: bool) {
        self.auto_scroll = auto_scroll;
    }

    pub fn get_auto_scroll(&self) -> bool {
        self.auto_scroll
    }

    pub fn get_wrap(&self) -> bool {
        self.wrap
    }

    pub fn set_markup(&mut self, markup: bool) {
        self.markup = markup;
    }

    pub fn is_markup(&self) -> bool {
        self.markup
    }

    pub fn get_min_width(&self) -> i32 {
        self.min_width
    }

    pub fn set_min_width(&mut self, min_width: i32) {
        self.min_width = min_width.max(1);
        *self.cache.borrow_mut() = WrapCache::default();
    }

    pub fn get_max_lines(&self) -> Option<usize> {
        self.max_lines
    }

    /// Change the maximum number of lines (oldest lines are discarded now if needed).
    pub fn set_max_lines(&mut self, max_lines: Option<usize>) {
        self.max_lines = max_lines;
        self.prune();
    }

    /// Number of logical lines.
    pub fn line_count(&self) -> usize {
        self.entries.len()
    }

    /// Logical lines as plain text.
    pub fn plain_lines(&self) -> Vec<String> {
        self.entries.iter().map(|e| e.line.plain_text()).collect()
    }

    /// Write a string (markup if `markup(true)`, else plain; highlighted if `highlight(true)`).
    pub fn write(&mut self, content: &str, ctx: &mut Ctx) {
        let mut text = if self.markup {
            Text::from_markup(content)
        } else {
            Text::plain(content)
        };
        if self.highlight {
            let dark = ctx.is_dark();
            text = pretty::highlight_text(&text, &|t| pretty::repr_style(t, dark));
        }
        self.write_text(text, ctx);
    }

    /// Write styled text; `\n` starts new lines.
    pub fn write_text(&mut self, text: Text, ctx: &mut Ctx) {
        self.write_text_with(text, None, ctx)
    }

    /// Like [`RichLog::write_text`]; `scroll_end` overrides `auto_scroll` when `Some`.
    pub fn write_text_with(&mut self, text: Text, scroll_end: Option<bool>, ctx: &mut Ctx) {
        self.push_text(text);
        if scroll_end.unwrap_or(self.auto_scroll) {
            self::scroll_end(ctx);
        }
    }

    /// Append text without a ctx (no auto-scroll).
    pub fn push_text(&mut self, text: Text) {
        let lines = text.lines();
        if lines.is_empty() {
            self.entries.push_back(Entry {
                line: Strip::default(),
                wrap: true,
            });
        }
        for line in lines {
            let line = expand_strip(&line);
            self.widest = self.widest.max(line.cell_len());
            self.entries.push_back(Entry { line, wrap: true });
        }
        self.prune();
    }

    /// Write a Rich-style renderable ([`crate::renderables`]). It is rendered once, at its
    /// measured width, shrunk to fit the content region and at least `min_width` wide
    /// (Textual's `RichLog.write` defaults: `expand=False, shrink=True`).
    pub fn write_renderable(&mut self, renderable: impl IntoRenderable, ctx: &mut Ctx) {
        self.write_renderable_with(renderable, None, false, None, ctx)
    }

    /// [`RichLog::write_renderable`] with Textual's options: a fixed `width` (ignores
    /// `expand`, the region and `min_width`), `expand` to the region width, and `scroll_end`
    /// overriding `auto_scroll`.
    pub fn write_renderable_with(
        &mut self,
        renderable: impl IntoRenderable,
        width: Option<i32>,
        expand: bool,
        scroll_end: Option<bool>,
        ctx: &mut Ctx,
    ) {
        let region_width = ctx.content_region().width;
        let console_width = ctx.app_size().width;
        self.push_renderable(renderable, width, expand, region_width, console_width);
        if scroll_end.unwrap_or(self.auto_scroll) {
            self::scroll_end(ctx);
        }
    }

    /// Append a renderable without a ctx (no auto-scroll). It is measured within
    /// `console_width` (the terminal width, as Textual's console does) and fitted to a content
    /// region `region_width` wide (`0`: not laid out yet, so not fitted).
    pub fn push_renderable(
        &mut self,
        renderable: impl IntoRenderable,
        width: Option<i32>,
        expand: bool,
        region_width: i32,
        console_width: i32,
    ) {
        let renderable = renderable.into_renderable();
        let render_width = match width {
            Some(w) => w,
            None => {
                let console_width = console_width.max(1);
                let measured =
                    measure(renderable.as_ref(), &RenderOptions::new(console_width)).maximum;
                let mut w = measured;
                if region_width > 0 {
                    if expand && measured < region_width {
                        w = region_width;
                    }
                    if measured > region_width {
                        w = region_width;
                    }
                }
                w.max(self.min_width)
            }
        }
        .max(1);
        let lines = render_lines(renderable.as_ref(), &RenderOptions::new(render_width));
        if lines.is_empty() {
            self.entries.push_back(Entry {
                line: Strip::blank(render_width, Default::default()),
                wrap: false,
            });
        }
        for line in lines {
            self.widest = self.widest.max(line.cell_len());
            self.entries.push_back(Entry { line, wrap: false });
        }
        self.prune();
    }

    fn prune(&mut self) {
        if let Some(max) = self.max_lines {
            while self.entries.len() > max {
                self.entries.pop_front();
                self.start_entry += 1;
            }
        }
    }

    /// Remove all lines.
    pub fn clear(&mut self, ctx: &mut Ctx) {
        self.start_entry += self.entries.len();
        self.entries.clear();
        self.widest = 0;
        *self.cache.borrow_mut() = WrapCache::default();
        ctx.scroll_to(0, 0);
    }

    /// Whether the log was scrolled to the end at the last layout.
    pub fn is_scroll_end(&self, ctx: &Ctx) -> bool {
        is_vertical_scroll_end(ctx)
    }

    fn wrap_width(&self, content_width: i32) -> i32 {
        content_width.max(self.min_width).max(1)
    }

    /// Bring the wrap cache up to date for `width` and return the number of display lines.
    fn sync_cache(&self, width: i32) -> usize {
        let mut cache = self.cache.borrow_mut();
        if cache.width != width {
            *cache = WrapCache {
                width,
                next_entry: self.start_entry,
                lines: VecDeque::new(),
                widest: 0,
            };
        }
        while cache
            .lines
            .front()
            .is_some_and(|(e, _)| *e < self.start_entry)
        {
            cache.lines.pop_front();
        }
        if cache.next_entry < self.start_entry {
            cache.next_entry = self.start_entry;
        }
        let end = self.start_entry + self.entries.len();
        while cache.next_entry < end {
            let abs = cache.next_entry;
            let entry = &self.entries[abs - self.start_entry];
            if !entry.wrap {
                cache.widest = cache.widest.max(entry.line.cell_len());
                cache.lines.push_back((abs, entry.line.clone()));
                cache.next_entry += 1;
                continue;
            }
            let text = Text {
                spans: entry.line.spans.clone(),
            };
            let wrapped = text.wrap(width, Justify::Left, TextOverflow::Fold, false);
            if wrapped.is_empty() {
                cache.lines.push_back((abs, Strip::default()));
            }
            for s in wrapped {
                cache.widest = cache.widest.max(s.cell_len());
                cache.lines.push_back((abs, s));
            }
            cache.next_entry += 1;
        }
        cache.lines.len()
    }

    /// Content width available for wrapping, given the size passed to `virtual_size`
    /// (which does not yet exclude a vertical scrollbar).
    fn layout_width(ctx: &RenderCtx) -> i32 {
        let st = ctx.styles();
        let mut w = ctx.size.width;
        if st.get_overflow_y() == Overflow::Scroll {
            w -= st.get_scrollbar_size_vertical();
        }
        w
    }
}

/// Expand tabs / drop control codes in each span.
fn expand_strip(strip: &Strip) -> Strip {
    if !strip
        .spans
        .iter()
        .any(|s| s.text.chars().any(|c| (c as u32) <= 0x14))
    {
        return strip.clone();
    }
    // Expand tabs relative to the whole line.
    let mut out = Strip::default();
    let mut col = 0usize;
    for span in &strip.spans {
        let mut text = String::new();
        for c in span.text.chars() {
            if c == '\t' {
                let n = 8 - (col % 8);
                text.extend(std::iter::repeat_n(' ', n));
                col += n;
            } else {
                let p = process_line(&c.to_string());
                col += rustual_render::cells::cell_len(&p) as usize;
                text.push_str(&p);
            }
        }
        out.push(Span::new(text, span.style.clone()));
    }
    out
}

impl Widget for RichLog {
    fn type_names(&self) -> &'static [&'static str] {
        &["RichLog", "ScrollView"]
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

    fn virtual_size(&self, ctx: &RenderCtx) -> Option<Size> {
        if self.wrap {
            let width = self.wrap_width(Self::layout_width(ctx));
            let lines = self.sync_cache(width);
            let widest = self.cache.borrow().widest;
            Some(Size::new(widest, lines as i32))
        } else {
            Some(Size::new(self.widest, self.entries.len() as i32))
        }
    }

    fn content_height(&self, ctx: &RenderCtx, width: i32) -> Option<i32> {
        if self.wrap {
            Some(self.sync_cache(self.wrap_width(width.max(Self::layout_width(ctx)))) as i32)
        } else {
            Some(self.entries.len() as i32)
        }
    }

    fn render(&self, ctx: &RenderCtx) -> Content {
        let Size { width, height } = ctx.size;
        let off = ctx.scroll_offset;
        let mut out = Vec::with_capacity(height.max(0) as usize);
        if self.wrap {
            let count = self.sync_cache(self.wrap_width(width));
            let cache = self.cache.borrow();
            for y in 0..height.max(0) {
                let idx = (off.y + y) as usize;
                if idx >= count {
                    break;
                }
                out.push(cache.lines[idx].1.crop(off.x, off.x + width));
            }
        } else {
            for y in 0..height.max(0) {
                let idx = (off.y + y) as usize;
                let Some(entry) = self.entries.get(idx) else {
                    break;
                };
                out.push(entry.line.crop(off.x, off.x + width));
            }
        }
        Content::Lines(out)
    }
}

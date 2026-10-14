//! Styling inline content and mapping clicks to links.
//!
//! The core does not execute `@click` markup yet, so blocks that may contain links record,
//! at render time, where each link span ended up (a [`LinkMap`]) and resolve clicks against it.

use super::parse::{Inline, InlineSpan};
use crate::context::Ctx;
use crate::widget::RenderCtx;
use std::cell::RefCell;
use textual_css::AlignH;
use textual_css::AlignV;
use textual_render::{Strip, Style, Text};

/// Only the attributes of `style` that differ from `base`.
fn partial(style: Style, base: &Style) -> Style {
    let mut s = style;
    if s.fg == base.fg {
        s.fg = None;
    }
    if s.bg == base.bg {
        s.bg = None;
    }
    macro_rules! attr {
        ($($f:ident),*) => { $( if s.$f == base.$f { s.$f = None; } )* };
    }
    attr!(bold, dim, italic, underline, strike, reverse, blink);
    s
}

/// Resolved styles for the inline component classes of one block.
pub(crate) struct InlineStyles {
    strong: Style,
    em: Style,
    strike: Style,
    code: Style,
    link: Style,
    link_hover: Style,
}

impl InlineStyles {
    pub(crate) fn new(ctx: &RenderCtx) -> Self {
        let base = ctx.base_style();
        let styles = ctx.styles();
        let var_style = |name: &str, fallback: &str| {
            ctx.env
                .stylesheet
                .variables()
                .get(name)
                .and_then(|v| Style::parse(v).ok())
                .unwrap_or_else(|| Style::parse(fallback).unwrap_or_default())
        };
        let bg = ctx.background();
        let mut link = styles
            .link_style
            .clone()
            .unwrap_or_else(|| var_style("link-style", "underline"));
        link.fg = styles
            .link_color
            .or_else(|| ctx.var_color("link-color"))
            .map(|c| bg.composite(c));
        link.bg = styles
            .link_background
            .or_else(|| ctx.var_color("link-background"))
            .map(|c| bg.composite(c));
        let mut link_hover = styles
            .link_style_hover
            .clone()
            .unwrap_or_else(|| var_style("link-style-hover", "bold"));
        link_hover.fg = styles
            .link_color_hover
            .or_else(|| ctx.var_color("link-color-hover"))
            .map(|c| bg.composite(c));
        link_hover.bg = styles
            .link_background_hover
            .or_else(|| ctx.var_color("link-background-hover"))
            .map(|c| bg.composite(c));
        InlineStyles {
            strong: partial(ctx.component_style("strong"), &base),
            em: partial(ctx.component_style("em"), &base),
            strike: partial(ctx.component_style("s"), &base),
            code: partial(ctx.component_style("code_inline"), &base),
            link,
            link_hover,
        }
    }

    pub(crate) fn span_style(&self, span: &InlineSpan, hover: Option<&str>) -> Style {
        let mut style = Style::default();
        if span.code {
            style = style.combine(&self.code);
        }
        if span.strong {
            style = style.combine(&self.strong);
        }
        if span.em {
            style = style.combine(&self.em);
        }
        if span.strike {
            style = style.combine(&self.strike);
        }
        if let Some(href) = &span.link {
            style = style.combine(&self.link);
            if hover == Some(href.as_str()) {
                // Hover style replaces the link's text attributes (e.g. "bold not underline").
                style = style.combine(&self.link_hover);
            }
            style.link = Some(href.as_str().into());
        }
        style
    }

    /// Build styled text for inline content.
    pub(crate) fn text(&self, inline: &Inline, hover: Option<&str>) -> Text {
        let mut t = Text::new();
        for span in &inline.spans {
            t.push(span.text.clone(), self.span_style(span, hover));
        }
        t
    }
}

/// A link's position within rendered content lines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LinkSpan {
    pub y: i32,
    pub x0: i32,
    pub x1: i32,
    pub href: String,
}

/// Where links were drawn at the last render (content coordinates, before `content-align`).
#[derive(Debug, Clone, Default)]
pub(crate) struct LinkMap {
    pub spans: Vec<LinkSpan>,
    /// Size of the rendered block (for replaying `content-align` offsets).
    pub block_w: i32,
    pub block_h: i32,
}

impl LinkMap {
    /// Record links found in `lines` drawn at `(dx, dy)`.
    pub(crate) fn add_lines(&mut self, lines: &[Strip], dx: i32, dy: i32) {
        for (y, line) in lines.iter().enumerate() {
            let mut x = dx;
            for span in &line.spans {
                let w = span.cell_len();
                if let Some(href) = &span.style.link {
                    match self.spans.last_mut() {
                        Some(last)
                            if last.y == dy + y as i32
                                && last.x1 == x
                                && last.href.as_str() == &**href =>
                        {
                            last.x1 = x + w;
                        }
                        _ => self.spans.push(LinkSpan {
                            y: dy + y as i32,
                            x0: x,
                            x1: x + w,
                            href: href.to_string(),
                        }),
                    }
                }
                x += w;
            }
        }
    }

    pub(crate) fn finish(&mut self, lines: &[Strip]) {
        self.block_w = lines.iter().map(|l| l.cell_len()).max().unwrap_or(0);
        self.block_h = lines.len() as i32;
    }

    pub(crate) fn at(&self, x: i32, y: i32) -> Option<&str> {
        self.spans
            .iter()
            .find(|s| s.y == y && x >= s.x0 && x < s.x1)
            .map(|s| s.href.as_str())
    }
}

/// Interior-mutable link map, written by `render` (which only gets `&self`).
#[derive(Debug, Clone, Default)]
pub(crate) struct LinkTracker {
    map: RefCell<LinkMap>,
    /// Href under the mouse.
    pub hover: Option<String>,
}

impl LinkTracker {
    pub(crate) fn set(&self, map: LinkMap) {
        *self.map.borrow_mut() = map;
    }

    pub(crate) fn hover(&self) -> Option<&str> {
        self.hover.as_deref()
    }

    /// The link at a screen position, replaying the compositor's `content-align` offset.
    pub(crate) fn link_at(&self, screen_x: i32, screen_y: i32, ctx: &Ctx) -> Option<String> {
        let cr = ctx.content_region();
        let map = self.map.borrow();
        let styles = ctx.styles();
        let (ah, av) = styles.get_content_align();
        let off_x = match ah {
            AlignH::Left => 0,
            AlignH::Center => ((cr.width - map.block_w) / 2).max(0),
            AlignH::Right => (cr.width - map.block_w).max(0),
        };
        let off_y = match av {
            AlignV::Top => 0,
            AlignV::Middle => ((cr.height - map.block_h) / 2).max(0),
            AlignV::Bottom => (cr.height - map.block_h).max(0),
        };
        let x = screen_x - cr.x - off_x;
        let y = screen_y - cr.y - off_y;
        map.at(x, y).map(String::from)
    }

    /// Track hover and post [`super::LinkClicked`] on clicks. Returns `Some(flow)` if handled.
    pub(crate) fn handle_event(
        &mut self,
        event: &crate::events::Event,
        ctx: &mut Ctx,
    ) -> Option<crate::message::Flow> {
        use crate::events::Event;
        use crate::message::Flow;
        match event {
            Event::MouseMove(m) => {
                self.hover = self.link_at(m.screen_x, m.screen_y, ctx);
                None
            }
            Event::Leave => {
                self.hover = None;
                None
            }
            Event::Click(m) => {
                let href = self.link_at(m.screen_x, m.screen_y, ctx)?;
                ctx.post_message(super::LinkClicked {
                    href: percent_decode(&href),
                });
                Some(Flow::Stop)
            }
            _ => None,
        }
    }
}

/// Decode `%XX` escapes (Textual's `unquote`); invalid sequences are kept verbatim.
pub(crate) fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).ok();
            if let Some(v) = hex.and_then(|h| u8::from_str_radix(h, 16).ok()) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8(out).unwrap_or_else(|_| s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode() {
        assert_eq!(percent_decode("a%20b"), "a b");
        assert_eq!(percent_decode("100%"), "100%");
        assert_eq!(percent_decode("%zz"), "%zz");
    }

    #[test]
    fn link_map_merges_adjacent_spans() {
        let mut t = Text::new();
        t.push("ab ", Style::default());
        t.push("cd", Style::default().link("x"));
        t.push("ef", Style::default().bold().link("x"));
        let lines = t.lines();
        let mut m = LinkMap::default();
        m.add_lines(&lines, 0, 0);
        assert_eq!(m.spans.len(), 1);
        assert_eq!((m.spans[0].x0, m.spans[0].x1), (3, 7));
        assert_eq!(m.at(5, 0), Some("x"));
        assert_eq!(m.at(1, 0), None);
    }
}

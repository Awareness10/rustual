//! `Footer` (port of `textual/widgets/_footer.py`): shows the active key bindings, docked at
//! the bottom of the screen.
//!
//! Textual composes one `FooterKey` widget per binding; textual-rs renders all items in a single
//! widget (`Content::Lines`), styled with the same component classes (`footer-key--key`,
//! `footer-key--description`) plus `footer-key--hover` for the item under the mouse.
//!
//! Pressing the mouse on an item simulates its key (like Textual's `app.simulate_key`), so the
//! binding runs on whichever widget owns it — including non-namespaced actions of the focused
//! widget, screen or app.

use crate::binding::Binding;
use crate::context::Ctx;
use crate::events::{Event, KeyEvent};
use crate::message::Flow;
use crate::widget::{Content, RenderCtx, Widget};
use textual_render::{cells::cell_len, Span, Strip, Style};

const DEFAULT_CSS: &str = "
Footer {
    layout: horizontal;
    color: $footer-foreground;
    background: $footer-background;
    dock: bottom;
    height: 1;
    scrollbar-size: 0 0;
    text-wrap: nowrap;

    & > .footer-key--key {
        color: $footer-key-foreground;
        background: $footer-key-background;
        text-style: bold;
    }
    & > .footer-key--description {
        color: $footer-description-foreground;
        background: $footer-description-background;
    }
    & > .footer-key--hover {
        color: $footer-key-foreground;
        background: $block-hover-background;
    }
}
";

/// One rendered footer item: the cell range it covers and its binding.
#[derive(Debug, Clone, PartialEq)]
pub struct FooterItem {
    pub start: i32,
    pub end: i32,
    pub key_text: String,
    pub description_text: String,
    pub binding: Binding,
    /// The description of a binding group (not clickable; `binding` is the group's first).
    pub group_label: bool,
}

/// Displays the active bindings (`RenderEnv::active_bindings`) as ` key description ` items.
#[derive(Debug, Clone, Default)]
pub struct Footer {
    compact: bool,
    hover: Option<usize>,
}

impl Footer {
    pub fn new() -> Self {
        Footer::default()
    }

    /// Compact style: less whitespace between items (`-compact` class).
    pub fn compact(mut self, compact: bool) -> Self {
        self.compact = compact;
        self
    }

    pub fn is_compact(&self) -> bool {
        self.compact
    }

    pub fn set_compact(&mut self, compact: bool, ctx: &mut Ctx) {
        self.compact = compact;
        ctx.set_class(compact, "-compact");
    }

    /// Lay out items for `bindings` (padding as in Textual's `FooterKey` CSS:
    /// key `0 1` / description `0 1 0 0`; compact: key `0`, description `0 0 0 1`, margin-right 1).
    ///
    /// Two or more consecutive bindings with the same [`BindingGroup`](crate::binding::BindingGroup)
    /// are shown as their keys (one item each, no description) followed by the group's
    /// description (an item with `group_label`), as in Textual's `KeyGroup` / `FooterLabel`.
    pub fn layout_items(&self, bindings: &[Binding]) -> Vec<FooterItem> {
        let shown: Vec<&Binding> = bindings.iter().filter(|b| b.show).collect();
        let mut x = 0;
        let mut out = Vec::new();
        let mut i = 0;
        while i < shown.len() {
            let b = shown[i];
            let run = shown[i..]
                .iter()
                .take_while(|o| b.group.is_some() && o.group == b.group)
                .count();
            if let (Some(group), true) = (&b.group, run > 1) {
                // KeyGroup: grouped keys have margin `0 1` (collapsed between siblings), or
                // none in a compact group (which has `padding-left: 1; margin-right: 1`).
                x += 1;
                for (k, gb) in shown[i..i + run].iter().enumerate() {
                    if k > 0 && !group.compact {
                        x += 1;
                    }
                    let key_text = gb.display_key();
                    let w = cell_len(&key_text);
                    out.push(FooterItem {
                        start: x,
                        end: x + w,
                        key_text,
                        description_text: String::new(),
                        binding: (*gb).clone(),
                        group_label: false,
                    });
                    x += w;
                }
                x += 1;
                // FooterLabel: margin `0 1 0 0` (`0` in a compact footer).
                let w = cell_len(&group.description);
                out.push(FooterItem {
                    start: x,
                    end: x + w,
                    key_text: String::new(),
                    description_text: group.description.clone(),
                    binding: b.clone(),
                    group_label: true,
                });
                x += w;
                if !self.compact {
                    x += 1;
                }
                i += run;
                continue;
            }
            let key = b.display_key();
            let (key_text, description_text) = if b.description.is_empty() {
                (key, String::new())
            } else if self.compact {
                (key, format!(" {}", b.description))
            } else {
                (format!(" {key} "), format!("{} ", b.description))
            };
            let width = cell_len(&key_text) + cell_len(&description_text);
            out.push(FooterItem {
                start: x,
                end: x + width,
                key_text,
                description_text,
                binding: b.clone(),
                group_label: false,
            });
            x += width;
            if self.compact {
                x += 1;
            }
            i += 1;
        }
        out
    }

    fn item_at(&self, bindings: &[Binding], x: i32) -> Option<usize> {
        self.layout_items(bindings)
            .iter()
            .position(|i| x >= i.start && x < i.end)
    }

    /// Mouse x (relative to the widget region) → content x.
    fn content_x(ctx: &Ctx, x: i32) -> i32 {
        x - (ctx.content_region().x - ctx.region().x)
    }
}

impl Widget for Footer {
    fn type_names(&self) -> &'static [&'static str] {
        &["Footer"]
    }

    fn default_css(&self) -> &'static str {
        DEFAULT_CSS
    }

    fn can_focus_children(&self) -> bool {
        false
    }

    fn on_mount(&mut self, ctx: &mut Ctx) {
        ctx.set_class(self.compact, "-compact");
    }

    fn on_event(&mut self, event: &Event, ctx: &mut Ctx) -> Flow {
        match event {
            Event::MouseMove(m) => {
                let x = Self::content_x(ctx, m.x);
                self.hover = self.item_at(&ctx.app.env.active_bindings, x);
                Flow::Continue
            }
            Event::Leave => {
                self.hover = None;
                Flow::Continue
            }
            Event::MouseDown(m) => {
                let x = Self::content_x(ctx, m.x);
                let items = self.layout_items(&ctx.app.env.active_bindings);
                let Some(item) = items
                    .into_iter()
                    .find(|i| x >= i.start && x < i.end && !i.group_label)
                else {
                    return Flow::Continue;
                };
                // Textual: `FooterKey.on_mouse_down` → `app.simulate_key(key)`.
                if let Some(key) = item.binding.keys.first() {
                    ctx.app.handle_key(KeyEvent::new(key.as_str()));
                }
                Flow::Stop
            }
            _ => Flow::Continue,
        }
    }

    fn render(&self, ctx: &RenderCtx) -> Content {
        let key_style = ctx.component_style("footer-key--key");
        let desc_style = ctx.component_style("footer-key--description");
        let hover_style = ctx.component_style("footer-key--hover");
        let mut spans = Vec::new();
        let mut x = 0;
        for (i, item) in self
            .layout_items(&ctx.env.active_bindings)
            .iter()
            .enumerate()
        {
            if item.start > x {
                spans.push(Span::new(
                    " ".repeat((item.start - x) as usize),
                    Style::new(),
                ));
            }
            let (mut ks, mut ds) = (key_style.clone(), desc_style.clone());
            if self.hover == Some(i) {
                ks.bg = hover_style.bg;
                ds.bg = hover_style.bg;
            }
            if self.hover == Some(i) && item.group_label {
                ds.bg = desc_style.bg;
            }
            if item.binding.disabled {
                ks.dim = Some(true);
                ds.dim = Some(true);
            }
            if !item.key_text.is_empty() {
                spans.push(Span::new(item.key_text.clone(), ks));
            }
            if !item.description_text.is_empty() {
                spans.push(Span::new(item.description_text.clone(), ds));
            }
            x = item.end;
        }
        let strip = Strip::new(spans).set_cell_len(ctx.size.width, &Style::new());
        Content::Lines(vec![strip])
    }
}

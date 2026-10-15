//! Scrollbar rendering and interaction, and mouse capture.
//!
//! - Rendering: [`render_bar`] is a port of Textual's `ScrollBarRender.render_bar`
//!   (`textual/scrollbar.py`), drawing the thumb with ⅛-cell glyphs at its ends.
//! - Interaction (Textual's `ScrollBar` widget): pressing the left button on a thumb grabs it and
//!   dragging scrolls proportionally (`virtual_size / window_size` per cell); pressing on the
//!   track above / below the thumb scrolls a page (animated, like `scroll_page_up/down`).
//!   Scrollbars use `scrollbar-color-hover` / `scrollbar-background-hover` while hovered and the
//!   `-active` colors while grabbed.
//! - Mouse capture ([`Ctx::capture_mouse`]): while captured, all mouse events go to the
//!   capturing widget. While a button is held after a `MouseDown`, `MouseMove` events go to the
//!   widget that received the `MouseDown` (implicit capture for drags).
//! - Click chains: [`Ctx::click_chain`] is 2 for a double click, 3 for a triple click (Textual's
//!   `Click.chain`; same position within 500 ms).
//!
//! Deviations: scrollbars are not widgets here (the compositor paints them; this module hit-tests
//! the geometry from the last layout). Dragging scrolls immediately (Textual animates for 0.1s on
//! terminals without smooth scrolling). There are no `MouseCapture` / `MouseRelease` events.

use crate::app::{AppState, MouseKind};
use crate::context::Ctx;
use crate::dom::{Node, NodeId};
use crate::events::{MouseButton, MouseEvent};
use rustual_render::{Color, Offset, Region};
use std::time::Duration;
use tokio::time::Instant;

/// Glyphs for vertical scrollbar ends (Textual's `ScrollBarRender.VERTICAL_BARS`).
pub const VERTICAL_BARS: [&str; 8] = ["▁", "▂", "▃", "▄", "▅", "▆", "▇", " "];
/// Glyphs for horizontal scrollbar ends (Textual's `ScrollBarRender.HORIZONTAL_BARS`).
pub const HORIZONTAL_BARS: [&str; 8] = ["▉", "▊", "▋", "▌", "▍", "▎", "▏", " "];

/// Textual's `App.CLICK_CHAIN_TIME_THRESHOLD`.
const CLICK_CHAIN_THRESHOLD: Duration = Duration::from_millis(500);

/// One cell of a rendered scrollbar (along its length).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BarCell {
    pub glyph: &'static str,
    pub fg: Color,
    pub bg: Color,
    /// Part of the thumb (grabbable).
    pub thumb: bool,
}

/// Thumb geometry in ⅛ cells: `(start_index, start_bar, end_index, end_bar)` as in
/// `ScrollBarRender.render_bar`, or None when there is no thumb.
fn thumb_eighths(
    size: i32,
    virtual_size: f32,
    window_size: f32,
    position: f32,
) -> Option<(i32, i32, i32, i32)> {
    let len_bars = 8;
    if window_size <= 0.0 || size <= 0 || virtual_size <= 0.0 || size as f32 == virtual_size {
        return None;
    }
    let bar_ratio = virtual_size / size as f32;
    let thumb_size = (window_size / bar_ratio).max(1.0);
    let position_ratio = position / (virtual_size - window_size);
    let position = (size as f32 - thumb_size) * position_ratio;
    let start = (position * len_bars as f32) as i32;
    let end = start + (thumb_size * len_bars as f32).ceil() as i32;
    let (start_index, start_bar) = (start.max(0) / len_bars, start.max(0) % len_bars);
    let (end_index, end_bar) = (end.max(0) / len_bars, end.max(0) % len_bars);
    Some((start_index, start_bar, end_index, end_bar))
}

/// Render a scrollbar of `size` cells (port of `ScrollBarRender.render_bar`). `window_size` is
/// 0 when the content fits (no thumb). `back` / `bar` are the track and thumb colors.
pub fn render_bar(
    size: i32,
    virtual_size: f32,
    window_size: f32,
    position: f32,
    vertical: bool,
    back: Color,
    bar: Color,
) -> Vec<BarCell> {
    let blank = BarCell {
        glyph: " ",
        fg: bar,
        bg: back,
        thumb: false,
    };
    let mut cells = vec![blank; size.max(0) as usize];
    let Some((start_index, start_bar, end_index, end_bar)) =
        thumb_eighths(size, virtual_size, window_size, position)
    else {
        return cells;
    };
    let bars = if vertical {
        &VERTICAL_BARS
    } else {
        &HORIZONTAL_BARS
    };
    for i in start_index.max(0)..end_index.min(size) {
        // Blank with `color=bar, reverse=True`: the thumb color as background.
        cells[i as usize] = BarCell {
            glyph: " ",
            fg: back,
            bg: bar,
            thumb: true,
        };
    }
    if (0..size).contains(&start_index) {
        let glyph = bars[7 - start_bar as usize];
        if glyph != " " {
            cells[start_index as usize] = if vertical {
                BarCell {
                    glyph,
                    fg: bar,
                    bg: back,
                    thumb: true,
                }
            } else {
                BarCell {
                    glyph,
                    fg: back,
                    bg: bar,
                    thumb: true,
                }
            };
        }
    }
    if (0..size).contains(&end_index) {
        let glyph = bars[7 - end_bar as usize];
        if glyph != " " {
            cells[end_index as usize] = if vertical {
                BarCell {
                    glyph,
                    fg: back,
                    bg: bar,
                    thumb: true,
                }
            } else {
                BarCell {
                    glyph,
                    fg: bar,
                    bg: back,
                    thumb: true,
                }
            };
        }
    }
    cells
}

/// Which scrollbar is highlighted for painting (hovered or grabbed).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScrollbarHighlight {
    pub node: NodeId,
    pub vertical: bool,
    /// Grabbed (active colors) rather than hovered.
    pub active: bool,
}

/// Track / thumb colors for a scrollbar of `node` over background `inner_bg`.
pub(crate) fn bar_colors(node: &Node, inner_bg: Color, state: Option<bool>) -> (Color, Color) {
    let s = &node.styles;
    let default_back = Color::rgb(30, 30, 30);
    let default_bar = Color::rgb(90, 90, 90);
    let (back, bar) = match state {
        Some(true) => (
            s.scrollbar_background_active
                .or(s.scrollbar_background)
                .unwrap_or(default_back),
            s.scrollbar_color_active
                .or(s.scrollbar_color)
                .unwrap_or(default_bar),
        ),
        Some(false) => (
            s.scrollbar_background_hover
                .or(s.scrollbar_background)
                .unwrap_or(default_back),
            s.scrollbar_color_hover
                .or(s.scrollbar_color)
                .unwrap_or(default_bar),
        ),
        None => (
            s.scrollbar_background.unwrap_or(default_back),
            s.scrollbar_color.unwrap_or(default_bar),
        ),
    };
    let back = inner_bg.composite(back);
    (back, back.composite(bar))
}

/// The region of a node's vertical / horizontal scrollbar (screen coordinates), if shown.
pub(crate) fn bar_region(node: &Node, vertical: bool) -> Option<Region> {
    let l = &node.layout;
    let cr = l.content_region;
    if vertical && l.scrollbar_v {
        let w = node.styles.get_scrollbar_size_vertical();
        Some(Region::new(cr.right(), cr.y, w, cr.height))
    } else if !vertical && l.scrollbar_h {
        let h = node.styles.get_scrollbar_size_horizontal();
        Some(Region::new(cr.x, cr.bottom(), cr.width, h))
    } else {
        None
    }
}

/// `(size, virtual_size, window_size, position)` of a node's scrollbar, as Textual's
/// `ScrollBar` reactives (`window_size` is 0 when the content fits).
pub(crate) fn bar_metrics(node: &Node, vertical: bool) -> (i32, f32, f32, f32) {
    let l = &node.layout;
    let cr = l.content_region;
    let (size, virt, pos) = if vertical {
        (cr.height, l.virtual_size.height, node.scroll_offset.y)
    } else {
        (cr.width, l.virtual_size.width, node.scroll_offset.x)
    };
    let window = if size < virt { size } else { 0 };
    (size, virt as f32, window as f32, pos as f32)
}

/// A grabbed (or pressed) scrollbar.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Grab {
    node: NodeId,
    vertical: bool,
    /// Dragging the thumb (false: the track was pressed; the press is swallowed until release).
    dragging: bool,
    /// Screen coordinate along the bar where the thumb was grabbed.
    grab_pos: i32,
    /// Scroll position when grabbed.
    grab_offset: i32,
}

/// Mouse state: capture, scrollbar grab / hover, click chains.
#[derive(Debug, Default)]
pub(crate) struct MouseState {
    pub captured: Option<NodeId>,
    pub grab: Option<Grab>,
    pub hover_bar: Option<(NodeId, bool)>,
    /// Chain count of the last click.
    pub click_chain: u32,
    last_click: Option<(Instant, Offset)>,
}

impl AppState {
    fn sync_scrollbar_highlight(&mut self) {
        let h = match (self.mouse.grab, self.mouse.hover_bar) {
            (Some(g), _) if g.dragging => Some(ScrollbarHighlight {
                node: g.node,
                vertical: g.vertical,
                active: true,
            }),
            (_, Some((node, vertical))) => Some(ScrollbarHighlight {
                node,
                vertical,
                active: false,
            }),
            _ => None,
        };
        if self.env.scrollbar_highlight != h {
            self.env.scrollbar_highlight = h;
            self.dirty = true;
        }
    }

    /// The scrollbar under (x, y): the topmost node at that point whose scrollbar contains it.
    fn scrollbar_at(&self, x: i32, y: i32) -> Option<(NodeId, bool)> {
        let id = self.node_at(x, y)?;
        let node = self.dom.get(id)?;
        if !node.layout.clip.contains(x, y) {
            return None;
        }
        [true, false]
            .into_iter()
            .find(|v| bar_region(node, *v).is_some_and(|r| r.contains(x, y)))
            .map(|v| (id, v))
    }

    /// Scrollbar handling, run before normal mouse dispatch. Returns true if the event was
    /// consumed by a scrollbar.
    pub(crate) fn scrollbar_mouse(&mut self, kind: MouseKind, m: &MouseEvent) -> bool {
        if let Some(c) = self.mouse.captured {
            if !self.dom.contains(c) {
                self.mouse.captured = None;
            }
        }
        if let Some(g) = self.mouse.grab {
            if !self.dom.contains(g.node) {
                self.mouse.grab = None;
            }
        }
        let (x, y) = (m.screen_x, m.screen_y);
        if let Some(g) = self.mouse.grab {
            match kind {
                MouseKind::Move => {
                    if g.dragging {
                        self.drag_scrollbar(g, x, y);
                    }
                }
                MouseKind::Up => {
                    self.mouse.grab = None;
                    self.mouse.hover_bar = self.scrollbar_at(x, y);
                    self.sync_scrollbar_highlight();
                }
                _ => return false,
            }
            return true;
        }
        let at = if self.mouse.captured.is_some() {
            None
        } else {
            self.scrollbar_at(x, y)
        };
        if matches!(kind, MouseKind::Move | MouseKind::Down) && self.mouse.hover_bar != at {
            self.mouse.hover_bar = at;
            self.sync_scrollbar_highlight();
        }
        let Some((id, vertical)) = at else {
            return false;
        };
        match kind {
            MouseKind::Down => {
                if m.button == Some(MouseButton::Left) {
                    self.press_scrollbar(id, vertical, x, y);
                }
                true
            }
            // Releasing over a scrollbar that wasn't pressed: Textual's ScrollBar stops it.
            MouseKind::Up => true,
            _ => false,
        }
    }

    fn press_scrollbar(&mut self, id: NodeId, vertical: bool, x: i32, y: i32) {
        // Focus the nearest focusable ancestor (Textual focuses the widget under the pointer).
        let focus = self.dom.ancestors_with_self(id).into_iter().find(|n| {
            self.dom.get(*n).map(|n| n.can_focus).unwrap_or(false) && !self.dom.is_disabled(*n)
        });
        if let Some(f) = focus {
            if self.focus_chain().contains(&f) {
                self.set_focus(Some(f));
            }
        }
        let Some(node) = self.dom.get(id) else { return };
        let Some(region) = bar_region(node, vertical) else {
            return;
        };
        let (size, virt, window, pos) = bar_metrics(node, vertical);
        let along = if vertical { y - region.y } else { x - region.x };
        let mut grab = Grab {
            node: id,
            vertical,
            dragging: false,
            grab_pos: if vertical { y } else { x },
            grab_offset: pos as i32,
        };
        if let Some((start, _, end, end_bar)) = thumb_eighths(size, virt, window, pos) {
            // A partial end glyph is part of the thumb.
            let end = if end_bar != 0 { end + 1 } else { end };
            if along >= start && along < end {
                grab.dragging = true;
            } else {
                let page = if vertical {
                    node.layout.content_region.height
                } else {
                    node.layout.content_region.width
                };
                let cur = self.scroll_target(id);
                let delta = if along < start { -page } else { page };
                let target = if vertical {
                    Offset::new(cur.x, cur.y + delta)
                } else {
                    Offset::new(cur.x + delta, cur.y)
                };
                self.scroll_page(id, target);
            }
        }
        self.mouse.grab = Some(grab);
        self.sync_scrollbar_highlight();
    }

    /// Scroll a page (from a track click), animated unless animations are off.
    fn scroll_page(&mut self, id: NodeId, target: Offset) {
        self.scroll_animated(id, target, None, crate::animation::Easing::DEFAULT_SCROLL);
    }

    fn drag_scrollbar(&mut self, g: Grab, x: i32, y: i32) {
        let Some(node) = self.dom.get(g.node) else {
            return;
        };
        let (_, virt, window, _) = bar_metrics(node, g.vertical);
        if window <= 0.0 {
            return;
        }
        let delta = if g.vertical { y } else { x } - g.grab_pos;
        let pos = g.grab_offset as f32 + delta as f32 * (virt / window);
        let max = node.layout.max_scroll();
        let pos = pos.round() as i32;
        let mut off = node.scroll_offset;
        if g.vertical {
            off.y = pos.clamp(0, max.y);
        } else {
            off.x = pos.clamp(0, max.x);
        }
        self.animator.cancel_scroll(g.node);
        if let Some(n) = self.dom.get_mut(g.node) {
            if n.scroll_offset != off {
                n.scroll_offset = off;
                self.dirty = true;
            }
        }
    }

    /// Where a (non-scrollbar) mouse event goes: the capturing widget, the widget that received
    /// `MouseDown` for drags (a move with a button held), or the widget under the pointer.
    pub(crate) fn mouse_route(
        &self,
        kind: MouseKind,
        m: &MouseEvent,
        under: Option<NodeId>,
    ) -> Option<NodeId> {
        if let Some(c) = self.mouse.captured.filter(|c| self.dom.contains(*c)) {
            return Some(c);
        }
        if kind == MouseKind::Move && m.button.is_some() {
            if let Some(d) = self.mouse_down_on.filter(|d| self.dom.contains(*d)) {
                return Some(d);
            }
        }
        under
    }

    /// Record a click at `pos` and return its chain count (1 = single, 2 = double, ...).
    pub(crate) fn register_click(&mut self, pos: Offset) -> u32 {
        let now = Instant::now();
        let chained = matches!(self.mouse.last_click,
            Some((t, p)) if p == pos && now.saturating_duration_since(t) <= CLICK_CHAIN_THRESHOLD);
        self.mouse.click_chain = if chained {
            self.mouse.click_chain + 1
        } else {
            1
        };
        self.mouse.last_click = Some((now, pos));
        self.mouse.click_chain
    }
}

impl<'a> Ctx<'a> {
    /// Capture the mouse: all mouse events go to this widget until [`Ctx::release_mouse`]
    /// (Textual: `Widget.capture_mouse()`).
    pub fn capture_mouse(&mut self) {
        self.app.mouse.captured = Some(self.node);
    }

    /// Release the mouse if this widget captured it.
    pub fn release_mouse(&mut self) {
        if self.app.mouse.captured == Some(self.node) {
            self.app.mouse.captured = None;
        }
    }

    /// The widget that has captured the mouse, if any.
    pub fn mouse_captured(&self) -> Option<NodeId> {
        self.app.mouse.captured
    }

    /// The click count of the most recent click: 1 for a single click, 2 for a double click,
    /// ... (Textual's `Click.chain`). Read it while handling `Event::Click`.
    pub fn click_chain(&self) -> u32 {
        self.app.mouse.click_chain
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn glyphs(cells: &[BarCell]) -> String {
        cells
            .iter()
            .map(|c| {
                if c.thumb && c.glyph == " " {
                    "█"
                } else {
                    c.glyph
                }
            })
            .collect()
    }

    #[test]
    fn no_thumb_when_content_fits() {
        let back = Color::rgb(0, 0, 0);
        let bar = Color::rgb(255, 255, 255);
        let cells = render_bar(10, 10.0, 0.0, 0.0, true, back, bar);
        assert!(cells.iter().all(|c| !c.thumb && c.bg == back));
    }

    #[test]
    fn eighth_cell_ends() {
        let back = Color::rgb(0, 0, 0);
        let bar = Color::rgb(255, 255, 255);
        // Textual: render_bar(size=10, virtual_size=100, window_size=25, position=0)
        let top = render_bar(10, 100.0, 25.0, 0.0, true, back, bar);
        assert_eq!(glyphs(&top), "██▄       ");
        let mid = render_bar(10, 100.0, 25.0, 10.0, true, back, bar);
        // position 10/75 → 1.0 cells with thumb 2.5 cells: starts at cell 1, ends half way into 3.
        assert_eq!(glyphs(&mid), " ██▄      ");
        let end = render_bar(10, 100.0, 25.0, 75.0, true, back, bar);
        assert_eq!(glyphs(&end), "       ▄██");
        // The start glyph is drawn bar-on-back (vertical), the end glyph reversed.
        assert_eq!(end[7].fg, bar);
        assert_eq!(top[2].bg, bar);
        let h = render_bar(10, 100.0, 25.0, 0.0, false, back, bar);
        assert_eq!(glyphs(&h), "██▌       ");
    }
}

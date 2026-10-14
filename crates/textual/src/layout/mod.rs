//! Layout: resolves the box model and arranges widgets (vertical, horizontal, grid, dock),
//! writing [`NodeLayout`](crate::dom::NodeLayout) for every node of a screen.
//!
//! Semantics follow Textual (`reference/textual-src/textual/_arrange.py`, `layouts/*.py`,
//! `widget.py::_get_box_model`):
//! - `width` unset → fill the container width minus margins; `height` unset → fill container height minus margins.
//! - `auto` → content size: for widgets with children, the extent of the arranged children
//!   (plus gutter); for leaf widgets `Widget::content_width/content_height`, falling back to
//!   measuring `Widget::render` output (Text: longest line / wrapped line count; Lines: max width / count).
//! - `fr` shares remaining space along the layout axis; `%`, `w`, `h`, `vw`, `vh` per [`textual_css::Scalar::resolve`].
//! - `box-sizing: border-box` (default): width/height include padding + border.
//! - min/max constraints apply after resolution.
//! - Vertical/horizontal layouts collapse adjacent margins (max of the two), apply `align`.
//! - Docked children (`dock: top|bottom|left|right`) are placed first against the container edges and
//!   shrink the area left for the other children.
//! - `display: none` children take no space and are not visible; `visibility: hidden` take space but are not visible.
//! - `offset` shifts the final region (after layout) without affecting siblings.
//! - Overflow: if `overflow-y` is `auto` and content height > viewport, a vertical scrollbar of
//!   `scrollbar-size-vertical` width is reserved (content area shrinks, re-layout); `scroll` always reserves.
//!   Same for horizontal. `scroll_offset` is clamped to `NodeLayout::max_scroll()` and children are
//!   translated by `-scroll_offset`.
//! - `clip` = region ∩ parent's content clip (scrollbars excluded); nodes whose clip is empty are `visible: false`.
//! - Layers: children with a `layer` listed in the parent's `layers` paint in that order (later = on top).
//!
//! Further details, mirroring Textual:
//! - Paint order (`_compositor.py::_arrange_root`): a parent before its subtree; siblings sorted
//!   by (layer index, docked-above-flow, placement order). Docked widgets are not scrolled and
//!   do not count towards the virtual size (except that right/bottom docks extend it).
//! - Docks and flow layout are resolved per layer: docks in one layer do not shrink another.
//! - Multiple widgets docked to the same edge overlap (not stacked), as in Textual.
//! - `virtual_size` of a container is the union of the viewport (content region at the origin)
//!   and the children's margin boxes, so it is never smaller than the content region.
//! - `visibility` is inherited by descendants unless they set it explicitly.
//! - Children are always clipped to the parent's content region (Textual clips regardless of
//!   `overflow`).
//!
//! Implementation notes (where this port deviates from / interprets Textual):
//! - Scrollbar visibility is decided within one pass by re-arranging until it is stable
//!   (Textual converges over successive frames).
//! - `width: auto` / `height: auto` add scrollbar space only for `overflow: scroll` (or a
//!   stable gutter); Textual also uses the previous frame's scrollbar state.
//! - Leaf widgets (no children, no `layout` rule) never get scrollbars unless they report a
//!   `virtual_size` (Textual: only containers and `ScrollView`s are scrollable).
//! - `align` on an auto-sized axis does nothing (Textual additionally applies min/max there).
//! - Grid `position: absolute` uses the child's `position` (Textual reads the parent's — a bug).

mod arrange;
mod box_model;
mod dock;
mod grid;
mod horizontal;
#[cfg(test)]
mod tests;
mod vertical;

use crate::dom::{Dom, NodeId, NodeLayout};
use crate::widget::{RenderCtx, RenderEnv};
use arrange::{layer_name, ArrangeResult};
use box_model::{union_raw, LayoutCx};
use std::rc::Rc;
use textual_css::{Overflow, ScrollbarGutter, Styles, Visibility};
use textual_render::{Offset, Region, Size};

/// Paint order produced by layout (back to front), only visible nodes.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Arrangement {
    pub order: Vec<NodeId>,
    pub size: Size,
}

impl Arrangement {
    /// Topmost node whose clip contains (x, y).
    pub fn node_at(&self, dom: &Dom, x: i32, y: i32) -> Option<NodeId> {
        self.order.iter().rev().copied().find(|id| {
            dom.get(*id)
                .map(|n| n.layout.visible && n.layout.clip.contains(x, y))
                .unwrap_or(false)
        })
    }
}

/// Lay out `screen` (and all descendants) to fill `size`. Writes `layout` and clamps
/// `scroll_offset` on every node in the subtree; nodes not displayed get `visible: false`.
/// Computed styles (`Node::styles`) must already be up to date.
pub fn layout_screen(dom: &mut Dom, env: &RenderEnv, screen: NodeId, size: Size) -> Arrangement {
    let (results, order) = {
        let mut pass = Pass {
            cx: LayoutCx::new(dom, env, size),
            results: Vec::new(),
            order: Vec::new(),
        };
        if dom.contains(screen) {
            let region = size.region();
            pass.place(screen, region, region, true);
        }
        (pass.results, pass.order)
    };

    for id in dom.descendants(screen) {
        if let Some(n) = dom.get_mut(id) {
            n.layout = NodeLayout::default();
        }
    }
    for (id, layout, scroll) in results {
        if let Some(n) = dom.get_mut(id) {
            n.layout = layout;
            n.scroll_offset = scroll;
        }
    }
    Arrangement { order, size }
}

struct Pass<'a> {
    cx: LayoutCx<'a>,
    results: Vec<(NodeId, NodeLayout, Offset)>,
    order: Vec<NodeId>,
}

/// Region minus scrollbars (`Widget._get_scrollable_region`).
fn scrollable_region(st: &Styles, region: Region, show_v: bool, show_h: bool) -> Region {
    let size_v = st.get_scrollbar_size_vertical();
    let size_h = st.get_scrollbar_size_horizontal();
    let mut show_v = show_v && size_v > 0;
    let show_h = show_h && size_h > 0;
    if st.scrollbar_gutter == Some(ScrollbarGutter::Stable) {
        // Always reserve space for the vertical scrollbar.
        show_v = true;
    }
    let mut r = region;
    if show_v {
        r.width = (r.width - size_v).max(0);
    }
    if show_h {
        r.height = (r.height - size_h).max(0);
    }
    r
}

/// Which scrollbars to show (`Widget._refresh_scrollbars`), as (vertical, horizontal).
fn scrollbars_needed(st: &Styles, virtual_size: Size, container: Size) -> (bool, bool) {
    if container.area() == 0 {
        return (false, false);
    }
    let (ox, oy) = (st.get_overflow_x(), st.get_overflow_y());
    let (width, height) = (container.width, container.height);
    let mut show_h = match ox {
        Overflow::Hidden => false,
        Overflow::Scroll => true,
        Overflow::Auto => virtual_size.width > width,
    };
    let mut show_v = match oy {
        Overflow::Hidden => false,
        Overflow::Scroll => true,
        Overflow::Auto => virtual_size.height > height,
    };
    // A single scrollbar shrinks the other dimension.
    if ox == Overflow::Auto && show_v && !show_h {
        show_h = virtual_size.width > width - st.get_scrollbar_size_vertical();
    }
    if oy == Overflow::Auto && show_h && !show_v {
        show_v = virtual_size.height > height - st.get_scrollbar_size_horizontal();
    }
    (
        show_v && st.get_scrollbar_size_vertical() > 0,
        show_h && st.get_scrollbar_size_horizontal() > 0,
    )
}

/// Layer names of a container (`Widget.layers`): the outermost ancestor (up to the
/// screen) with a `layers` rule wins, as in Textual.
fn container_layers(cx: &LayoutCx, id: NodeId) -> Vec<String> {
    let mut layers = vec!["default".to_string()];
    for n in cx.dom.ancestors_with_self(id) {
        let Some(node) = cx.dom.get(n) else { break };
        if let Some(l) = &node.styles.layers {
            layers = l.clone();
        }
        if node.is_screen {
            break;
        }
    }
    layers
}

impl Pass<'_> {
    fn place(&mut self, id: NodeId, region: Region, clip: Region, parent_visible: bool) {
        let cx = &self.cx;
        let Some(node) = cx.dom.get(id) else { return };
        let st = &node.styles;
        // Visibility is inherited unless set explicitly (as in `_compositor.py`).
        let shown = match st.visibility {
            Some(v) => v == Visibility::Visible,
            None => parent_visible,
        };
        let container_region = region.shrink(st.gutter());
        let node_clip = region.intersection(&clip);

        let mut layout = NodeLayout {
            region,
            content_region: container_region,
            clip: node_clip,
            virtual_size: container_region.size(),
            scrollbar_v: false,
            scrollbar_h: false,
            visible: shown && !node_clip.is_empty(),
        };

        let self_virtual = node.widget().and_then(|w| {
            w.virtual_size(&RenderCtx::new(cx.dom, cx.env, id, container_region.size()))
        });

        let mut arrangement: Option<Rc<ArrangeResult>> = None;
        if let Some(vs) = self_virtual {
            let (sv, sh) = scrollbars_needed(st, vs, container_region.size());
            layout.content_region = scrollable_region(st, container_region, sv, sh);
            layout.virtual_size = vs;
            layout.scrollbar_v = sv;
            layout.scrollbar_h = sh;
            if !node.children.is_empty() {
                arrangement = Some(cx.arrange(id, layout.content_region.size(), true));
            }
        } else if cx.is_container(id) {
            let mut bars = (
                st.get_overflow_y() == Overflow::Scroll,
                st.get_overflow_x() == Overflow::Scroll,
            );
            let mut result = None;
            // Re-arrange until scrollbar visibility is stable (bounded).
            for _ in 0..4 {
                let child_region = scrollable_region(st, container_region, bars.0, bars.1);
                let arr = cx.arrange(id, child_region.size(), true);
                let virtual_size =
                    union_raw(child_region.size().region(), arr.total_region()).size();
                let needed = scrollbars_needed(st, virtual_size, container_region.size());
                result = Some((bars, child_region, arr, virtual_size));
                if needed == bars {
                    break;
                }
                bars = needed;
            }
            let ((sv, sh), child_region, arr, virtual_size) = result.expect("at least one pass");
            layout.content_region = child_region;
            layout.virtual_size = virtual_size;
            layout.scrollbar_v = sv;
            layout.scrollbar_h = sh;
            arrangement = Some(arr);
        }

        let max = layout.max_scroll();
        let scroll = node.scroll_offset.clamp(max.x, max.y);
        let content_origin = layout.content_region.offset();
        let sub_clip = node_clip.intersection(&layout.content_region);
        let visible = layout.visible;
        self.results.push((id, layout, scroll));
        if visible {
            self.order.push(id);
        }

        let Some(arr) = arrangement else { return };
        let layers = container_layers(&self.cx, id);
        let mut indexed: Vec<(usize, i32, usize)> = arr
            .placements
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let name = layer_name(self.cx.styles(p.node));
                let layer = layers.iter().position(|l| l == name).unwrap_or(0);
                (layer, p.z, i)
            })
            .collect();
        indexed.sort();
        for (_, _, i) in indexed {
            let p = &arr.placements[i];
            let origin = if p.fixed {
                content_origin
            } else {
                content_origin - scroll
            };
            let abs = p.region.translate(p.offset).translate(origin);
            self.place(p.node, abs, sub_clip, shown);
        }
    }
}

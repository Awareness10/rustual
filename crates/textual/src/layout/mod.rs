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

use crate::dom::{Dom, NodeId};
use crate::widget::RenderEnv;
use textual_render::{Region, Size};

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
    let _ = (dom, env, screen, size, Region::default());
    todo!("layout")
}

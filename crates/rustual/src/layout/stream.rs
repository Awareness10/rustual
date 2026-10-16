//! Stream layout: port of `layouts/stream.py`, a cut-down vertical layout for long lists (e.g.
//! a chat log in a scrolling container).
//!
//! Compared with the vertical layout:
//! - every child is full width (as if `width: 1fr`), less its left / right margin;
//! - every child is `height: auto` (content height plus padding and border), whatever its
//!   `height`; only `min-height` / `max-height` in cells apply;
//! - vertical margins collapse as in the vertical layout;
//! - `offset` and `position: absolute` are ignored.
//!
//! Textual also caches placements between frames while the width is unchanged; rustual lays out
//! every frame, so that optimization is not ported.

use super::arrange::Placement;
use super::box_model::LayoutCx;
use crate::dom::NodeId;
use rustual_css::{Scalar, Unit};
use rustual_render::{Offset, Region, Size};

/// A `min-height` / `max-height` in cells (other units are ignored by the stream layout).
fn cells(scalar: Option<Scalar>) -> Option<i32> {
    scalar
        .filter(|s| s.unit == Unit::Cells)
        .map(|s| s.value as i32)
}

pub(crate) fn arrange(cx: &LayoutCx, children: &[NodeId], size: Size) -> Vec<Placement> {
    let Some(&first) = children.first() else {
        return Vec::new();
    };
    let width = size.width;
    let mut y = 0;
    let mut previous_margin = cx.styles(first).get_margin().top;
    let mut placements = Vec::with_capacity(children.len());
    for &id in children {
        let styles = cx.styles(id);
        let margin = styles.get_margin();
        let gutter = styles.get_padding() + styles.border_spacing();
        y += margin.top.max(previous_margin);
        previous_margin = margin.bottom;
        let content_width = width - (gutter.left + gutter.right);
        let mut height = cx.content_height(id, size, content_width) + gutter.top + gutter.bottom;
        if let Some(max) = cells(styles.max_height) {
            height = height.min(max);
        }
        if let Some(min) = cells(styles.min_height) {
            height = height.max(min);
        }
        placements.push(Placement {
            node: id,
            region: Region::new(margin.left, y, width - (margin.left + margin.right), height),
            offset: Offset::ZERO,
            margin,
            z: 0,
            fixed: false,
            absolute: false,
        });
        y += height;
    }
    placements
}

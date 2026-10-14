//! Docked widgets: port of `_arrange.py::_arrange_dock_widgets`.
//!
//! Each docked widget is sized against the whole dock region and placed against its edge.
//! As in Textual, several widgets docked to the same edge overlap (they are not stacked);
//! the space reserved per edge is the maximum of their sizes.

use super::arrange::{resolve_offset, Placement, TOP_Z};
use super::box_model::{floor, LayoutCx};
use crate::dom::NodeId;
use textual_css::Dock;
use textual_render::{Region, Spacing};

pub(crate) fn arrange_docks(
    cx: &LayoutCx,
    docked: &[NodeId],
    region: Region,
    greedy: bool,
) -> (Vec<Placement>, Spacing) {
    let size = region.size();
    let (width, height) = (size.width, size.height);
    let (mut top, mut right, mut bottom, mut left) = (0, 0, 0, 0);
    let mut placements = Vec::with_capacity(docked.len());

    for &id in docked {
        let st = cx.styles(id);
        let Some(edge) = st.dock else { continue };
        let bm = cx.box_model(id, size, width as f64, height as f64, false, greedy);
        let w = floor(bm.width) + bm.margin.width();
        let h = floor(bm.height) + bm.margin.height();
        let dock_region = match edge {
            Dock::Bottom => {
                bottom = bottom.max(h);
                Region::new(0, height - h, w, h)
            }
            Dock::Top => {
                top = top.max(h);
                Region::new(0, 0, w, h)
            }
            Dock::Left => {
                left = left.max(w);
                Region::new(0, 0, w, h)
            }
            Dock::Right => {
                right = right.max(w);
                Region::new(width - w, 0, w, h)
            }
        };
        let dock_region = dock_region.shrink(bm.margin);
        placements.push(Placement {
            node: id,
            region: dock_region.translate(region.offset()),
            offset: resolve_offset(cx, st, size),
            margin: Spacing::ZERO,
            z: TOP_Z,
            fixed: true,
            absolute: false,
        });
    }
    (placements, Spacing::new(top, right, bottom, left))
}

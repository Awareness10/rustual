//! Horizontal layout: port of `layouts/horizontal.py`.
//!
//! Children are placed left to right; horizontal margins between siblings collapse
//! (max of `margin-right` / next `margin-left`). `fr` widths share the remaining width.

use super::arrange::{floor_size, is_absolute, resolve_offset, Placement};
use super::box_model::{floor, Axis, LayoutCx};
use crate::dom::NodeId;
use textual_render::{Region, Size};

pub(crate) fn arrange(
    cx: &LayoutCx,
    children: &[NodeId],
    size: Size,
    greedy: bool,
) -> Vec<Placement> {
    if children.is_empty() {
        return Vec::new();
    }
    let margins: Vec<_> = children
        .iter()
        .map(|c| cx.styles(*c).get_margin())
        .collect();
    let resolve_margin = Size::new(
        margins
            .windows(2)
            .map(|w| w[0].right.max(w[1].left))
            .sum::<i32>()
            + margins[0].left
            + margins[margins.len() - 1].right,
        margins.iter().map(|m| m.top + m.bottom).max().unwrap_or(0),
    );

    let models = cx.resolve_box_models(children, size, resolve_margin, Axis::Width, greedy);

    let mut gaps: Vec<i32> = models
        .windows(2)
        .map(|w| w[0].margin.right.max(w[1].margin.left))
        .collect();
    gaps.push(models[models.len() - 1].margin.right);

    let mut x = models[0].margin.left as f64;
    let mut placements = Vec::with_capacity(children.len());
    for ((&id, bm), gap) in children.iter().zip(&models).zip(gaps) {
        let st = cx.styles(id);
        let offset = resolve_offset(cx, st, floor_size(bm.width, bm.height));
        let next_x = x + bm.width;
        let region = Region::new(
            floor(x),
            bm.margin.top,
            floor(next_x - floor(x) as f64),
            floor(bm.height),
        );
        let absolute = is_absolute(st);
        placements.push(Placement {
            node: id,
            region,
            offset,
            margin: bm.margin,
            z: 0,
            fixed: false,
            absolute,
        });
        if !absolute {
            x = next_x + gap as f64;
        }
    }
    placements
}

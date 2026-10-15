//! Vertical layout: port of `layouts/vertical.py`.
//!
//! Children are stacked top to bottom; vertical margins between siblings collapse
//! (the larger of `margin-bottom` / next `margin-top` is used). `fr` heights share the
//! space left after fixed heights and the collapsed margins.

use super::arrange::{floor_size, is_absolute, resolve_offset, Placement};
use super::box_model::{floor, Axis, LayoutCx};
use crate::dom::NodeId;
use rustual_render::{Region, Size};

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
        margins.iter().map(|m| m.left + m.right).max().unwrap_or(0),
        margins
            .windows(2)
            .map(|w| w[0].bottom.max(w[1].top))
            .sum::<i32>()
            + margins[0].top
            + margins[margins.len() - 1].bottom,
    );

    let models = cx.resolve_box_models(children, size, resolve_margin, Axis::Height, greedy);

    let mut gaps: Vec<i32> = models
        .windows(2)
        .map(|w| w[0].margin.bottom.max(w[1].margin.top))
        .collect();
    gaps.push(models[models.len() - 1].margin.bottom);

    let mut y = models[0].margin.top as f64;
    let mut placements = Vec::with_capacity(children.len());
    for ((&id, bm), gap) in children.iter().zip(&models).zip(gaps) {
        let st = cx.styles(id);
        let next_y = y + bm.height;
        let offset = resolve_offset(cx, st, floor_size(bm.width, bm.height));
        let region = Region::new(
            bm.margin.left,
            floor(y),
            floor(bm.width),
            floor(next_y) - floor(y),
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
            y = next_y + gap as f64;
        }
    }
    placements
}

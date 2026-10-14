//! Port of `_arrange.py`: layers, docking, layout dispatch and `align`.

use super::box_model::{floor, resolve_scalar, union_raw, LayoutCx};
use super::{dock, grid, horizontal, vertical};
use crate::dom::NodeId;
use textual_css::{AlignH, AlignV, Dimension, LayoutKind, Position, Styles};
use textual_render::{Offset, Region, Size, Spacing};

/// `z` of docked placements (Textual's `TOP_Z`): docked widgets paint over siblings in the same layer.
pub(crate) const TOP_Z: i32 = i32::MAX;

/// Where a child goes, relative to the parent's content (scrollable) region origin.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Placement {
    pub node: NodeId,
    /// Border box, before `offset`.
    pub region: Region,
    /// `offset` style, applied after layout.
    pub offset: Offset,
    pub margin: Spacing,
    /// Paint order within the layer (`TOP_Z` for docked widgets).
    pub z: i32,
    /// Docked: not affected by the parent's scroll position, not part of the virtual size.
    pub fixed: bool,
    /// `position: absolute`.
    pub absolute: bool,
}

/// Result of arranging a container's children (`DockArrangeResult`).
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct ArrangeResult {
    pub placements: Vec<Placement>,
    /// Space taken by docked widgets (only right/bottom extend the scrollable area).
    pub scroll_spacing: Spacing,
}

impl ArrangeResult {
    /// Area covered by non-docked children (incl. margins), grown by right/bottom dock spacing.
    /// Always contains the origin, like Textual's spatial map total region.
    pub fn total_region(&self) -> Region {
        let mut total = Region::ZERO;
        for p in self.placements.iter().filter(|p| !p.fixed) {
            total = union_raw(total, p.region.grow(p.margin));
        }
        let s = self.scroll_spacing;
        total.grow(Spacing::new(0, s.right, s.bottom, 0))
    }
}

/// Resolve the `offset` style against `size`.
pub(crate) fn resolve_offset(cx: &LayoutCx, st: &Styles, size: Size) -> Offset {
    if st.offset_x.is_none() && st.offset_y.is_none() {
        return Offset::ZERO;
    }
    let r = |s: Option<textual_css::Scalar>, dim| {
        s.map(|s| resolve_scalar(&s, dim, size, cx.viewport, 1.0).round_ties_even() as i32)
            .unwrap_or(0)
    };
    Offset::new(
        r(st.offset_x, Dimension::Width),
        r(st.offset_y, Dimension::Height),
    )
}

pub(crate) fn is_absolute(st: &Styles) -> bool {
    st.position == Some(Position::Absolute)
}

/// Layer name of a widget (`Widget.layer`).
pub(crate) fn layer_name(st: &Styles) -> &str {
    st.layer.as_deref().unwrap_or("default")
}

/// `arrange()` from `_arrange.py`.
pub(crate) fn arrange(cx: &LayoutCx, parent: NodeId, size: Size, greedy: bool) -> ArrangeResult {
    let styles = cx.styles(parent);
    let children = cx.displayed_children(parent);

    // Group into layers, in order of first appearance.
    let mut layers: Vec<(&str, Vec<NodeId>)> = Vec::new();
    for c in children {
        let name = layer_name(cx.styles(c));
        match layers.iter_mut().find(|(n, _)| *n == name) {
            Some((_, v)) => v.push(c),
            None => layers.push((name, vec![c])),
        }
    }

    let mut placements = Vec::new();
    let mut scroll_spacing = Spacing::ZERO;
    for (_, widgets) in layers {
        let mut dock_region = size.region();
        let (docked, flow): (Vec<NodeId>, Vec<NodeId>) = widgets
            .into_iter()
            .partition(|c| cx.styles(*c).dock.is_some());

        let mut dock_spacing = Spacing::ZERO;
        if !docked.is_empty() {
            let (dock_placements, spacing) = dock::arrange_docks(cx, &docked, dock_region, greedy);
            placements.extend(dock_placements);
            dock_spacing = spacing;
            dock_region = dock_region.shrink(spacing);
        }

        if flow.is_empty() {
            continue;
        }
        let mut layout_placements = match cx.layout_kind(parent) {
            LayoutKind::Vertical => vertical::arrange(cx, &flow, dock_region.size(), greedy),
            LayoutKind::Horizontal => horizontal::arrange(cx, &flow, dock_region.size(), greedy),
            LayoutKind::Grid => grid::arrange(cx, parent, &flow, dock_region.size(), greedy),
        };
        scroll_spacing = scroll_spacing.max(dock_spacing);
        let mut placement_offset = dock_region.offset();

        let (ah, av) = styles.get_align();
        if (ah != AlignH::Left || av != AlignV::Top) && !layout_placements.is_empty() {
            let bounds = layout_placements
                .iter()
                .map(|p| p.region.grow(p.margin))
                .reduce(union_raw)
                .unwrap_or_default();
            let auto_w = matches!(styles.width, Some(w) if w.is_auto());
            let auto_h = matches!(styles.height, Some(h) if h.is_auto());
            let cw = if auto_w { 0 } else { dock_region.width };
            let ch = if auto_h { 0 } else { dock_region.height };
            let dx = match ah {
                AlignH::Left => 0,
                AlignH::Center => (cw - bounds.width).div_euclid(2),
                AlignH::Right => cw - bounds.width,
            };
            let dy = match av {
                AlignV::Top => 0,
                AlignV::Middle => (ch - bounds.height).div_euclid(2),
                AlignV::Bottom => ch - bounds.height,
            };
            placement_offset = placement_offset + Offset::new(dx.max(0), dy.max(0));
        }

        for p in &mut layout_placements {
            if p.absolute {
                // `WidgetPlacement.apply_absolute`: reset to the origin.
                p.region = Region::new(0, 0, p.region.width, p.region.height);
            } else {
                p.region = p.region.translate(placement_offset);
            }
        }
        placements.extend(layout_placements);
    }

    ArrangeResult {
        placements,
        scroll_spacing,
    }
}

/// Helper used by the flow layouts: floor a fractional size.
pub(crate) fn floor_size(w: f64, h: f64) -> Size {
    Size::new(floor(w), floor(h))
}

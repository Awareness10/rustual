//! Box model resolution: port of `widget.py::_get_box_model`, `_resolve_extrema`,
//! `get_content_width/height` and `_resolve.py` (`resolve_box_models`,
//! `resolve_fraction_unit`, `resolve`).
//!
//! Textual uses exact `Fraction`s; we use `f64` and floor with a tiny epsilon
//! ([`floor`]) so that e.g. `3 * (1/3)` still floors to 1.

use super::arrange::{arrange, ArrangeResult};
use crate::dom::{Dom, NodeId};
use crate::widget::{Content, RenderCtx, RenderEnv};
use rustual_css::{
    BoxSizing, Dimension, Display, LayoutKind, Overflow, Scalar, ScrollbarGutter, Styles, TextWrap,
    Unit,
};
use rustual_render::{Region, Size, Spacing};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

/// Floor a fractional cell value (tolerating float error).
pub(crate) fn floor(x: f64) -> i32 {
    (x + 1e-7).floor() as i32
}

/// Resolve a scalar to (fractional) cells. Mirrors `Scalar.resolve` in Textual.
pub(crate) fn resolve_scalar(
    s: &Scalar,
    dim: Dimension,
    container: Size,
    viewport: Size,
    fraction_unit: f64,
) -> f64 {
    let v = s.value as f64;
    match s.unit {
        Unit::Cells => v,
        Unit::Fraction => v * fraction_unit,
        Unit::Percent => match dim {
            Dimension::Width => v * container.width as f64 / 100.0,
            Dimension::Height => v * container.height as f64 / 100.0,
        },
        Unit::Width => v * container.width as f64 / 100.0,
        Unit::Height => v * container.height as f64 / 100.0,
        Unit::ViewWidth => v * viewport.width as f64 / 100.0,
        Unit::ViewHeight => v * viewport.height as f64 / 100.0,
        Unit::Auto => 0.0,
    }
}

fn is_relative(s: Option<Scalar>) -> bool {
    matches!(s, Some(s) if matches!(s.unit, Unit::Fraction | Unit::Percent))
}

fn is_auto(s: Option<Scalar>) -> bool {
    matches!(s, Some(s) if s.is_auto())
}

/// `styles.is_dynamic_height`: auto, fr or %.
pub(crate) fn is_dynamic_height(st: &Styles) -> bool {
    matches!(st.height, Some(s) if matches!(s.unit, Unit::Auto | Unit::Fraction | Unit::Percent))
}

/// Size of a widget's border box (fractional) plus its margin.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct BoxModel {
    pub width: f64,
    pub height: f64,
    pub margin: Spacing,
}

/// Which axis a layout resolves `fr` units along.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Axis {
    Width,
    Height,
}

type ArrangeKey = (NodeId, Size, bool);

/// Shared, read-only state for one layout pass.
pub(crate) struct LayoutCx<'a> {
    pub dom: &'a Dom,
    pub env: &'a RenderEnv,
    /// Size of the terminal (for `vw`/`vh`).
    pub viewport: Size,
    cache: RefCell<HashMap<ArrangeKey, Rc<ArrangeResult>>>,
}

static EMPTY_STYLES: std::sync::OnceLock<Styles> = std::sync::OnceLock::new();

impl<'a> LayoutCx<'a> {
    pub fn new(dom: &'a Dom, env: &'a RenderEnv, viewport: Size) -> Self {
        LayoutCx {
            dom,
            env,
            viewport,
            cache: RefCell::new(HashMap::new()),
        }
    }

    pub fn styles(&self, id: NodeId) -> &'a Styles {
        self.dom
            .get(id)
            .map(|n| &n.styles)
            .unwrap_or_else(|| EMPTY_STYLES.get_or_init(Styles::default))
    }

    /// Children with `display != none`. A screen with a maximized widget arranges only that
    /// widget plus the children allowed in the maximized view (see `crate::screens`).
    pub fn displayed_children(&self, id: NodeId) -> Vec<NodeId> {
        if let Some(children) = crate::screens::maximized_view_children(self.dom, id) {
            return children
                .into_iter()
                .filter(|c| self.styles(*c).get_display() != Display::None)
                .collect();
        }
        self.dom
            .children(id)
            .iter()
            .copied()
            .filter(|c| self.styles(*c).get_display() != Display::None)
            .collect()
    }

    /// Textual's `is_container`: has a `layout` rule or children.
    pub fn is_container(&self, id: NodeId) -> bool {
        self.styles(id).layout.is_some() || !self.dom.children(id).is_empty()
    }

    pub fn layout_kind(&self, id: NodeId) -> LayoutKind {
        self.styles(id).get_layout()
    }

    /// Arrange children of `id` within `size` (cached for the duration of the pass).
    pub fn arrange(&self, id: NodeId, size: Size, greedy: bool) -> Rc<ArrangeResult> {
        let key = (id, size, greedy);
        if let Some(r) = self.cache.borrow().get(&key) {
            return r.clone();
        }
        let result = Rc::new(arrange(self, id, size, greedy));
        self.cache.borrow_mut().insert(key, result.clone());
        result
    }

    fn render_ctx(&self, id: NodeId, size: Size) -> RenderCtx<'a> {
        RenderCtx::new(self.dom, self.env, id, size)
    }

    /// `_has_relative_children_width`.
    fn has_relative_children_width(&self, id: NodeId) -> bool {
        if !self.is_container(id) {
            return false;
        }
        self.displayed_children(id).into_iter().any(|c| {
            let st = self.styles(c);
            match st.width {
                None => false,
                Some(w) => {
                    is_relative(Some(w)) || (w.is_auto() && self.has_relative_children_width(c))
                }
            }
        })
    }

    /// `_has_relative_children_height`.
    fn has_relative_children_height(&self, id: NodeId) -> bool {
        if !self.is_container(id) {
            return false;
        }
        self.displayed_children(id).into_iter().any(|c| {
            let st = self.styles(c);
            match st.height {
                None => false,
                Some(h) => {
                    is_relative(Some(h)) || (h.is_auto() && self.has_relative_children_height(c))
                }
            }
        })
    }

    fn parent_styles(&self, id: NodeId) -> Option<&'a Styles> {
        self.dom.parent(id).map(|p| self.styles(p))
    }

    /// Optimal content width (`Widget.get_content_width`).
    pub fn content_width(&self, id: NodeId, container: Size) -> i32 {
        if self.is_container(id) {
            if self.dom.children(id).is_empty() {
                return 0;
            }
            if self.layout_kind(id) == LayoutKind::Stream {
                // Stream children are full width: the content is as wide as the space.
                return container.width.max(0);
            }
            let arr = self.arrange(id, Size::new(container.width, 0), false);
            return arr.total_region().right();
        }
        let Some(widget) = self.dom.get(id).and_then(|n| n.widget()) else {
            return 0;
        };
        let ctx = self.render_ctx(id, container);
        if let Some(w) = widget.content_width(&ctx) {
            return w.max(0);
        }
        match widget.render(&ctx) {
            Content::Empty => 0,
            Content::Text(t) => t.cell_len(),
            Content::Lines(lines) => lines.iter().map(|l| l.cell_len()).max().unwrap_or(0),
        }
    }

    /// Content height at the given content width (`Widget.get_content_height`).
    pub fn content_height(&self, id: NodeId, container: Size, width: i32) -> i32 {
        if width <= 0 {
            return 0;
        }
        if self.is_container(id) {
            if self.dom.children(id).is_empty() {
                return 0;
            }
            if self.layout_kind(id) == LayoutKind::Stream {
                return self
                    .arrange(id, Size::new(width, 0), true)
                    .total_region()
                    .height;
            }
            let st = self.styles(id);
            let all_dynamic = self
                .displayed_children(id)
                .into_iter()
                .all(|c| is_dynamic_height(self.styles(c)));
            let size = if st.dock.is_none() && all_dynamic {
                Size::new(width, container.height)
            } else {
                Size::new(width, 0)
            };
            return self.arrange(id, size, true).total_region().height;
        }
        let Some(widget) = self.dom.get(id).and_then(|n| n.widget()) else {
            return 0;
        };
        let ctx = self.render_ctx(id, Size::new(width, container.height));
        if let Some(h) = widget.content_height(&ctx, width) {
            return h.max(0);
        }
        match widget.render(&ctx) {
            Content::Empty => 0,
            Content::Text(t) => {
                if self.styles(id).text_wrap == Some(TextWrap::NoWrap) {
                    t.lines().len() as i32
                } else {
                    t.height_for_width(width)
                }
            }
            Content::Lines(lines) => lines.len() as i32,
        }
    }

    /// `Widget._get_box_model`.
    pub fn box_model(
        &self,
        id: NodeId,
        container: Size,
        width_fraction: f64,
        height_fraction: f64,
        constrain_width: bool,
        greedy: bool,
    ) -> BoxModel {
        let st = self.styles(id);
        let viewport = self.viewport;
        let border_box = st.get_box_sizing() == BoxSizing::BorderBox;
        let gutter = st.gutter();
        let margin = st.get_margin();

        let mut styles_width = st.width;
        if !greedy && matches!(styles_width, Some(w) if w.is_fraction()) {
            styles_width = Some(Scalar::AUTO);
        }

        let content_container = container.shrink(gutter);
        let (min_w, max_w, min_h, max_h) =
            self.resolve_extrema(st, container, width_fraction, height_fraction);
        let parent = self.parent_styles(id);

        // ---- width ----
        let mut content_width = match styles_width {
            None => (content_container.width - margin.width()) as f64,
            Some(w) if w.is_auto() => {
                let mut cw = self.content_width(id, content_container.shrink(margin)) as f64;
                // Textual adds the scrollbar if it is (or will always be) shown.
                if (st.get_overflow_x() == Overflow::Auto
                    && st.scrollbar_gutter == Some(ScrollbarGutter::Stable))
                    || st.get_overflow_y() == Overflow::Scroll
                {
                    cw += st.get_scrollbar_size_vertical() as f64;
                }
                if cw < content_container.width as f64 && self.has_relative_children_width(id) {
                    cw = content_container.width as f64;
                }
                cw
            }
            Some(w) => {
                let mut cw = resolve_scalar(
                    &w,
                    Dimension::Width,
                    container.shrink(margin),
                    viewport,
                    width_fraction,
                );
                if border_box {
                    cw -= gutter.width() as f64;
                }
                cw
            }
        };
        if let Some(m) = min_w {
            content_width = content_width.max(m).max(0.0);
        }
        if let Some(m) = max_w {
            let skip = container.width == 0
                && !st.max_width.map(|s| s.is_cells()).unwrap_or(true)
                && parent.map(|p| is_auto(p.width)).unwrap_or(false);
            if !skip {
                content_width = content_width.min(m);
            }
        }
        content_width = content_width.max(0.0);
        if constrain_width {
            content_width = content_width.min((container.width - gutter.width()) as f64);
        }

        // ---- height ----
        let mut content_height = match st.height {
            None => (content_container.height - margin.height()) as f64,
            Some(h) if h.is_auto() => {
                let mut ch =
                    self.content_height(id, content_container.shrink(margin), floor(content_width))
                        as f64;
                if (st.get_overflow_y() == Overflow::Auto
                    && st.scrollbar_gutter == Some(ScrollbarGutter::Stable))
                    || st.get_overflow_x() == Overflow::Scroll
                {
                    ch += st.get_scrollbar_size_horizontal() as f64;
                }
                if ch < content_container.height as f64 && self.has_relative_children_height(id) {
                    ch = content_container.height as f64;
                }
                ch
            }
            Some(h) => {
                let mut ch = resolve_scalar(
                    &h,
                    Dimension::Height,
                    container.shrink(margin),
                    viewport,
                    height_fraction,
                );
                if border_box {
                    ch -= gutter.height() as f64;
                }
                ch
            }
        };
        if let Some(m) = min_h {
            content_height = content_height.max(m).max(0.0);
        }
        if let Some(m) = max_h {
            let skip = container.height == 0
                && !st.max_height.map(|s| s.is_cells()).unwrap_or(true)
                && parent.map(|p| is_auto(p.height)).unwrap_or(false);
            if !skip {
                content_height = content_height.min(m);
            }
        }
        content_height = content_height.max(0.0);

        BoxModel {
            width: content_width + gutter.width() as f64,
            height: content_height + gutter.height() as f64,
            margin,
        }
    }

    /// `Widget._resolve_extrema`: min/max in content-box terms.
    fn resolve_extrema(
        &self,
        st: &Styles,
        container: Size,
        width_fraction: f64,
        height_fraction: f64,
    ) -> (Option<f64>, Option<f64>, Option<f64>, Option<f64>) {
        let container = container.shrink(st.get_margin());
        let (gw, gh) = if st.get_box_sizing() == BoxSizing::BorderBox {
            let g = st.gutter();
            (g.width() as f64, g.height() as f64)
        } else {
            (0.0, 0.0)
        };
        let vp = self.viewport;
        let r = |s: Option<Scalar>, dim, fr: f64, g: f64| {
            s.map(|s| resolve_scalar(&s, dim, container, vp, fr) - g)
        };
        (
            r(st.min_width, Dimension::Width, width_fraction, gw),
            r(st.max_width, Dimension::Width, width_fraction, gw),
            r(st.min_height, Dimension::Height, height_fraction, gh),
            r(st.max_height, Dimension::Height, height_fraction, gh),
        )
    }

    /// `_resolve.py::resolve_box_models`.
    pub fn resolve_box_models(
        &self,
        children: &[NodeId],
        size: Size,
        margin: Size,
        axis: Axis,
        greedy: bool,
    ) -> Vec<BoxModel> {
        let dimension = |c: NodeId| {
            let st = self.styles(c);
            match axis {
                Axis::Width => st.width,
                Axis::Height => st.height,
            }
        };
        let mut models: Vec<Option<BoxModel>> = children
            .iter()
            .map(|&c| {
                if matches!(dimension(c), Some(d) if d.is_fraction()) {
                    None
                } else {
                    let m = self.styles(c).get_margin();
                    let w = ((size.width - m.width()) as f64).max(0.0);
                    let h = ((size.height - m.height()) as f64).max(0.0);
                    Some(self.box_model(c, size, w, h, false, greedy))
                }
            })
            .collect();
        if models.iter().all(|m| m.is_some()) {
            return models.into_iter().flatten().collect();
        }
        let margin_size = Size::new(
            (size.width - margin.width).max(0),
            (size.height - margin.height).max(0),
        );
        let fr_children: Vec<NodeId> = children
            .iter()
            .copied()
            .filter(|c| matches!(dimension(*c), Some(d) if d.is_fraction()))
            .collect();
        let (width_fraction, height_fraction) = match axis {
            Axis::Width => {
                let total: f64 = models.iter().flatten().map(|m| m.width).sum();
                let remaining = (size.width - total as i32 - margin.width).max(0);
                let fu = self.resolve_fraction_unit(&fr_children, size, remaining as f64, axis);
                (fu, margin_size.height as f64)
            }
            Axis::Height => {
                let total: f64 = models.iter().flatten().map(|m| m.height).sum();
                let remaining = (size.height - total as i32 - margin.height).max(0);
                let fu = self.resolve_fraction_unit(&fr_children, size, remaining as f64, axis);
                (margin_size.width as f64, fu)
            }
        };
        for (m, &c) in models.iter_mut().zip(children) {
            if m.is_none() {
                *m = Some(self.box_model(c, size, width_fraction, height_fraction, false, greedy));
            }
        }
        models.into_iter().flatten().collect()
    }

    /// `_resolve.py::resolve_fraction_unit`: size of 1fr honouring min/max of fr widgets.
    fn resolve_fraction_unit(
        &self,
        children: &[NodeId],
        size: Size,
        remaining_space: f64,
        axis: Axis,
    ) -> f64 {
        if remaining_space == 0.0 || children.is_empty() {
            return 1.0;
        }
        let initial_space = remaining_space;
        let mut remaining_space = remaining_space;
        let vp = self.viewport;
        let dim = match axis {
            Axis::Width => Dimension::Width,
            Axis::Height => Dimension::Height,
        };
        let entries: Vec<(Scalar, Option<f64>, Option<f64>)> = children
            .iter()
            .map(|&c| {
                let st = self.styles(c);
                let (s, mn, mx) = match axis {
                    Axis::Width => (st.width, st.min_width, st.max_width),
                    Axis::Height => (st.height, st.min_height, st.max_height),
                };
                let r = |s: Option<Scalar>| s.map(|s| resolve_scalar(&s, dim, size, vp, 1.0));
                (s.unwrap_or(Scalar::fr(1.0)), r(mn), r(mx))
            })
            .collect();
        let mut resolved: Vec<Option<f64>> = vec![None; entries.len()];
        let mut remaining_fraction: f64 = entries.iter().map(|(s, _, _)| s.value as f64).sum();
        while remaining_fraction > 0.0 {
            let mut changed = false;
            let resolve_fraction = remaining_space / remaining_fraction;
            for (i, (scalar, min_v, max_v)) in entries.iter().enumerate() {
                if resolved[i].is_some() {
                    continue;
                }
                let value = resolve_scalar(scalar, dim, size, vp, resolve_fraction);
                if let Some(mn) = min_v.filter(|mn| value < *mn) {
                    remaining_space -= mn;
                    remaining_fraction -= scalar.value as f64;
                    resolved[i] = Some(mn);
                    changed = true;
                } else if let Some(mx) = max_v.filter(|mx| value > *mx) {
                    remaining_space -= mx;
                    remaining_fraction -= scalar.value as f64;
                    resolved[i] = Some(mx);
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
        if remaining_fraction > 0.0 {
            remaining_space / remaining_fraction
        } else {
            initial_space
        }
    }
}

/// `_resolve.py::resolve` (grid tracks): returns `(offset, length)` per dimension.
pub(crate) fn resolve_tracks(
    dimensions: &[Scalar],
    total: i32,
    gutter: i32,
    size: Size,
    viewport: Size,
    dim: Dimension,
) -> Vec<(i32, i32)> {
    let resolved: Vec<Option<f64>> = dimensions
        .iter()
        .map(|s| {
            if s.is_fraction() {
                None
            } else {
                Some(resolve_scalar(s, dim, size, viewport, 1.0))
            }
        })
        .collect();
    let total_fraction: f64 = dimensions
        .iter()
        .zip(&resolved)
        .filter(|(_, r)| r.is_none())
        .map(|(s, _)| s.value as f64)
        .sum();
    let total_gutter = gutter * (dimensions.len() as i32 - 1);
    let values: Vec<f64> = if total_fraction > 0.0 {
        let consumed: f64 = resolved.iter().flatten().sum();
        let remaining = ((total - total_gutter) as f64 - consumed).max(0.0);
        let unit = remaining / total_fraction;
        dimensions
            .iter()
            .zip(&resolved)
            .map(|(s, r)| r.unwrap_or(s.value as f64 * unit))
            .collect()
    } else {
        resolved.iter().map(|r| r.unwrap_or(0.0)).collect()
    };
    let mut out = Vec::with_capacity(values.len());
    let mut acc = 0.0;
    for v in values {
        let start = floor(acc);
        acc += v;
        let end = floor(acc);
        out.push((start, end - start));
        acc += gutter as f64;
    }
    out
}

/// Union where an empty region still contributes its corners (Textual's `Region.union`).
pub(crate) fn union_raw(a: Region, b: Region) -> Region {
    Region::from_corners(
        a.x.min(b.x),
        a.y.min(b.y),
        a.right().max(b.right()),
        a.bottom().max(b.bottom()),
    )
}

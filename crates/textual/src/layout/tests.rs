use super::*;
use crate::dom::Node;
use crate::widget::Widget;
use textual_css::{
    AlignH, AlignV, BorderEdge, BoxSizing, Display, Dock, LayoutKind, Overflow, Scalar,
    ScrollbarGutter, Styles, Visibility,
};
use textual_render::{BorderKind, Color, Spacing};

/// Test widget with explicit content measurements.
#[derive(Default)]
struct W {
    width: Option<i32>,
    height: Option<i32>,
    /// Text-like: `chars` cells of text that wrap to the width.
    chars: Option<i32>,
    virtual_size: Option<Size>,
}

impl Widget for W {
    fn content_width(&self, _ctx: &RenderCtx) -> Option<i32> {
        self.width.or(self.chars)
    }
    fn content_height(&self, _ctx: &RenderCtx, width: i32) -> Option<i32> {
        if let Some(c) = self.chars {
            return Some((c + width - 1) / width.max(1));
        }
        self.height
    }
    fn virtual_size(&self, _ctx: &RenderCtx) -> Option<Size> {
        self.virtual_size
    }
}

fn sized(w: i32, h: i32) -> W {
    W {
        width: Some(w),
        height: Some(h),
        ..W::default()
    }
}

struct T {
    dom: Dom,
    screen: NodeId,
}

impl T {
    fn new(screen_styles: Styles) -> T {
        let mut dom = Dom::new();
        let root = dom.insert(Node::new(Box::new(W::default())));
        dom.root = Some(root);
        let mut node = Node::new(Box::new(W::default()));
        node.is_screen = true;
        node.styles = screen_styles;
        let screen = dom.insert(node);
        dom.append_child(root, screen);
        T { dom, screen }
    }

    fn add_w(&mut self, parent: NodeId, w: W, styles: Styles) -> NodeId {
        let mut node = Node::new(Box::new(w));
        node.styles = styles;
        let id = self.dom.insert(node);
        self.dom.append_child(parent, id);
        id
    }

    fn add(&mut self, parent: NodeId, styles: Styles) -> NodeId {
        self.add_w(parent, W::default(), styles)
    }

    fn layout(&mut self, w: i32, h: i32) -> Arrangement {
        let env = RenderEnv::default();
        layout_screen(&mut self.dom, &env, self.screen, Size::new(w, h))
    }

    fn l(&self, id: NodeId) -> &NodeLayout {
        &self.dom.get(id).unwrap().layout
    }

    fn r(&self, id: NodeId) -> Region {
        self.l(id).region
    }
}

fn st() -> Styles {
    Styles::default()
}

fn cells(v: f32) -> Option<Scalar> {
    Some(Scalar::cells(v))
}

fn fr(v: f32) -> Option<Scalar> {
    Some(Scalar::fr(v))
}

fn wh(w: Option<Scalar>, h: Option<Scalar>) -> Styles {
    Styles {
        width: w,
        height: h,
        ..st()
    }
}

fn border_all(s: &mut Styles) {
    let e = Some(BorderEdge {
        kind: BorderKind::Solid,
        color: Color::WHITE,
    });
    s.border_top = e;
    s.border_right = e;
    s.border_bottom = e;
    s.border_left = e;
}

// ---------------------------------------------------------------- basics

#[test]
fn screen_fills_size() {
    let mut t = T::new(st());
    let a = t.layout(80, 24);
    assert_eq!(t.r(t.screen), Region::new(0, 0, 80, 24));
    assert_eq!(t.l(t.screen).content_region, Region::new(0, 0, 80, 24));
    assert_eq!(a.order, vec![t.screen]);
    assert_eq!(a.size, Size::new(80, 24));
}

#[test]
fn fill_defaults() {
    let mut t = T::new(st());
    let s = t.screen;
    let a = t.add(s, st());
    let b = t.add(s, st());
    let arr = t.layout(80, 24);
    // Unset width/height fill the container; the second child is pushed off-screen.
    assert_eq!(t.r(a), Region::new(0, 0, 80, 24));
    assert_eq!(t.r(b), Region::new(0, 24, 80, 24));
    assert!(t.l(a).visible);
    assert!(!t.l(b).visible);
    assert_eq!(arr.order, vec![s, a]);
}

#[test]
fn fixed_cells() {
    let mut t = T::new(st());
    let s = t.screen;
    let a = t.add(s, wh(cells(10.0), cells(3.0)));
    let b = t.add(s, wh(None, cells(5.0)));
    t.layout(80, 24);
    assert_eq!(t.r(a), Region::new(0, 0, 10, 3));
    assert_eq!(t.r(b), Region::new(0, 3, 80, 5));
}

#[test]
fn percent_sizes() {
    let mut t = T::new(st());
    let s = t.screen;
    let a = t.add(
        s,
        wh(Some(Scalar::percent(50.0)), Some(Scalar::percent(25.0))),
    );
    let b = t.add(
        s,
        wh(Some("50vw".parse().unwrap()), Some("50h".parse().unwrap())),
    );
    t.layout(80, 24);
    assert_eq!(t.r(a), Region::new(0, 0, 40, 6));
    assert_eq!(t.r(b), Region::new(0, 6, 40, 12));
}

#[test]
fn fr_distribution() {
    let mut t = T::new(st());
    let s = t.screen;
    let a = t.add(s, wh(None, fr(1.0)));
    let b = t.add(s, wh(None, cells(6.0)));
    let c = t.add(s, wh(None, fr(2.0)));
    t.layout(80, 24);
    assert_eq!(t.r(a), Region::new(0, 0, 80, 6));
    assert_eq!(t.r(b), Region::new(0, 6, 80, 6));
    assert_eq!(t.r(c), Region::new(0, 12, 80, 12));
}

#[test]
fn fr_rounding_fills_exactly() {
    let mut t = T::new(st());
    let s = t.screen;
    let ids: Vec<_> = (0..3).map(|_| t.add(s, wh(None, fr(1.0)))).collect();
    t.layout(80, 10);
    let rs: Vec<_> = ids.iter().map(|i| t.r(*i)).collect();
    assert_eq!(rs[0], Region::new(0, 0, 80, 3));
    assert_eq!(rs[1], Region::new(0, 3, 80, 3));
    assert_eq!(rs[2], Region::new(0, 6, 80, 4));
}

#[test]
fn fr_with_max() {
    let mut t = T::new(st());
    let s = t.screen;
    let a = t.add(
        s,
        Styles {
            max_height: cells(4.0),
            ..wh(None, fr(1.0))
        },
    );
    let b = t.add(s, wh(None, fr(1.0)));
    t.layout(80, 24);
    assert_eq!(t.r(a), Region::new(0, 0, 80, 4));
    assert_eq!(t.r(b), Region::new(0, 4, 80, 20));
}

#[test]
fn fr_with_min() {
    let mut t = T::new(st());
    let s = t.screen;
    let a = t.add(
        s,
        Styles {
            min_height: cells(10.0),
            ..wh(None, fr(1.0))
        },
    );
    let b = t.add(s, wh(None, fr(3.0)));
    t.layout(80, 24);
    assert_eq!(t.r(a), Region::new(0, 0, 80, 10));
    assert_eq!(t.r(b), Region::new(0, 10, 80, 14));
}

#[test]
fn min_max_on_fixed() {
    let mut t = T::new(st());
    let s = t.screen;
    let a = t.add(
        s,
        Styles {
            max_width: cells(30.0),
            min_height: cells(4.0),
            ..wh(cells(50.0), cells(2.0))
        },
    );
    t.layout(80, 24);
    assert_eq!(t.r(a), Region::new(0, 0, 30, 4));
}

// ---------------------------------------------------------------- box model

#[test]
fn padding_border_gutter() {
    let mut t = T::new(st());
    let s = t.screen;
    let mut styles = wh(cells(20.0), cells(10.0));
    styles.padding = Some(Spacing::new(1, 2, 1, 2));
    border_all(&mut styles);
    let a = t.add(s, styles);
    t.layout(80, 24);
    // border-box: the 20x10 includes padding + border.
    assert_eq!(t.r(a), Region::new(0, 0, 20, 10));
    assert_eq!(t.l(a).content_region, Region::new(3, 2, 14, 6));
}

#[test]
fn content_box_sizing() {
    let mut t = T::new(st());
    let s = t.screen;
    let mut styles = wh(cells(10.0), cells(3.0));
    styles.padding = Some(Spacing::all(1));
    styles.box_sizing = Some(BoxSizing::ContentBox);
    let a = t.add(s, styles);
    t.layout(80, 24);
    assert_eq!(t.r(a), Region::new(0, 0, 12, 5));
    assert_eq!(t.l(a).content_region, Region::new(1, 1, 10, 3));
}

#[test]
fn hidden_border_takes_no_space() {
    let mut t = T::new(st());
    let s = t.screen;
    let mut styles = wh(cells(10.0), cells(3.0));
    styles.border_top = Some(BorderEdge {
        kind: BorderKind::Hidden,
        color: Color::WHITE,
    });
    styles.border_left = Some(BorderEdge {
        kind: BorderKind::Blank,
        color: Color::WHITE,
    });
    let a = t.add(s, styles);
    t.layout(80, 24);
    assert_eq!(t.l(a).content_region, Region::new(1, 0, 9, 3));
}

#[test]
fn auto_height_leaf() {
    let mut t = T::new(st());
    let s = t.screen;
    let mut styles = wh(None, Some(Scalar::AUTO));
    styles.padding = Some(Spacing::all(1));
    border_all(&mut styles);
    let a = t.add_w(s, sized(5, 3), styles);
    // Text-like widget: 100 chars wrap at the content width (80 - 0) → 2 lines.
    let b = t.add_w(
        s,
        W {
            chars: Some(100),
            ..W::default()
        },
        wh(None, Some(Scalar::AUTO)),
    );
    t.layout(80, 24);
    assert_eq!(t.r(a), Region::new(0, 0, 80, 7));
    assert_eq!(t.l(a).content_region, Region::new(2, 2, 76, 3));
    assert_eq!(t.r(b), Region::new(0, 7, 80, 2));
}

#[test]
fn auto_width_leaf_in_horizontal() {
    let mut t = T::new(Styles {
        layout: Some(LayoutKind::Horizontal),
        ..st()
    });
    let s = t.screen;
    let mut styles = wh(Some(Scalar::AUTO), Some(Scalar::AUTO));
    styles.padding = Some(Spacing::symmetric(0, 1));
    let a = t.add_w(s, sized(7, 2), styles);
    let b = t.add_w(s, sized(3, 1), wh(Some(Scalar::AUTO), None));
    t.layout(80, 24);
    assert_eq!(t.r(a), Region::new(0, 0, 9, 2));
    assert_eq!(t.r(b), Region::new(9, 0, 3, 24));
}

#[test]
fn auto_height_container() {
    let mut t = T::new(st());
    let s = t.screen;
    let mut cs = wh(None, Some(Scalar::AUTO));
    border_all(&mut cs);
    let c = t.add(s, cs);
    let a = t.add(
        c,
        Styles {
            margin: Some(Spacing::new(0, 0, 1, 0)),
            ..wh(None, cells(2.0))
        },
    );
    let b = t.add(
        c,
        Styles {
            margin: Some(Spacing::new(1, 0, 0, 0)),
            ..wh(None, cells(3.0))
        },
    );
    let after = t.add(s, wh(None, cells(1.0)));
    t.layout(80, 24);
    // 2 + collapsed margin 1 + 3 = 6, plus border 2.
    assert_eq!(t.r(c), Region::new(0, 0, 80, 8));
    assert_eq!(t.r(a), Region::new(1, 1, 78, 2));
    assert_eq!(t.r(b), Region::new(1, 4, 78, 3));
    assert_eq!(t.r(after), Region::new(0, 8, 80, 1));
}

#[test]
fn nested_auto_height_containers() {
    let mut t = T::new(st());
    let s = t.screen;
    let outer = t.add(s, wh(None, Some(Scalar::AUTO)));
    let inner = t.add(outer, wh(None, Some(Scalar::AUTO)));
    let leaf = t.add_w(
        inner,
        W {
            chars: Some(30),
            ..W::default()
        },
        wh(cells(10.0), Some(Scalar::AUTO)),
    );
    t.layout(40, 24);
    assert_eq!(t.r(leaf), Region::new(0, 0, 10, 3));
    assert_eq!(t.r(inner), Region::new(0, 0, 40, 3));
    assert_eq!(t.r(outer), Region::new(0, 0, 40, 3));
}

#[test]
fn auto_width_container_treats_fr_as_auto() {
    let mut t = T::new(st());
    let s = t.screen;
    let c = t.add(
        s,
        Styles {
            layout: Some(LayoutKind::Horizontal),
            ..wh(Some(Scalar::AUTO), cells(3.0))
        },
    );
    let a = t.add(c, wh(cells(5.0), None));
    let b = t.add_w(c, sized(4, 1), wh(fr(1.0), None));
    t.layout(80, 24);
    // Textual (`_has_relative_children_width`): an auto-width container with relative
    // children expands to the available width.
    assert_eq!(t.r(c), Region::new(0, 0, 80, 3));
    assert_eq!(t.r(a), Region::new(0, 0, 5, 3));
    assert_eq!(t.r(b), Region::new(5, 0, 75, 3));
}

#[test]
fn auto_width_container_shrink_wraps() {
    let mut t = T::new(st());
    let s = t.screen;
    let mut cs = Styles {
        layout: Some(LayoutKind::Horizontal),
        ..wh(Some(Scalar::AUTO), cells(3.0))
    };
    border_all(&mut cs);
    let c = t.add(s, cs);
    let a = t.add(
        c,
        Styles {
            margin: Some(Spacing::new(0, 1, 0, 0)),
            ..wh(cells(5.0), None)
        },
    );
    let b = t.add_w(c, sized(4, 1), wh(Some(Scalar::AUTO), None));
    t.layout(80, 24);
    assert_eq!(t.r(c), Region::new(0, 0, 12, 3));
    assert_eq!(t.r(a), Region::new(1, 1, 5, 1));
    assert_eq!(t.r(b), Region::new(7, 1, 4, 1));
}

// ---------------------------------------------------------------- margins

#[test]
fn vertical_margins_collapse() {
    let mut t = T::new(st());
    let s = t.screen;
    let a = t.add(
        s,
        Styles {
            margin: Some(Spacing::new(1, 2, 3, 4)),
            ..wh(None, cells(2.0))
        },
    );
    let b = t.add(
        s,
        Styles {
            margin: Some(Spacing::new(5, 0, 0, 0)),
            ..wh(None, cells(2.0))
        },
    );
    t.layout(80, 24);
    assert_eq!(t.r(a), Region::new(4, 1, 74, 2));
    // max(3, 5) = 5 between them.
    assert_eq!(t.r(b), Region::new(0, 8, 80, 2));
}

#[test]
fn vertical_fill_with_margin() {
    let mut t = T::new(st());
    let s = t.screen;
    let a = t.add(
        s,
        Styles {
            margin: Some(Spacing::all(2)),
            ..st()
        },
    );
    t.layout(80, 24);
    assert_eq!(t.r(a), Region::new(2, 2, 76, 20));
}

#[test]
fn horizontal_margins_collapse() {
    let mut t = T::new(Styles {
        layout: Some(LayoutKind::Horizontal),
        ..st()
    });
    let s = t.screen;
    let a = t.add(
        s,
        Styles {
            margin: Some(Spacing::new(1, 2, 0, 1)),
            ..wh(cells(10.0), None)
        },
    );
    let b = t.add(
        s,
        Styles {
            margin: Some(Spacing::new(0, 0, 0, 3)),
            ..wh(cells(10.0), cells(2.0))
        },
    );
    t.layout(80, 24);
    assert_eq!(t.r(a), Region::new(1, 1, 10, 23));
    assert_eq!(t.r(b), Region::new(14, 0, 10, 2));
}

// ---------------------------------------------------------------- horizontal

#[test]
fn horizontal_layout_fr() {
    let mut t = T::new(Styles {
        layout: Some(LayoutKind::Horizontal),
        ..st()
    });
    let s = t.screen;
    let a = t.add(s, wh(cells(10.0), None));
    let b = t.add(s, wh(fr(1.0), None));
    let c = t.add(s, wh(cells(20.0), cells(5.0)));
    t.layout(80, 24);
    assert_eq!(t.r(a), Region::new(0, 0, 10, 24));
    assert_eq!(t.r(b), Region::new(10, 0, 50, 24));
    assert_eq!(t.r(c), Region::new(60, 0, 20, 5));
}

// ---------------------------------------------------------------- dock

#[test]
fn docking_edges() {
    let mut t = T::new(st());
    let s = t.screen;
    let body = t.add(s, st());
    let header = t.add(
        s,
        Styles {
            dock: Some(Dock::Top),
            ..wh(None, cells(1.0))
        },
    );
    let footer = t.add(
        s,
        Styles {
            dock: Some(Dock::Bottom),
            ..wh(None, cells(1.0))
        },
    );
    let side = t.add(
        s,
        Styles {
            dock: Some(Dock::Left),
            ..wh(cells(10.0), None)
        },
    );
    let right = t.add(
        s,
        Styles {
            dock: Some(Dock::Right),
            ..wh(cells(5.0), cells(4.0))
        },
    );
    let arr = t.layout(80, 24);
    assert_eq!(t.r(header), Region::new(0, 0, 80, 1));
    assert_eq!(t.r(footer), Region::new(0, 23, 80, 1));
    // Docks are sized against the whole region (they overlap each other, like Textual).
    assert_eq!(t.r(side), Region::new(0, 0, 10, 24));
    assert_eq!(t.r(right), Region::new(75, 0, 5, 4));
    assert_eq!(t.r(body), Region::new(10, 1, 65, 22));
    // Docked widgets paint over the flow widgets, in DOM order.
    assert_eq!(arr.order, vec![s, body, header, footer, side, right]);
    assert_eq!(arr.node_at(&t.dom, 0, 0), Some(side));
    assert_eq!(arr.node_at(&t.dom, 40, 0), Some(header));
    assert_eq!(arr.node_at(&t.dom, 40, 10), Some(body));
}

#[test]
fn dock_with_margin() {
    let mut t = T::new(st());
    let s = t.screen;
    let d = t.add(
        s,
        Styles {
            dock: Some(Dock::Bottom),
            margin: Some(Spacing::all(1)),
            ..wh(None, cells(2.0))
        },
    );
    let body = t.add(s, st());
    t.layout(80, 24);
    assert_eq!(t.r(d), Region::new(1, 21, 78, 2));
    assert_eq!(t.r(body), Region::new(0, 0, 80, 20));
}

#[test]
fn docked_widgets_do_not_scroll() {
    let mut t = T::new(st());
    let s = t.screen;
    let c = t.add(
        s,
        Styles {
            overflow_y: Some(Overflow::Auto),
            ..wh(cells(20.0), cells(10.0))
        },
    );
    let head = t.add(
        c,
        Styles {
            dock: Some(Dock::Top),
            ..wh(None, cells(1.0))
        },
    );
    let kids: Vec<_> = (0..6).map(|_| t.add(c, wh(None, cells(3.0)))).collect();
    t.dom.get_mut(c).unwrap().scroll_offset = Offset::new(0, 4);
    t.layout(80, 24);
    let l = t.l(c).clone();
    assert!(l.scrollbar_v);
    // 1 (dock) + 18 rows of content.
    assert_eq!(l.virtual_size, Size::new(18, 19));
    assert_eq!(t.dom.get(c).unwrap().scroll_offset, Offset::new(0, 4));
    assert_eq!(t.r(head), Region::new(0, 0, 18, 1));
    assert_eq!(t.r(kids[0]), Region::new(0, -3, 18, 3));
    assert_eq!(t.r(kids[1]), Region::new(0, 0, 18, 3));
}

// ---------------------------------------------------------------- grid

#[test]
fn grid_spans_and_gutter() {
    let mut t = T::new(st());
    let s = t.screen;
    let g = t.add(
        s,
        Styles {
            layout: Some(LayoutKind::Grid),
            grid_size_columns: Some(3),
            grid_gutter_horizontal: Some(1),
            grid_gutter_vertical: Some(1),
            ..wh(cells(11.0), cells(11.0))
        },
    );
    let c0 = t.add(
        g,
        Styles {
            column_span: Some(2),
            ..st()
        },
    );
    let c1 = t.add(g, st());
    let c2 = t.add(
        g,
        Styles {
            row_span: Some(2),
            ..st()
        },
    );
    let c3 = t.add(g, st());
    let c4 = t.add(g, st());
    let c5 = t.add(g, st());
    t.layout(80, 24);
    assert_eq!(t.r(c0), Region::new(0, 0, 7, 3));
    assert_eq!(t.r(c1), Region::new(8, 0, 3, 3));
    assert_eq!(t.r(c2), Region::new(0, 4, 3, 7));
    assert_eq!(t.r(c3), Region::new(4, 4, 3, 3));
    assert_eq!(t.r(c4), Region::new(8, 4, 3, 3));
    assert_eq!(t.r(c5), Region::new(4, 8, 3, 3));
}

#[test]
fn grid_columns_rows_repeat_and_auto() {
    let mut t = T::new(st());
    let s = t.screen;
    let g = t.add(
        s,
        Styles {
            layout: Some(LayoutKind::Grid),
            grid_size_columns: Some(3),
            grid_columns: Some(vec![Scalar::cells(10.0), Scalar::fr(1.0)]),
            grid_rows: Some(vec![Scalar::AUTO]),
            ..wh(cells(40.0), None)
        },
    );
    let a = t.add_w(g, sized(3, 2), wh(None, Some(Scalar::AUTO)));
    let b = t.add_w(g, sized(3, 4), wh(None, Some(Scalar::AUTO)));
    let c = t.add_w(g, sized(3, 1), wh(None, Some(Scalar::AUTO)));
    let d = t.add_w(g, sized(3, 1), wh(None, Some(Scalar::AUTO)));
    t.layout(80, 24);
    // Columns: 10, 1fr, 10 (repeated) → 10, 20, 10.
    assert_eq!(t.r(a), Region::new(0, 0, 10, 2));
    assert_eq!(t.r(b), Region::new(10, 0, 20, 4));
    assert_eq!(t.r(c), Region::new(30, 0, 10, 1));
    // Auto row 0 is 4 tall.
    assert_eq!(t.r(d), Region::new(0, 4, 10, 1));
}

#[test]
fn grid_auto_column_and_limited_rows() {
    let mut t = T::new(st());
    let s = t.screen;
    let g = t.add(
        s,
        Styles {
            layout: Some(LayoutKind::Grid),
            grid_size_columns: Some(2),
            grid_size_rows: Some(1),
            grid_columns: Some(vec![Scalar::AUTO, Scalar::fr(1.0)]),
            ..wh(cells(30.0), cells(6.0))
        },
    );
    let a = t.add_w(g, sized(7, 1), st());
    let b = t.add(g, st());
    let hidden = t.add(g, st());
    t.layout(80, 24);
    assert_eq!(t.r(a), Region::new(0, 0, 7, 6));
    assert_eq!(t.r(b), Region::new(7, 0, 23, 6));
    // Does not fit the 2x1 grid: not placed.
    assert!(!t.l(hidden).visible);
}

// ---------------------------------------------------------------- display / visibility

#[test]
fn display_none_and_visibility_hidden() {
    let mut t = T::new(st());
    let s = t.screen;
    let gone = t.add(
        s,
        Styles {
            display: Some(Display::None),
            ..wh(None, cells(5.0))
        },
    );
    let gone_child = t.add(gone, st());
    let hidden = t.add(
        s,
        Styles {
            visibility: Some(Visibility::Hidden),
            ..wh(None, cells(5.0))
        },
    );
    let hidden_child = t.add(hidden, wh(cells(5.0), cells(1.0)));
    let shown_child = t.add(
        hidden,
        Styles {
            visibility: Some(Visibility::Visible),
            ..wh(cells(5.0), cells(1.0))
        },
    );
    let after = t.add(s, wh(None, cells(1.0)));
    let arr = t.layout(80, 24);
    assert!(!t.l(gone).visible);
    assert!(!t.l(gone_child).visible);
    assert_eq!(t.r(hidden), Region::new(0, 0, 80, 5));
    assert!(!t.l(hidden).visible);
    assert!(!t.l(hidden_child).visible);
    assert!(t.l(shown_child).visible);
    assert_eq!(t.r(shown_child), Region::new(0, 1, 5, 1));
    assert_eq!(t.r(after), Region::new(0, 5, 80, 1));
    assert_eq!(arr.order, vec![s, shown_child, after]);
}

#[test]
fn relayout_resets_previously_visible() {
    let mut t = T::new(st());
    let s = t.screen;
    let a = t.add(s, wh(None, cells(2.0)));
    t.layout(80, 24);
    assert!(t.l(a).visible);
    t.dom.get_mut(a).unwrap().styles.display = Some(Display::None);
    let arr = t.layout(80, 24);
    assert_eq!(*t.l(a), NodeLayout::default());
    assert_eq!(arr.order, vec![s]);
}

// ---------------------------------------------------------------- scrolling

fn scroll_container(
    t: &mut T,
    ox: Overflow,
    oy: Overflow,
    child_w: Option<Scalar>,
) -> (NodeId, Vec<NodeId>) {
    let s = t.screen;
    let c = t.add(
        s,
        Styles {
            overflow_x: Some(ox),
            overflow_y: Some(oy),
            ..wh(cells(20.0), cells(10.0))
        },
    );
    let kids = (0..5).map(|_| t.add(c, wh(child_w, cells(3.0)))).collect();
    (c, kids)
}

#[test]
fn overflow_auto_vertical_scrollbar_and_clamp() {
    let mut t = T::new(st());
    let (c, kids) = scroll_container(&mut t, Overflow::Hidden, Overflow::Auto, None);
    t.dom.get_mut(c).unwrap().scroll_offset = Offset::new(3, 100);
    let arr = t.layout(80, 24);
    let l = t.l(c).clone();
    assert!(l.scrollbar_v);
    assert!(!l.scrollbar_h);
    assert_eq!(l.content_region, Region::new(0, 0, 18, 10));
    assert_eq!(l.virtual_size, Size::new(18, 15));
    assert_eq!(l.max_scroll(), Offset::new(0, 5));
    assert_eq!(t.dom.get(c).unwrap().scroll_offset, Offset::new(0, 5));
    // Children are translated by -scroll and clipped to the viewport.
    assert_eq!(t.r(kids[0]), Region::new(0, -5, 18, 3));
    assert!(!t.l(kids[0]).visible);
    assert_eq!(t.r(kids[1]), Region::new(0, -2, 18, 3));
    assert_eq!(t.l(kids[1]).clip, Region::new(0, 0, 18, 1));
    assert_eq!(t.r(kids[4]), Region::new(0, 7, 18, 3));
    assert!(!arr.order.contains(&kids[0]));
    assert!(arr.order.contains(&kids[1]));
}

#[test]
fn overflow_auto_not_needed() {
    let mut t = T::new(st());
    let s = t.screen;
    let c = t.add(
        s,
        Styles {
            overflow_x: Some(Overflow::Auto),
            overflow_y: Some(Overflow::Auto),
            ..wh(cells(20.0), cells(10.0))
        },
    );
    t.add(c, wh(None, cells(3.0)));
    t.dom.get_mut(c).unwrap().scroll_offset = Offset::new(2, 2);
    t.layout(80, 24);
    let l = t.l(c);
    assert!(!l.scrollbar_v && !l.scrollbar_h);
    assert_eq!(l.virtual_size, Size::new(20, 10));
    assert_eq!(t.dom.get(c).unwrap().scroll_offset, Offset::ZERO);
}

#[test]
fn overflow_auto_both_scrollbars() {
    let mut t = T::new(st());
    let (c, kids) = scroll_container(&mut t, Overflow::Auto, Overflow::Auto, cells(30.0));
    t.dom.get_mut(c).unwrap().scroll_offset = Offset::new(50, 50);
    t.layout(80, 24);
    let l = t.l(c).clone();
    assert!(l.scrollbar_v && l.scrollbar_h);
    assert_eq!(l.content_region, Region::new(0, 0, 18, 9));
    assert_eq!(l.virtual_size, Size::new(30, 15));
    assert_eq!(t.dom.get(c).unwrap().scroll_offset, Offset::new(12, 6));
    assert_eq!(t.r(kids[2]), Region::new(-12, 0, 30, 3));
    assert_eq!(t.l(kids[2]).clip, Region::new(0, 0, 18, 3));
}

#[test]
fn horizontal_scrollbar_triggers_vertical() {
    // Content is 9 rows tall: fits 10 rows, but not the 9 left by the horizontal bar.
    let mut t = T::new(st());
    let s = t.screen;
    let c = t.add(
        s,
        Styles {
            overflow_x: Some(Overflow::Auto),
            overflow_y: Some(Overflow::Auto),
            scrollbar_size_horizontal: Some(2),
            ..wh(cells(20.0), cells(10.0))
        },
    );
    t.add(c, wh(cells(30.0), cells(9.0)));
    t.layout(80, 24);
    let l = t.l(c);
    assert!(l.scrollbar_h);
    assert!(l.scrollbar_v);
    assert_eq!(l.content_region, Region::new(0, 0, 18, 8));
}

#[test]
fn overflow_scroll_always_reserves() {
    let mut t = T::new(st());
    let s = t.screen;
    let c = t.add(
        s,
        Styles {
            overflow_x: Some(Overflow::Scroll),
            overflow_y: Some(Overflow::Scroll),
            scrollbar_size_vertical: Some(1),
            ..wh(cells(20.0), cells(10.0))
        },
    );
    let k = t.add(c, st());
    t.layout(80, 24);
    let l = t.l(c);
    assert!(l.scrollbar_v && l.scrollbar_h);
    assert_eq!(l.content_region, Region::new(0, 0, 19, 9));
    assert_eq!(t.r(k), Region::new(0, 0, 19, 9));
}

#[test]
fn scrollbar_gutter_stable() {
    let mut t = T::new(st());
    let s = t.screen;
    let c = t.add(
        s,
        Styles {
            overflow_y: Some(Overflow::Auto),
            scrollbar_gutter: Some(ScrollbarGutter::Stable),
            ..wh(cells(20.0), cells(10.0))
        },
    );
    let k = t.add(c, wh(None, cells(1.0)));
    t.layout(80, 24);
    assert!(!t.l(c).scrollbar_v);
    assert_eq!(t.r(k), Region::new(0, 0, 18, 1));
}

#[test]
fn scrollbars_inside_border() {
    let mut t = T::new(st());
    let s = t.screen;
    let mut cs = Styles {
        overflow_y: Some(Overflow::Auto),
        ..wh(cells(20.0), cells(10.0))
    };
    border_all(&mut cs);
    let c = t.add(s, cs);
    t.add(c, wh(None, cells(30.0)));
    t.layout(80, 24);
    let l = t.l(c);
    assert!(l.scrollbar_v);
    assert_eq!(l.content_region, Region::new(1, 1, 16, 8));
    assert_eq!(l.max_scroll(), Offset::new(0, 22));
}

#[test]
fn self_scrolling_widget() {
    let mut t = T::new(st());
    let s = t.screen;
    let w = t.add_w(
        s,
        W {
            virtual_size: Some(Size::new(100, 50)),
            ..W::default()
        },
        Styles {
            overflow_x: Some(Overflow::Auto),
            overflow_y: Some(Overflow::Auto),
            ..wh(cells(20.0), cells(10.0))
        },
    );
    t.dom.get_mut(w).unwrap().scroll_offset = Offset::new(1000, 7);
    t.layout(80, 24);
    let l = t.l(w);
    assert!(l.scrollbar_v && l.scrollbar_h);
    assert_eq!(l.virtual_size, Size::new(100, 50));
    assert_eq!(l.content_region, Region::new(0, 0, 18, 9));
    assert_eq!(t.dom.get(w).unwrap().scroll_offset, Offset::new(82, 7));
}

#[test]
fn leaf_scroll_offset_clamped_to_zero() {
    let mut t = T::new(st());
    let s = t.screen;
    let a = t.add(s, wh(None, cells(3.0)));
    t.dom.get_mut(a).unwrap().scroll_offset = Offset::new(4, 4);
    t.layout(80, 24);
    assert_eq!(t.dom.get(a).unwrap().scroll_offset, Offset::ZERO);
}

// ---------------------------------------------------------------- clipping / offset / align

#[test]
fn nested_clipping() {
    let mut t = T::new(st());
    let s = t.screen;
    let outer = t.add(s, wh(cells(10.0), cells(5.0)));
    let inner = t.add(outer, wh(cells(20.0), cells(3.0)));
    let mut gs = wh(cells(4.0), cells(1.0));
    gs.offset_x = cells(8.0);
    let grand = t.add(inner, gs);
    let far = t.add(
        inner,
        Styles {
            offset_x: cells(12.0),
            ..wh(cells(4.0), cells(1.0))
        },
    );
    t.layout(80, 24);
    assert_eq!(t.r(inner), Region::new(0, 0, 20, 3));
    assert_eq!(t.l(inner).clip, Region::new(0, 0, 10, 3));
    assert_eq!(t.r(grand), Region::new(8, 0, 4, 1));
    assert_eq!(t.l(grand).clip, Region::new(8, 0, 2, 1));
    assert_eq!(t.r(far), Region::new(12, 1, 4, 1));
    assert!(!t.l(far).visible);
}

#[test]
fn offset_does_not_affect_siblings() {
    let mut t = T::new(st());
    let s = t.screen;
    let a = t.add(
        s,
        Styles {
            offset_x: cells(3.0),
            offset_y: cells(2.0),
            ..wh(cells(10.0), cells(2.0))
        },
    );
    let b = t.add(
        s,
        Styles {
            offset_x: Some(Scalar::percent(50.0)),
            ..wh(cells(10.0), cells(2.0))
        },
    );
    t.layout(80, 24);
    assert_eq!(t.r(a), Region::new(3, 2, 10, 2));
    // Percent offsets resolve against the widget's own size.
    assert_eq!(t.r(b), Region::new(5, 2, 10, 2));
}

#[test]
fn align_center_middle() {
    let mut t = T::new(Styles {
        align_horizontal: Some(AlignH::Center),
        align_vertical: Some(AlignV::Middle),
        ..st()
    });
    let s = t.screen;
    let a = t.add(s, wh(cells(20.0), cells(4.0)));
    let b = t.add(s, wh(cells(10.0), cells(4.0)));
    t.layout(80, 24);
    // The stack is aligned as a group (bounding box 20x8).
    assert_eq!(t.r(a), Region::new(30, 8, 20, 4));
    assert_eq!(t.r(b), Region::new(30, 12, 10, 4));
}

#[test]
fn align_right_bottom_in_padded_container() {
    let mut t = T::new(st());
    let s = t.screen;
    let c = t.add(
        s,
        Styles {
            align_horizontal: Some(AlignH::Right),
            align_vertical: Some(AlignV::Bottom),
            padding: Some(Spacing::all(1)),
            ..wh(cells(20.0), cells(10.0))
        },
    );
    let a = t.add(c, wh(cells(5.0), cells(2.0)));
    t.layout(80, 24);
    assert_eq!(t.r(a), Region::new(14, 7, 5, 2));
}

// ---------------------------------------------------------------- layers / hit testing

#[test]
fn layers_ordering() {
    let mut t = T::new(Styles {
        layers: Some(vec!["below".into(), "above".into()]),
        ..st()
    });
    let s = t.screen;
    let top = t.add(
        s,
        Styles {
            layer: Some("above".into()),
            ..wh(cells(10.0), cells(3.0))
        },
    );
    let bottom = t.add(
        s,
        Styles {
            layer: Some("below".into()),
            ..wh(cells(20.0), cells(5.0))
        },
    );
    let bottom2 = t.add(
        s,
        Styles {
            layer: Some("below".into()),
            ..wh(cells(20.0), cells(5.0))
        },
    );
    let top_child = t.add(top, wh(cells(2.0), cells(1.0)));
    let arr = t.layout(80, 24);
    // Each layer is arranged independently: both layers start at the top.
    assert_eq!(t.r(top), Region::new(0, 0, 10, 3));
    assert_eq!(t.r(bottom), Region::new(0, 0, 20, 5));
    assert_eq!(t.r(bottom2), Region::new(0, 5, 20, 5));
    assert_eq!(arr.order, vec![s, bottom, bottom2, top, top_child]);
    assert_eq!(arr.node_at(&t.dom, 1, 0), Some(top_child));
    assert_eq!(arr.node_at(&t.dom, 5, 1), Some(top));
    assert_eq!(arr.node_at(&t.dom, 15, 1), Some(bottom));
    assert_eq!(arr.node_at(&t.dom, 15, 6), Some(bottom2));
    assert_eq!(arr.node_at(&t.dom, 50, 20), Some(s));
    assert_eq!(arr.node_at(&t.dom, 90, 20), None);
}

#[test]
fn layers_defined_on_outer_ancestor_apply() {
    let mut t = T::new(Styles {
        layers: Some(vec!["a".into(), "b".into()]),
        ..st()
    });
    let s = t.screen;
    let c = t.add(s, st());
    let x = t.add(
        c,
        Styles {
            layer: Some("b".into()),
            ..wh(cells(5.0), cells(5.0))
        },
    );
    let y = t.add(
        c,
        Styles {
            layer: Some("a".into()),
            ..wh(cells(5.0), cells(5.0))
        },
    );
    let arr = t.layout(80, 24);
    assert_eq!(arr.order, vec![s, c, y, x]);
    assert_eq!(arr.node_at(&t.dom, 2, 2), Some(x));
}

#[test]
fn node_at_respects_scroll_clip() {
    let mut t = T::new(st());
    let (c, kids) = scroll_container(&mut t, Overflow::Hidden, Overflow::Auto, None);
    t.dom.get_mut(c).unwrap().scroll_offset = Offset::new(0, 4);
    let arr = t.layout(80, 24);
    // kids[1] spans y -1..2.
    assert_eq!(arr.node_at(&t.dom, 0, 0), Some(kids[1]));
    assert_eq!(arr.node_at(&t.dom, 0, 2), Some(kids[2]));
    // Scrollbar column belongs to the container.
    assert_eq!(arr.node_at(&t.dom, 19, 0), Some(c));
    // Below the container: the screen.
    assert_eq!(arr.node_at(&t.dom, 0, 12), Some(t.screen));
}

#[test]
fn screen_padding_and_border() {
    let mut ss = Styles {
        padding: Some(Spacing::symmetric(1, 2)),
        ..st()
    };
    border_all(&mut ss);
    let mut t = T::new(ss);
    let s = t.screen;
    let a = t.add(s, st());
    t.layout(80, 24);
    assert_eq!(t.l(s).content_region, Region::new(3, 2, 74, 20));
    assert_eq!(t.r(a), Region::new(3, 2, 74, 20));
}

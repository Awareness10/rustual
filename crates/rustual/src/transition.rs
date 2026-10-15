//! CSS `transition` runtime (port of the animated branch of Textual's
//! `Stylesheet.replace_rules`).
//!
//! Every frame `AppState::update_styles` recomputes each node's styles
//! from scratch. For nodes whose new styles declare `transition`s, `apply` compares each
//! transitioned property of the new styles (the *target*) with the value displayed in the
//! previous frame; when the target changed, a transition starts from the displayed value to the
//! new target. Running transitions then overwrite the freshly computed styles with the
//! interpolated value (an overlay applied after the cascade and inline styles, before
//! inheritance — so children inherit e.g. an animating `color`), which the layout and
//! compositor see like any other style.
//!
//! Like Textual:
//! - a node's very first styling is never animated;
//! - the transition of the *new* styles is used (`:hover { transition: .. }` animates the
//!   hover-in, not the hover-out);
//! - changing the target while a transition runs restarts it from the current value;
//! - with an [`AnimationLevel`] below `full`, values jump.
//!
//! Supported properties: `opacity`, `text-opacity`, `offset`, `width`, `height`,
//! `min-width`/`min-height`/`max-width`/`max-height`, `margin`, `padding`, `color`,
//! `background`, `background-tint`, `tint`, `border` / `border-top|right|bottom|left` (the
//! color; the border kind changes immediately), the `scrollbar-*` colors and the `link-*`
//! colors. Properties set by inline styles (e.g. by [`Ctx::animate_style`](crate::Ctx)) are not
//! transitioned: inline styles win anyway.

use crate::animation::{AnimationLevel, Animator, Easing};
use crate::dom::{Dom, NodeId};
use rustual_css::{BorderEdge, ColorValue, Dimension, Scalar, Styles, Transition, Unit};
use rustual_render::{Color, Size, Spacing};
use std::collections::HashMap;
use std::time::Duration;
use tokio::time::Instant;

/// A single animatable `Styles` field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Field {
    Opacity,
    TextOpacity,
    OffsetX,
    OffsetY,
    Width,
    Height,
    MinWidth,
    MinHeight,
    MaxWidth,
    MaxHeight,
    Margin,
    Padding,
    Color,
    Background,
    BackgroundTint,
    Tint,
    BorderTop,
    BorderRight,
    BorderBottom,
    BorderLeft,
    ScrollbarColor,
    ScrollbarColorHover,
    ScrollbarColorActive,
    ScrollbarBackground,
    ScrollbarBackgroundHover,
    ScrollbarBackgroundActive,
    ScrollbarCornerColor,
    LinkColor,
    LinkBackground,
    LinkColorHover,
    LinkBackgroundHover,
}

const BORDERS: &[Field] = &[
    Field::BorderTop,
    Field::BorderRight,
    Field::BorderBottom,
    Field::BorderLeft,
];

/// The fields a `transition` property name animates.
fn fields(property: &str) -> &'static [Field] {
    use Field::*;
    match property {
        "opacity" => &[Opacity],
        "text-opacity" => &[TextOpacity],
        "offset" => &[OffsetX, OffsetY],
        "width" => &[Width],
        "height" => &[Height],
        "min-width" => &[MinWidth],
        "min-height" => &[MinHeight],
        "max-width" => &[MaxWidth],
        "max-height" => &[MaxHeight],
        "margin" => &[Margin],
        "padding" => &[Padding],
        "color" => &[Color],
        "background" => &[Background],
        "background-tint" => &[BackgroundTint],
        "tint" => &[Tint],
        "border" => BORDERS,
        "border-top" => &[BorderTop],
        "border-right" => &[BorderRight],
        "border-bottom" => &[BorderBottom],
        "border-left" => &[BorderLeft],
        "scrollbar-color" => &[ScrollbarColor],
        "scrollbar-color-hover" => &[ScrollbarColorHover],
        "scrollbar-color-active" => &[ScrollbarColorActive],
        "scrollbar-background" => &[ScrollbarBackground],
        "scrollbar-background-hover" => &[ScrollbarBackgroundHover],
        "scrollbar-background-active" => &[ScrollbarBackgroundActive],
        "scrollbar-corner-color" => &[ScrollbarCornerColor],
        "link-color" => &[LinkColor],
        "link-background" => &[LinkBackground],
        "link-color-hover" => &[LinkColorHover],
        "link-background-hover" => &[LinkBackgroundHover],
        _ => &[],
    }
}

/// Whether CSS property `property` can be transitioned.
pub fn is_animatable(property: &str) -> bool {
    !fields(&property.replace('_', "-")).is_empty()
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Value {
    Num(f32),
    Scalar(Scalar),
    Color(Color),
    Spacing(Spacing),
    Edge(BorderEdge),
}

/// Value of `field` in `s` (with Textual's defaults where unset values have an obvious one);
/// `None` = nothing to animate from/to.
fn get(field: Field, s: &Styles) -> Option<Value> {
    use Field::*;
    let color = |c: Option<rustual_render::Color>| c.map(Value::Color);
    let scalar = |c: Option<Scalar>| c.map(Value::Scalar);
    let edge = |e: Option<BorderEdge>| e.map(Value::Edge);
    match field {
        Opacity => Some(Value::Num(s.get_opacity())),
        TextOpacity => Some(Value::Num(s.get_text_opacity())),
        OffsetX => Some(Value::Scalar(s.offset_x.unwrap_or(Scalar::cells(0.0)))),
        OffsetY => Some(Value::Scalar(s.offset_y.unwrap_or(Scalar::cells(0.0)))),
        Width => Some(Value::Scalar(s.width.unwrap_or(Scalar::AUTO))),
        Height => Some(Value::Scalar(s.height.unwrap_or(Scalar::AUTO))),
        MinWidth => scalar(s.min_width),
        MinHeight => scalar(s.min_height),
        MaxWidth => scalar(s.max_width),
        MaxHeight => scalar(s.max_height),
        Margin => Some(Value::Spacing(s.get_margin())),
        Padding => Some(Value::Spacing(s.get_padding())),
        Color => match s.color {
            Some(ColorValue::Color(c)) => Some(Value::Color(c)),
            _ => None,
        },
        Background => Some(Value::Color(s.get_background())),
        BackgroundTint => Some(Value::Color(
            s.background_tint
                .unwrap_or(rustual_render::Color::TRANSPARENT),
        )),
        Tint => Some(Value::Color(
            s.tint.unwrap_or(rustual_render::Color::TRANSPARENT),
        )),
        BorderTop => edge(s.border_top),
        BorderRight => edge(s.border_right),
        BorderBottom => edge(s.border_bottom),
        BorderLeft => edge(s.border_left),
        ScrollbarColor => color(s.scrollbar_color),
        ScrollbarColorHover => color(s.scrollbar_color_hover),
        ScrollbarColorActive => color(s.scrollbar_color_active),
        ScrollbarBackground => color(s.scrollbar_background),
        ScrollbarBackgroundHover => color(s.scrollbar_background_hover),
        ScrollbarBackgroundActive => color(s.scrollbar_background_active),
        ScrollbarCornerColor => color(s.scrollbar_corner_color),
        LinkColor => color(s.link_color),
        LinkBackground => color(s.link_background),
        LinkColorHover => color(s.link_color_hover),
        LinkBackgroundHover => color(s.link_background_hover),
    }
}

/// Whether `field` is set in `s` (used to skip inline-styled fields).
fn is_set(field: Field, s: &Styles) -> bool {
    use Field::*;
    match field {
        Opacity => s.opacity.is_some(),
        TextOpacity => s.text_opacity.is_some(),
        OffsetX => s.offset_x.is_some(),
        OffsetY => s.offset_y.is_some(),
        Width => s.width.is_some(),
        Height => s.height.is_some(),
        Margin => s.margin.is_some(),
        Padding => s.padding.is_some(),
        Color => s.color.is_some(),
        Background => s.background.is_some(),
        BackgroundTint => s.background_tint.is_some(),
        Tint => s.tint.is_some(),
        _ => get(field, s).is_some(),
    }
}

fn set(field: Field, s: &mut Styles, v: Value) {
    use Field::*;
    match (field, v) {
        (Opacity, Value::Num(n)) => s.opacity = Some(n.clamp(0.0, 1.0)),
        (TextOpacity, Value::Num(n)) => s.text_opacity = Some(n.clamp(0.0, 1.0)),
        (OffsetX, Value::Scalar(v)) => s.offset_x = Some(v),
        (OffsetY, Value::Scalar(v)) => s.offset_y = Some(v),
        (Width, Value::Scalar(v)) => s.width = Some(v),
        (Height, Value::Scalar(v)) => s.height = Some(v),
        (MinWidth, Value::Scalar(v)) => s.min_width = Some(v),
        (MinHeight, Value::Scalar(v)) => s.min_height = Some(v),
        (MaxWidth, Value::Scalar(v)) => s.max_width = Some(v),
        (MaxHeight, Value::Scalar(v)) => s.max_height = Some(v),
        (Margin, Value::Spacing(v)) => s.margin = Some(v),
        (Padding, Value::Spacing(v)) => s.padding = Some(v),
        (Color, Value::Color(c)) => s.color = Some(ColorValue::Color(c)),
        (Background, Value::Color(c)) => s.background = Some(c),
        (BackgroundTint, Value::Color(c)) => s.background_tint = Some(c),
        (Tint, Value::Color(c)) => s.tint = Some(c),
        (BorderTop, Value::Edge(e)) => s.border_top = Some(e),
        (BorderRight, Value::Edge(e)) => s.border_right = Some(e),
        (BorderBottom, Value::Edge(e)) => s.border_bottom = Some(e),
        (BorderLeft, Value::Edge(e)) => s.border_left = Some(e),
        (ScrollbarColor, Value::Color(c)) => s.scrollbar_color = Some(c),
        (ScrollbarColorHover, Value::Color(c)) => s.scrollbar_color_hover = Some(c),
        (ScrollbarColorActive, Value::Color(c)) => s.scrollbar_color_active = Some(c),
        (ScrollbarBackground, Value::Color(c)) => s.scrollbar_background = Some(c),
        (ScrollbarBackgroundHover, Value::Color(c)) => s.scrollbar_background_hover = Some(c),
        (ScrollbarBackgroundActive, Value::Color(c)) => s.scrollbar_background_active = Some(c),
        (ScrollbarCornerColor, Value::Color(c)) => s.scrollbar_corner_color = Some(c),
        (LinkColor, Value::Color(c)) => s.link_color = Some(c),
        (LinkBackground, Value::Color(c)) => s.link_background = Some(c),
        (LinkColorHover, Value::Color(c)) => s.link_color_hover = Some(c),
        (LinkBackgroundHover, Value::Color(c)) => s.link_background_hover = Some(c),
        _ => {}
    }
}

/// Sizes scalars resolve against when a transition changes units (e.g. `auto` → `30`).
#[derive(Debug, Clone, Copy)]
pub(crate) struct Geometry {
    /// The node's current outer size.
    pub own: Size,
    /// The parent's content size.
    pub container: Size,
    pub viewport: Size,
}

/// Resolve a scalar of `field` to cells (`from`: the currently displayed value, where `auto`
/// / `fr` sizes are known from the layout).
fn resolve(field: Field, s: Scalar, geo: Geometry, from: bool) -> Option<f32> {
    let (dim, base) = match field {
        Field::OffsetX => (Dimension::Width, geo.own),
        Field::OffsetY => (Dimension::Height, geo.own),
        Field::Width | Field::MinWidth | Field::MaxWidth => (Dimension::Width, geo.container),
        _ => (Dimension::Height, geo.container),
    };
    match s.unit {
        Unit::Cells => Some(s.value),
        Unit::Auto | Unit::Fraction => match (from, field) {
            (true, Field::Width) => Some(geo.own.width as f32),
            (true, Field::Height) => Some(geo.own.height as f32),
            _ => None,
        },
        _ => Some(s.resolve(dim, base, geo.viewport, 1.0)),
    }
}

/// Make `from` / `to` interpolable (`None`: not animatable, jump).
fn endpoints(field: Field, from: Value, to: Value, geo: Geometry) -> Option<(Value, Value)> {
    match (from, to) {
        (Value::Scalar(a), Value::Scalar(b)) if a.unit == b.unit => {
            (a.unit != Unit::Auto).then_some((from, to))
        }
        (Value::Scalar(a), Value::Scalar(b)) => {
            let a = resolve(field, a, geo, true)?;
            let b = resolve(field, b, geo, false)?;
            Some((
                Value::Scalar(Scalar::cells(a)),
                Value::Scalar(Scalar::cells(b)),
            ))
        }
        (Value::Num(_), Value::Num(_))
        | (Value::Color(_), Value::Color(_))
        | (Value::Spacing(_), Value::Spacing(_))
        | (Value::Edge(_), Value::Edge(_)) => Some((from, to)),
        _ => None,
    }
}

fn lerp(from: Value, to: Value, t: f64) -> Value {
    let f = |a: f32, b: f32| (a as f64 + (b as f64 - a as f64) * t) as f32;
    let i = |a: i32, b: i32| f(a as f32, b as f32).round() as i32;
    match (from, to) {
        (Value::Num(a), Value::Num(b)) => Value::Num(f(a, b)),
        (Value::Scalar(a), Value::Scalar(b)) => {
            let v = f(a.value, b.value);
            let v = if b.unit == Unit::Cells { v.round() } else { v };
            Value::Scalar(Scalar {
                value: v,
                unit: b.unit,
            })
        }
        (Value::Color(a), Value::Color(b)) => Value::Color(a.blend(b, t as f32)),
        (Value::Spacing(a), Value::Spacing(b)) => Value::Spacing(Spacing::new(
            i(a.top, b.top),
            i(a.right, b.right),
            i(a.bottom, b.bottom),
            i(a.left, b.left),
        )),
        (Value::Edge(a), Value::Edge(b)) => Value::Edge(BorderEdge {
            kind: b.kind,
            color: a.color.blend(b.color, t as f32),
        }),
        _ => to,
    }
}

#[derive(Debug, Clone)]
struct Running {
    field: Field,
    /// Interpolation endpoints (scalars converted to a common unit).
    from: Value,
    lerp_to: Value,
    /// The exact target, set on the final frame.
    to: Value,
    /// When interpolation starts (after the delay).
    start: Instant,
    duration: Duration,
    easing: Easing,
}

impl Running {
    /// Value at `now`, and whether the transition is finished.
    fn value(&self, now: Instant) -> (Value, bool) {
        if now < self.start {
            return (self.from, false);
        }
        let factor = if self.duration.is_zero() {
            1.0
        } else {
            (now.duration_since(self.start).as_secs_f64() / self.duration.as_secs_f64()).min(1.0)
        };
        if factor >= 1.0 {
            return (self.to, true);
        }
        (
            lerp(self.from, self.lerp_to, self.easing.apply_f64(factor)),
            false,
        )
    }
}

/// Running CSS transitions, per node.
#[derive(Default)]
pub(crate) struct Transitions {
    running: HashMap<NodeId, Vec<Running>>,
}

impl Transitions {
    /// Number of running (or delayed) transitions.
    pub fn len(&self) -> usize {
        self.running.values().map(Vec::len).sum()
    }

    /// Apply the transitions of `node`: `styles` are its freshly computed styles (cascade +
    /// inline), `previous` its styles from the last frame.
    #[allow(clippy::too_many_arguments)]
    fn apply(
        &mut self,
        node: NodeId,
        styles: &mut Styles,
        previous: &Styles,
        inline: &Styles,
        geo: Geometry,
        now: Instant,
        level: AnimationLevel,
    ) {
        let Some(transitions) = styles.transitions.clone().filter(|t| !t.is_empty()) else {
            self.running.remove(&node);
            return;
        };
        // Never styled before (new node): no animation.
        let first = *previous == Styles::default();
        let mut list = self.running.remove(&node).unwrap_or_default();
        let mut handled: Vec<Field> = Vec::new();
        for t in transitions.iter().rev() {
            for &field in fields(&t.property) {
                if handled.contains(&field) {
                    continue;
                }
                handled.push(field);
                let idx = list.iter().position(|r| r.field == field);
                let target = get(field, styles);
                let Some(to) = target.filter(|_| !is_set(field, inline)) else {
                    if let Some(i) = idx {
                        list.remove(i);
                    }
                    continue;
                };
                let changed = match idx {
                    Some(i) => list[i].to != to,
                    None => !first && get(field, previous).is_some_and(|p| p != to),
                };
                if !changed {
                    continue;
                }
                if let Some(i) = idx {
                    list.remove(i);
                }
                if level < AnimationLevel::Full {
                    continue;
                }
                if let Some(r) =
                    get(field, previous).and_then(|from| start(field, from, to, t, geo, now))
                {
                    list.push(r);
                }
            }
        }
        list.retain(|r| handled.contains(&r.field));
        list.retain(|r| {
            let (v, done) = r.value(now);
            set(r.field, styles, v);
            !done
        });
        if !list.is_empty() {
            self.running.insert(node, list);
        }
    }
}

fn start(
    field: Field,
    from: Value,
    to: Value,
    t: &Transition,
    geo: Geometry,
    now: Instant,
) -> Option<Running> {
    let (from, lerp_to) = endpoints(field, from, to, geo)?;
    Some(Running {
        field,
        from,
        lerp_to,
        to,
        start: now + t.delay,
        duration: t.duration,
        easing: Easing::from_name(&t.easing).unwrap_or(Easing::Linear),
    })
}

/// Hook for `update_styles`: apply node `id`'s CSS transitions to its freshly computed
/// `styles` (cascade + inline, before inheritance).
pub(crate) fn apply(
    animator: &mut Animator,
    dom: &Dom,
    id: NodeId,
    styles: &mut Styles,
    viewport: Size,
    now: Instant,
) {
    if styles.transitions.is_none() && !animator.transitions.running.contains_key(&id) {
        return;
    }
    let Some(node) = dom.get(id) else { return };
    let container = node
        .parent
        .and_then(|p| dom.get(p))
        .map(|p| p.layout.content_region.size())
        .unwrap_or(viewport);
    let geo = Geometry {
        own: node.layout.region.size(),
        container,
        viewport,
    };
    let level = animator.level;
    animator.transitions.apply(
        id,
        styles,
        &node.styles,
        &node.inline_styles,
        geo,
        now,
        level,
    );
}

/// Hook for the end of `update_styles`: forget removed nodes and keep frames coming while
/// transitions run.
pub(crate) fn end_frame(animator: &mut Animator, dom: &Dom, now: Instant) {
    if animator.transitions.running.is_empty() {
        return;
    }
    animator
        .transitions
        .running
        .retain(|id, _| dom.contains(*id));
    if !animator.transitions.running.is_empty() {
        animator.schedule(now);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn property_names() {
        assert!(is_animatable("offset"));
        assert!(is_animatable("text_opacity"));
        assert!(is_animatable("background"));
        assert!(!is_animatable("display"));
    }

    #[test]
    fn interpolation() {
        let a = Value::Color(Color::rgb(0, 0, 0));
        let b = Value::Color(Color::rgb(200, 100, 50));
        assert_eq!(lerp(a, b, 0.5), Value::Color(Color::rgb(100, 50, 25)));
        let geo = Geometry {
            own: Size::new(10, 4),
            container: Size::new(40, 20),
            viewport: Size::new(80, 24),
        };
        // auto → 30 cells: starts from the laid-out width.
        let (f, t) = endpoints(
            Field::Width,
            Value::Scalar(Scalar::AUTO),
            Value::Scalar(Scalar::cells(30.0)),
            geo,
        )
        .unwrap();
        assert_eq!(lerp(f, t, 0.5), Value::Scalar(Scalar::cells(20.0)));
        // 50% of the container → 0.
        let (f, t) = endpoints(
            Field::Width,
            Value::Scalar(Scalar::percent(50.0)),
            Value::Scalar(Scalar::cells(0.0)),
            geo,
        )
        .unwrap();
        assert_eq!(f, Value::Scalar(Scalar::cells(20.0)));
        assert_eq!(lerp(f, t, 0.25), Value::Scalar(Scalar::cells(15.0)));
        // to `auto` cannot be animated.
        assert!(endpoints(
            Field::Width,
            Value::Scalar(Scalar::cells(3.0)),
            Value::Scalar(Scalar::AUTO),
            geo
        )
        .is_none());
    }
}

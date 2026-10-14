//! Animation: easing functions (port of `textual/_easing.py`) and the app's animator
//! (port of `textual/_animator.py`).
//!
//! The animator lives in the app state and is driven by the event loop: while any animation is
//! active, [`AppState::next_deadline`] includes the next frame time (~60 fps) and every frame
//! sets each animation's value for the current time. Under tokio's paused clock,
//! [`crate::Pilot::wait`] advances animations deterministically (and finishes with a frame at
//! exactly the target time).
//!
//! ```ignore
//! // Generic, callback-driven animation of any widget value (Textual: `self.animate("x", ..)`):
//! ctx.animate_value::<MyWidget>("glow", 0.0, 1.0, Duration::from_millis(300), Easing::InOutCubic,
//!     |w, value, _ctx| w.glow = value);
//! // Inline-style animation (opacity, offset, width/height in cells):
//! ctx.animate_style(id, StyleProp::Opacity, 0.0, Duration::from_millis(500), Easing::Linear);
//! // Smooth scrolling:
//! ctx.scroll_to_animated(0, 40, Some(Duration::from_millis(200)));
//! ```
//!
//! Semantics follow Textual:
//! - An animation is keyed by (node, key). Starting another animation with the same key replaces
//!   it (the replaced animation's `on_complete` still runs).
//! - The value is `from + (to - from) * easing(t)`; the final frame sets exactly `to`, then
//!   `on_complete` runs.
//! - [`AnimationLevel`]: with `none`, every animation jumps to its final value on the next frame;
//!   with `basic`, only animations whose level is `full` do.
//!
//! Deviations: the first frame is applied one frame after the animation starts (as in Textual),
//! so widgets should render their start state themselves; there is no `delay` / `speed` API on
//! `animate_value` (scrolling uses Textual's default speed of 50 cells/s).

use crate::app::AppState;
use crate::context::Ctx;
use crate::dom::NodeId;
use crate::widget::Widget;
use std::any::Any;
use std::collections::HashMap;
use std::f64::consts::PI;
use std::time::Duration;
use textual_css::{Scalar, Unit};
use textual_render::Offset;
use tokio::time::Instant;

/// Time between animation frames (60 fps).
pub const FRAME: Duration = Duration::from_nanos(16_666_666);

/// Textual's default scroll speed in cells per second (`widget.py::_scroll_to`).
pub const SCROLL_SPEED: f32 = 50.0;

// ---------------------------------------------------------------------------------- easing

/// An easing function mapping `[0, 1]` → (roughly) `[0, 1]`. Names match Textual's `EASING`
/// table (`"in_out_cubic"`, `"out_bounce"`, ...); see [`Easing::from_name`].
#[derive(Debug, Clone, Copy, Default)]
pub enum Easing {
    /// Jump to the end immediately (`"none"`).
    None,
    /// Jump half way through (`"round"`).
    Round,
    Linear,
    InSine,
    InOutSine,
    OutSine,
    InQuad,
    InOutQuad,
    OutQuad,
    InCubic,
    /// Textual's `DEFAULT_EASING`.
    #[default]
    InOutCubic,
    /// Textual's `DEFAULT_SCROLL_EASING`.
    OutCubic,
    InQuart,
    InOutQuart,
    OutQuart,
    InQuint,
    InOutQuint,
    OutQuint,
    InExpo,
    InOutExpo,
    OutExpo,
    InCirc,
    InOutCirc,
    OutCirc,
    InBack,
    InOutBack,
    OutBack,
    InElastic,
    InOutElastic,
    OutElastic,
    InBounce,
    InOutBounce,
    OutBounce,
    /// A custom easing function.
    Custom(fn(f32) -> f32),
}

impl Easing {
    /// Textual's default easing (`in_out_cubic`).
    pub const DEFAULT: Easing = Easing::InOutCubic;
    /// Textual's default scroll easing (`out_cubic`).
    pub const DEFAULT_SCROLL: Easing = Easing::OutCubic;

    /// Every named easing, in Textual's `EASING` order.
    pub const ALL: [Easing; 33] = [
        Easing::None,
        Easing::Round,
        Easing::Linear,
        Easing::InSine,
        Easing::InOutSine,
        Easing::OutSine,
        Easing::InQuad,
        Easing::InOutQuad,
        Easing::OutQuad,
        Easing::InCubic,
        Easing::InOutCubic,
        Easing::OutCubic,
        Easing::InQuart,
        Easing::InOutQuart,
        Easing::OutQuart,
        Easing::InQuint,
        Easing::InOutQuint,
        Easing::OutQuint,
        Easing::InExpo,
        Easing::InOutExpo,
        Easing::OutExpo,
        Easing::InCirc,
        Easing::InOutCirc,
        Easing::OutCirc,
        Easing::InBack,
        Easing::InOutBack,
        Easing::OutBack,
        Easing::InElastic,
        Easing::InOutElastic,
        Easing::OutElastic,
        Easing::InBounce,
        Easing::InOutBounce,
        Easing::OutBounce,
    ];

    /// Textual's name for this easing (`"custom"` for [`Easing::Custom`]).
    pub fn name(&self) -> &'static str {
        match self {
            Easing::None => "none",
            Easing::Round => "round",
            Easing::Linear => "linear",
            Easing::InSine => "in_sine",
            Easing::InOutSine => "in_out_sine",
            Easing::OutSine => "out_sine",
            Easing::InQuad => "in_quad",
            Easing::InOutQuad => "in_out_quad",
            Easing::OutQuad => "out_quad",
            Easing::InCubic => "in_cubic",
            Easing::InOutCubic => "in_out_cubic",
            Easing::OutCubic => "out_cubic",
            Easing::InQuart => "in_quart",
            Easing::InOutQuart => "in_out_quart",
            Easing::OutQuart => "out_quart",
            Easing::InQuint => "in_quint",
            Easing::InOutQuint => "in_out_quint",
            Easing::OutQuint => "out_quint",
            Easing::InExpo => "in_expo",
            Easing::InOutExpo => "in_out_expo",
            Easing::OutExpo => "out_expo",
            Easing::InCirc => "in_circ",
            Easing::InOutCirc => "in_out_circ",
            Easing::OutCirc => "out_circ",
            Easing::InBack => "in_back",
            Easing::InOutBack => "in_out_back",
            Easing::OutBack => "out_back",
            Easing::InElastic => "in_elastic",
            Easing::InOutElastic => "in_out_elastic",
            Easing::OutElastic => "out_elastic",
            Easing::InBounce => "in_bounce",
            Easing::InOutBounce => "in_out_bounce",
            Easing::OutBounce => "out_bounce",
            Easing::Custom(_) => "custom",
        }
    }

    /// Look up an easing by Textual name (`"in_out_cubic"`; `-` is accepted for `_`).
    pub fn from_name(name: &str) -> Option<Easing> {
        let name = name.trim().replace('-', "_");
        Easing::ALL.into_iter().find(|e| e.name() == name)
    }

    /// Apply the easing to `x` in `[0, 1]`.
    pub fn apply(&self, x: f32) -> f32 {
        if let Easing::Custom(f) = self {
            return f(x);
        }
        self.apply_f64(x as f64) as f32
    }

    /// Apply the easing in double precision (Textual computes in Python floats).
    pub fn apply_f64(&self, x: f64) -> f64 {
        match self {
            Easing::None => 1.0,
            Easing::Round => {
                if x < 0.5 {
                    0.0
                } else {
                    1.0
                }
            }
            Easing::Linear => x,
            Easing::InSine => 1.0 - ((x * PI) / 2.0).cos(),
            Easing::InOutSine => -((x * PI).cos() - 1.0) / 2.0,
            Easing::OutSine => ((x * PI) / 2.0).sin(),
            Easing::InQuad => x * x,
            Easing::InOutQuad => {
                if x < 0.5 {
                    2.0 * x * x
                } else {
                    1.0 - (-2.0 * x + 2.0).powi(2) / 2.0
                }
            }
            Easing::OutQuad => 1.0 - (1.0 - x).powi(2),
            Easing::InCubic => x * x * x,
            Easing::InOutCubic => {
                if x < 0.5 {
                    4.0 * x * x * x
                } else {
                    1.0 - (-2.0 * x + 2.0).powi(3) / 2.0
                }
            }
            Easing::OutCubic => 1.0 - (1.0 - x).powi(3),
            Easing::InQuart => x.powi(4),
            Easing::InOutQuart => {
                if x < 0.5 {
                    8.0 * x.powi(4)
                } else {
                    1.0 - (-2.0 * x + 2.0).powi(4) / 2.0
                }
            }
            Easing::OutQuart => 1.0 - (1.0 - x).powi(4),
            Easing::InQuint => x.powi(5),
            Easing::InOutQuint => {
                if x < 0.5 {
                    16.0 * x.powi(5)
                } else {
                    1.0 - (-2.0 * x + 2.0).powi(5) / 2.0
                }
            }
            Easing::OutQuint => 1.0 - (1.0 - x).powi(5),
            Easing::InExpo => {
                if x != 0.0 {
                    2f64.powf(10.0 * x - 10.0)
                } else {
                    0.0
                }
            }
            Easing::InOutExpo => {
                if 0.0 < x && x < 0.5 {
                    2f64.powf(20.0 * x - 10.0) / 2.0
                } else if (0.5..1.0).contains(&x) {
                    (2.0 - 2f64.powf(-20.0 * x + 10.0)) / 2.0
                } else {
                    x
                }
            }
            Easing::OutExpo => {
                if x != 1.0 {
                    1.0 - 2f64.powf(-10.0 * x)
                } else {
                    1.0
                }
            }
            Easing::InCirc => 1.0 - (1.0 - x.powi(2)).sqrt(),
            Easing::InOutCirc => {
                if x < 0.5 {
                    (1.0 - (1.0 - (2.0 * x).powi(2)).sqrt()) / 2.0
                } else {
                    ((1.0 - (-2.0 * x + 2.0).powi(2)).sqrt() + 1.0) / 2.0
                }
            }
            Easing::OutCirc => (1.0 - (x - 1.0).powi(2)).sqrt(),
            Easing::InBack => 2.70158 * x.powi(3) - 1.70158 * x.powi(2),
            Easing::InOutBack => {
                let c = 1.70158 * 1.525;
                if x < 0.5 {
                    ((2.0 * x).powi(2) * ((c + 1.0) * 2.0 * x - c)) / 2.0
                } else {
                    ((2.0 * x - 2.0).powi(2) * ((c + 1.0) * (x * 2.0 - 2.0) + c) + 2.0) / 2.0
                }
            }
            Easing::OutBack => 1.0 + 2.70158 * (x - 1.0).powi(3) + 1.70158 * (x - 1.0).powi(2),
            Easing::InElastic => {
                let c = 2.0 * PI / 3.0;
                if 0.0 < x && x < 1.0 {
                    -(2f64.powf(10.0 * x - 10.0)) * ((x * 10.0 - 10.75) * c).sin()
                } else {
                    x
                }
            }
            Easing::InOutElastic => {
                let c = 2.0 * PI / 4.5;
                if 0.0 < x && x < 0.5 {
                    -(2f64.powf(20.0 * x - 10.0) * ((20.0 * x - 11.125) * c).sin()) / 2.0
                } else if (0.5..1.0).contains(&x) {
                    (2f64.powf(-20.0 * x + 10.0) * ((20.0 * x - 11.125) * c).sin()) / 2.0 + 1.0
                } else {
                    x
                }
            }
            Easing::OutElastic => {
                let c = 2.0 * PI / 3.0;
                if 0.0 < x && x < 1.0 {
                    2f64.powf(-10.0 * x) * ((x * 10.0 - 0.75) * c).sin() + 1.0
                } else {
                    x
                }
            }
            Easing::InBounce => 1.0 - out_bounce(1.0 - x),
            Easing::InOutBounce => {
                if x < 0.5 {
                    (1.0 - out_bounce(1.0 - 2.0 * x)) / 2.0
                } else {
                    (1.0 + out_bounce(2.0 * x - 1.0)) / 2.0
                }
            }
            Easing::OutBounce => out_bounce(x),
            Easing::Custom(f) => f(x as f32) as f64,
        }
    }
}

fn out_bounce(x: f64) -> f64 {
    let (n, d) = (7.5625, 2.75);
    if x < 1.0 / d {
        n * x * x
    } else if x < 2.0 / d {
        let x = x - 1.5 / d;
        n * x * x + 0.75
    } else if x < 2.5 / d {
        let x = x - 2.25 / d;
        n * x * x + 0.9375
    } else {
        let x = x - 2.625 / d;
        n * x * x + 0.984375
    }
}

impl std::str::FromStr for Easing {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Easing::from_name(s).ok_or_else(|| format!("unknown easing {s:?}"))
    }
}

// ---------------------------------------------------------------------------------- levels

/// How much animation the app performs (Textual's `App.animation_level` /
/// `TEXTUAL_ANIMATIONS`). Ordered: `None < Basic < Full`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum AnimationLevel {
    /// No animations: everything jumps to its final value.
    None,
    /// Only "basic" animations (scrolling, tab underline, switch, ...).
    Basic,
    /// All animations.
    #[default]
    Full,
}

/// Identifies a running animation (returned by the `animate_*` methods).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AnimationId(pub(crate) u64);

/// Inline style properties [`Ctx::animate_style`] can animate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StyleProp {
    /// `opacity` (0..=1).
    Opacity,
    /// `text-opacity` (0..=1).
    TextOpacity,
    /// Horizontal `offset`, in cells.
    OffsetX,
    /// Vertical `offset`, in cells.
    OffsetY,
    /// `width`, in cells.
    Width,
    /// `height`, in cells.
    Height,
}

impl StyleProp {
    /// Animation key used for this property (`"style:opacity"`, ...).
    pub fn key(&self) -> &'static str {
        match self {
            StyleProp::Opacity => "style:opacity",
            StyleProp::TextOpacity => "style:text-opacity",
            StyleProp::OffsetX => "style:offset-x",
            StyleProp::OffsetY => "style:offset-y",
            StyleProp::Width => "style:width",
            StyleProp::Height => "style:height",
        }
    }
}

const SCROLL_X: &str = "scroll_x";
const SCROLL_Y: &str = "scroll_y";

// ---------------------------------------------------------------------------------- animator

/// Completion callback for [`Ctx::animate_value_with`].
pub type OnComplete<W> = Box<dyn FnOnce(&mut W, &mut Ctx)>;

pub(crate) type ValueCallback = Box<dyn FnMut(&mut dyn Widget, f32, &mut Ctx)>;
pub(crate) type CompleteCallback = Box<dyn FnOnce(&mut dyn Widget, &mut Ctx)>;

pub(crate) enum Target {
    Widget(ValueCallback),
    Style(StyleProp),
    Scroll { vertical: bool },
}

pub(crate) struct Animation {
    id: u64,
    node: NodeId,
    key: String,
    start: Instant,
    duration: Duration,
    from: f32,
    to: f32,
    easing: Easing,
    level: AnimationLevel,
    target: Target,
    on_complete: Option<CompleteCallback>,
}

/// Parameters for starting an animation.
pub(crate) struct Spec {
    pub node: NodeId,
    pub key: String,
    pub from: f32,
    pub to: f32,
    pub duration: Duration,
    pub easing: Easing,
    pub level: AnimationLevel,
    pub target: Target,
    pub on_complete: Option<CompleteCallback>,
}

/// The app's animator (Textual's `Animator`).
#[derive(Default)]
pub(crate) struct Animator {
    animations: Vec<Animation>,
    /// (node, key) → id of the live animation for that key.
    live: HashMap<(NodeId, String), u64>,
    /// `on_complete` callbacks of replaced animations, run on the next frame.
    completions: Vec<(NodeId, CompleteCallback)>,
    /// Stopped-with-complete animations: final value + `on_complete` on the next frame.
    finishing: Vec<Animation>,
    next_id: u64,
    pub level: AnimationLevel,
    /// When the next frame is due (None = idle).
    pub next_frame: Option<Instant>,
}

impl Animator {
    fn is_live(&self, node: NodeId, key: &str, id: u64) -> bool {
        self.live.get(&(node, key.to_string())) == Some(&id)
    }

    fn start(&mut self, spec: Spec) -> AnimationId {
        self.next_id += 1;
        let id = self.next_id;
        let k = (spec.node, spec.key.clone());
        if let Some(old) = self.live.insert(k, id) {
            self.drop_animation(old);
        }
        let now = Instant::now();
        self.animations.push(Animation {
            id,
            node: spec.node,
            key: spec.key,
            start: now,
            duration: spec.duration,
            from: spec.from,
            to: spec.to,
            easing: spec.easing,
            level: spec.level,
            target: spec.target,
            on_complete: spec.on_complete,
        });
        self.schedule(now);
        AnimationId(id)
    }

    /// Remove an animation that is not checked out, queueing its `on_complete`.
    fn drop_animation(&mut self, id: u64) {
        if let Some(i) = self.animations.iter().position(|a| a.id == id) {
            let mut a = self.animations.remove(i);
            if let Some(cb) = a.on_complete.take() {
                self.completions.push((a.node, cb));
            }
        }
    }

    fn schedule(&mut self, now: Instant) {
        if self.next_frame.is_none() {
            self.next_frame = Some(now + FRAME);
        }
    }

    fn find(&self, node: NodeId, key: &str) -> Option<&Animation> {
        let id = *self.live.get(&(node, key.to_string()))?;
        self.animations.iter().find(|a| a.id == id)
    }

    /// Whether an animation with this key is running on `node`.
    pub fn is_animating(&self, node: NodeId, key: &str) -> bool {
        self.live.contains_key(&(node, key.to_string()))
    }

    /// Number of running animations.
    pub fn len(&self) -> usize {
        self.live.len()
    }

    /// Stop the scroll animations of `node` without completing them (a direct scroll wins).
    pub fn cancel_scroll(&mut self, node: NodeId) {
        for key in [SCROLL_X, SCROLL_Y] {
            if let Some(id) = self.live.remove(&(node, key.to_string())) {
                self.drop_animation(id);
            }
        }
    }

    /// Where `node` is scrolling to (the target of its scroll animations), if animating.
    pub fn scroll_target(&self, node: NodeId) -> (Option<i32>, Option<i32>) {
        let x = self.find(node, SCROLL_X).map(|a| a.to.round() as i32);
        let y = self.find(node, SCROLL_Y).map(|a| a.to.round() as i32);
        (x, y)
    }
}

impl AppState {
    pub(crate) fn start_animation(&mut self, spec: Spec) -> AnimationId {
        self.dirty = true;
        self.animator.start(spec)
    }

    /// Stop an animation; with `complete` the final value is applied and `on_complete` runs on
    /// the next frame (the widget may be checked out right now).
    pub(crate) fn stop_animation(&mut self, node: NodeId, key: &str, complete: bool) {
        let Some(id) = self.animator.live.remove(&(node, key.to_string())) else {
            return;
        };
        let Some(i) = self.animator.animations.iter().position(|a| a.id == id) else {
            return;
        };
        let a = self.animator.animations.remove(i);
        if complete {
            self.animator.finishing.push(a);
            self.animator.schedule(Instant::now());
        }
    }

    /// Run one animation frame if due (or unconditionally with `force`). Returns whether a frame
    /// ran.
    pub(crate) fn tick_animations(&mut self, now: Instant, force: bool) -> bool {
        match self.animator.next_frame {
            Some(next) if force || next <= now => {}
            _ => return false,
        }
        self.animator.next_frame = None;
        for mut a in std::mem::take(&mut self.animator.finishing) {
            let to = a.to;
            self.apply_value(&mut a, to);
            if let Some(cb) = a.on_complete.take() {
                self.call_widget(a.node, |w, ctx| cb(w, ctx));
            }
        }
        for (node, cb) in std::mem::take(&mut self.animator.completions) {
            self.call_widget(node, |w, ctx| cb(w, ctx));
        }
        let level = self.animator.level;
        let current = std::mem::take(&mut self.animator.animations);
        let mut keep = Vec::new();
        for mut a in current {
            if !self.animator.is_live(a.node, &a.key, a.id) {
                if let Some(cb) = a.on_complete.take() {
                    self.call_widget(a.node, |w, ctx| cb(w, ctx));
                }
                continue;
            }
            if !self.dom.contains(a.node) {
                self.animator.live.remove(&(a.node, a.key.clone()));
                continue;
            }
            let factor = if a.duration.is_zero() || level < a.level {
                1.0
            } else {
                (now.saturating_duration_since(a.start).as_secs_f64() / a.duration.as_secs_f64())
                    .min(1.0)
            };
            let value = if factor >= 1.0 {
                a.to
            } else {
                let eased = a.easing.apply_f64(factor);
                (a.from as f64 + (a.to as f64 - a.from as f64) * eased) as f32
            };
            self.apply_value(&mut a, value);
            let live = self.animator.is_live(a.node, &a.key, a.id);
            if factor >= 1.0 {
                if live {
                    self.animator.live.remove(&(a.node, a.key.clone()));
                }
                if let Some(cb) = a.on_complete.take() {
                    self.call_widget(a.node, |w, ctx| cb(w, ctx));
                }
            } else if live {
                keep.push(a);
            } else if let Some(cb) = a.on_complete.take() {
                // Replaced from within its own callback.
                self.call_widget(a.node, |w, ctx| cb(w, ctx));
            }
        }
        // Animations started during this frame stay after the ones that were running.
        keep.append(&mut self.animator.animations);
        self.animator.animations = keep;
        if !self.animator.animations.is_empty()
            || !self.animator.completions.is_empty()
            || !self.animator.finishing.is_empty()
        {
            self.animator.next_frame = Some(now + FRAME);
        }
        self.dirty = true;
        true
    }

    fn apply_value(&mut self, a: &mut Animation, value: f32) {
        let node = a.node;
        match &mut a.target {
            Target::Widget(cb) => {
                self.call_widget(node, |w, ctx| cb(w, value, ctx));
            }
            Target::Style(prop) => {
                let Some(n) = self.dom.get_mut(node) else {
                    return;
                };
                let s = &mut n.inline_styles;
                match prop {
                    StyleProp::Opacity => s.opacity = Some(value.clamp(0.0, 1.0)),
                    StyleProp::TextOpacity => s.text_opacity = Some(value.clamp(0.0, 1.0)),
                    StyleProp::OffsetX => s.offset_x = Some(Scalar::cells(value.round())),
                    StyleProp::OffsetY => s.offset_y = Some(Scalar::cells(value.round())),
                    StyleProp::Width => s.width = Some(Scalar::cells(value.round().max(0.0))),
                    StyleProp::Height => s.height = Some(Scalar::cells(value.round().max(0.0))),
                }
                self.dirty = true;
            }
            Target::Scroll { vertical } => {
                let Some(n) = self.dom.get_mut(node) else {
                    return;
                };
                let v = value.round() as i32;
                if *vertical {
                    n.scroll_offset.y = v.max(0);
                } else {
                    n.scroll_offset.x = v.max(0);
                }
                self.dirty = true;
            }
        }
    }

    /// Current value of an animatable style property of `node` (inline style, else computed
    /// style, else the layout).
    fn style_value(&self, node: NodeId, prop: StyleProp) -> f32 {
        let Some(n) = self.dom.get(node) else {
            return 0.0;
        };
        let cells = |s: Option<Scalar>| s.filter(|s| s.unit == Unit::Cells).map(|s| s.value);
        let (inl, st) = (&n.inline_styles, &n.styles);
        match prop {
            StyleProp::Opacity => inl.opacity.unwrap_or(st.get_opacity()),
            StyleProp::TextOpacity => inl.text_opacity.unwrap_or(st.get_text_opacity()),
            StyleProp::OffsetX => cells(inl.offset_x).or(cells(st.offset_x)).unwrap_or(0.0),
            StyleProp::OffsetY => cells(inl.offset_y).or(cells(st.offset_y)).unwrap_or(0.0),
            StyleProp::Width => n.layout.region.width as f32,
            StyleProp::Height => n.layout.region.height as f32,
        }
    }

    /// The offset `node` is scrolling to: the target of a running scroll animation, else the
    /// current offset (Textual's `scroll_target_x/y`).
    pub(crate) fn scroll_target(&self, node: NodeId) -> Offset {
        let cur = self
            .dom
            .get(node)
            .map(|n| n.scroll_offset)
            .unwrap_or_default();
        let (x, y) = self.animator.scroll_target(node);
        Offset::new(x.unwrap_or(cur.x), y.unwrap_or(cur.y))
    }

    /// Scroll `node` to `target` (clamped to its max scroll), animating with Textual's default
    /// scroll easing. `duration: None` uses Textual's default speed (50 cells/s).
    pub(crate) fn scroll_animated(
        &mut self,
        node: NodeId,
        target: Offset,
        duration: Option<Duration>,
        easing: Easing,
    ) {
        let Some(n) = self.dom.get(node) else { return };
        let max = n.layout.max_scroll();
        let target = target.clamp(max.x, max.y);
        let cur = n.scroll_offset;
        self.animator.cancel_scroll(node);
        for (vertical, from, to) in [(false, cur.x, target.x), (true, cur.y, target.y)] {
            if from == to {
                continue;
            }
            let duration = duration.unwrap_or_else(|| {
                Duration::from_secs_f32((to - from).abs() as f32 / SCROLL_SPEED)
            });
            self.start_animation(Spec {
                node,
                key: if vertical { SCROLL_Y } else { SCROLL_X }.to_string(),
                from: from as f32,
                to: to as f32,
                duration,
                easing,
                level: AnimationLevel::Basic,
                target: Target::Scroll { vertical },
                on_complete: None,
            });
        }
    }
}

fn downcast_value<W: Widget>(mut f: impl FnMut(&mut W, f32, &mut Ctx) + 'static) -> ValueCallback {
    Box::new(move |w, v, ctx| {
        if let Some(w) = (w as &mut dyn Any).downcast_mut::<W>() {
            f(w, v, ctx)
        }
    })
}

fn downcast_complete<W: Widget>(f: impl FnOnce(&mut W, &mut Ctx) + 'static) -> CompleteCallback {
    Box::new(move |w, ctx| {
        if let Some(w) = (w as &mut dyn Any).downcast_mut::<W>() {
            f(w, ctx)
        }
    })
}

// ---------------------------------------------------------------------------------- Ctx API

impl<'a> Ctx<'a> {
    /// Animate a value of this widget from `from` to `to` over `duration`: `f` is called with
    /// the widget and the eased value on every frame (the last call gets exactly `to`).
    /// Starting another animation with the same `key` on this widget replaces this one.
    /// (Textual: `self.animate("attr", value, duration=.., easing=..)`.)
    pub fn animate_value<W: Widget>(
        &mut self,
        key: &str,
        from: f32,
        to: f32,
        duration: Duration,
        easing: Easing,
        f: impl FnMut(&mut W, f32, &mut Ctx) + 'static,
    ) -> AnimationId {
        let node = self.node;
        self.app.start_animation(Spec {
            node,
            key: key.to_string(),
            from,
            to,
            duration,
            easing,
            level: AnimationLevel::Full,
            target: Target::Widget(downcast_value(f)),
            on_complete: None,
        })
    }

    /// Like [`Ctx::animate_value`], with an animation level (animations above the app's level
    /// complete immediately) and a callback run when the animation completes (or is replaced).
    #[allow(clippy::too_many_arguments)]
    pub fn animate_value_with<W: Widget>(
        &mut self,
        key: &str,
        from: f32,
        to: f32,
        duration: Duration,
        easing: Easing,
        level: AnimationLevel,
        f: impl FnMut(&mut W, f32, &mut Ctx) + 'static,
        on_complete: Option<OnComplete<W>>,
    ) -> AnimationId {
        let node = self.node;
        self.app.start_animation(Spec {
            node,
            key: key.to_string(),
            from,
            to,
            duration,
            easing,
            level,
            target: Target::Widget(downcast_value(f)),
            on_complete: on_complete.map(|c| downcast_complete::<W>(c)),
        })
    }

    /// Animate an inline style property of node `id` from its current value to `to`.
    /// (Textual: `widget.styles.animate("opacity", 0.0, duration=..)`.)
    pub fn animate_style(
        &mut self,
        id: NodeId,
        prop: StyleProp,
        to: f32,
        duration: Duration,
        easing: Easing,
    ) -> AnimationId {
        let from = self.app.style_value(id, prop);
        self.animate_style_from(id, prop, from, to, duration, easing)
    }

    /// Animate an inline style property of node `id` from `from` to `to`.
    pub fn animate_style_from(
        &mut self,
        id: NodeId,
        prop: StyleProp,
        from: f32,
        to: f32,
        duration: Duration,
        easing: Easing,
    ) -> AnimationId {
        self.app.start_animation(Spec {
            node: id,
            key: prop.key().to_string(),
            from,
            to,
            duration,
            easing,
            level: AnimationLevel::Full,
            target: Target::Style(prop),
            on_complete: None,
        })
    }

    /// Stop this widget's animation `key` (`complete`: jump to the final value).
    pub fn stop_animation(&mut self, key: &str, complete: bool) {
        let node = self.node;
        self.app.stop_animation(node, key, complete);
    }

    /// Stop the animation `key` of any node (style animations use [`StyleProp::key`]).
    pub fn stop_animation_of(&mut self, id: NodeId, key: &str, complete: bool) {
        self.app.stop_animation(id, key, complete);
    }

    /// Whether this widget has a running animation `key`.
    pub fn is_animating(&self, key: &str) -> bool {
        self.app.animator.is_animating(self.node, key)
    }

    /// Whether node `id` has a running animation `key`.
    pub fn is_animating_node(&self, id: NodeId, key: &str) -> bool {
        self.app.animator.is_animating(id, key)
    }

    /// The app's animation level.
    pub fn animation_level(&self) -> AnimationLevel {
        self.app.animator.level
    }

    /// Change the app's animation level.
    pub fn set_animation_level(&mut self, level: AnimationLevel) {
        self.app.animator.level = level;
    }

    /// Smoothly scroll this widget to (`x`, `y`) (clamped to the max scroll) with Textual's
    /// scroll easing (`out_cubic`). `duration: None` uses Textual's speed of 50 cells/s.
    pub fn scroll_to_animated(&mut self, x: i32, y: i32, duration: Option<Duration>) {
        let id = self.node;
        self.scroll_node_to_animated(id, x, y, duration);
    }

    /// Smoothly scroll node `id` (see [`Ctx::scroll_to_animated`]).
    pub fn scroll_node_to_animated(
        &mut self,
        id: NodeId,
        x: i32,
        y: i32,
        duration: Option<Duration>,
    ) {
        self.app
            .scroll_animated(id, Offset::new(x, y), duration, Easing::DEFAULT_SCROLL);
    }

    /// Where this widget is scrolling to (the target of a running scroll animation, else the
    /// current offset). Relative scrolls should start from here, like Textual's `scroll_target_y`.
    pub fn scroll_target(&self) -> Offset {
        self.app.scroll_target(self.node)
    }
}

impl crate::app::App {
    /// Set the animation level (Textual: `App.animation_level` / `TEXTUAL_ANIMATIONS`).
    pub fn animation_level(mut self, level: AnimationLevel) -> Self {
        self.state.animator.level = level;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-5
    }

    #[test]
    fn endpoints() {
        for e in Easing::ALL {
            let name = e.name();
            if name == "none" {
                assert_eq!(e.apply(0.0), 1.0);
                continue;
            }
            assert!(close(e.apply(0.0), 0.0), "{name} at 0 = {}", e.apply(0.0));
            assert!(close(e.apply(1.0), 1.0), "{name} at 1 = {}", e.apply(1.0));
        }
    }

    #[test]
    fn known_values() {
        // Values computed with Python's textual._easing.EASING.
        let cases: &[(&str, f32, f32)] = &[
            ("linear", 0.25, 0.25),
            ("round", 0.49, 0.0),
            ("round", 0.5, 1.0),
            ("in_quad", 0.5, 0.25),
            ("out_quad", 0.5, 0.75),
            ("in_cubic", 0.5, 0.125),
            ("in_out_cubic", 0.25, 0.0625),
            ("in_out_cubic", 0.5, 0.5),
            ("in_out_cubic", 0.75, 0.9375),
            ("out_cubic", 0.5, 0.875),
            ("in_out_quart", 0.25, 0.03125),
            ("in_out_quint", 0.25, 0.015625),
            ("in_sine", 0.5, 0.29289323),
            ("out_sine", 0.5, 0.70710677),
            ("in_out_sine", 0.25, 0.14644662),
            ("in_expo", 0.5, 0.03125),
            ("out_expo", 0.5, 0.96875),
            ("in_out_expo", 0.25, 0.015625),
            ("in_circ", 0.5, 0.1339746),
            ("out_circ", 0.5, 0.8660254),
            ("in_out_circ", 0.25, 0.0669873),
            ("in_back", 0.5, -0.0876975),
            ("out_back", 0.5, 1.0876975),
            ("in_out_back", 0.25, -0.0996818),
            ("out_bounce", 0.5, 0.765625),
            ("in_bounce", 0.5, 0.234375),
            ("in_out_bounce", 0.25, 0.1171875),
            ("out_elastic", 0.5, 1.015625),
            ("in_elastic", 0.5, -0.015625),
            ("in_out_elastic", 0.25, 0.0119694),
        ];
        for (name, x, want) in cases {
            let e = Easing::from_name(name).unwrap();
            let got = e.apply(*x);
            assert!(
                (got - want).abs() < 1e-4,
                "{name}({x}) = {got}, want {want}"
            );
        }
    }

    #[test]
    fn names_round_trip() {
        assert_eq!(Easing::ALL.len(), 33);
        for e in Easing::ALL {
            assert_eq!(Easing::from_name(e.name()).unwrap().name(), e.name());
        }
        assert_eq!(
            Easing::from_name("in-out-cubic").unwrap().name(),
            "in_out_cubic"
        );
        assert!(Easing::from_name("bogus").is_none());
        assert_eq!(Easing::default().name(), "in_out_cubic");
        assert_eq!(Easing::DEFAULT_SCROLL.name(), "out_cubic");
        assert_eq!("linear".parse::<Easing>().unwrap().name(), "linear");
        fn half(x: f32) -> f32 {
            x / 2.0
        }
        assert_eq!(Easing::Custom(half).apply(0.5), 0.25);
    }
}

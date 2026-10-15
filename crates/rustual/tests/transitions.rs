//! CSS `transition`s, color animation and `AnimateOptions` (delay / speed). All tests run under
//! tokio's paused clock, so interpolated values are deterministic.

use rustual::css::{ColorValue, Scalar};
use rustual::prelude::*;
use rustual::Node;
use std::time::Duration;

fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
}

struct TApp;

impl Widget for TApp {
    fn compose(&mut self) -> Vec<Child> {
        compose![
            Static::new("hover me").id("b"),
            Static::new("mover").id("m"),
            Static::new("fader").id("f"),
            Static::new("wide").id("w"),
            Static::new("paint").id("c"),
        ]
    }
}

const CSS: &str = "
Screen { background: rgb(0, 0, 0); }
Static { height: 1; }
#b { width: 10; background: rgb(0, 0, 0); transition: background 1s; }
#b:hover { background: rgb(200, 100, 50); }
#m { width: 10; transition: offset 1s linear; }
#m.-active { offset: 10 0; }
#f { width: 10; transition: opacity 1s linear 500ms; }
#f.-hidden { opacity: 0; }
#w { width: 10; transition: width 1s in_out_cubic; }
#w.-wide { width: 30; }
#c { width: 10; background: rgb(0, 0, 0); }
";

async fn pilot_with(level: AnimationLevel) -> Pilot {
    App::new(TApp)
        .css(CSS)
        .animation_level(level)
        .test(Size::new(60, 12))
        .await
}

async fn pilot() -> Pilot {
    pilot_with(AnimationLevel::Full).await
}

fn node<'a>(p: &'a Pilot, sel: &str) -> &'a Node {
    let id = p.query(sel)[0];
    p.dom().get(id).unwrap()
}

fn bg(p: &Pilot, sel: &str) -> Color {
    node(p, sel).styles.get_background()
}

async fn class(p: &mut Pilot, sel: &str, class: &'static str, on: bool) {
    p.with::<Static, _>(sel, move |_w, ctx| ctx.set_class(on, class))
        .await;
}

#[tokio::test(start_paused = true)]
async fn css_parses_without_errors() {
    let p = pilot().await;
    assert!(p.css_errors().is_empty(), "{:?}", p.css_errors());
    let t = node(&p, "#f").styles.get_transition("opacity").unwrap();
    assert_eq!((t.duration, t.delay), (ms(1000), ms(500)));
}

#[tokio::test(start_paused = true)]
async fn hover_transition_on_background() {
    let mut p = pilot().await;
    // First styling is not animated.
    assert_eq!(bg(&p, "#b"), Color::rgb(0, 0, 0));
    assert_eq!(p.transitions_running(), 0);
    p.hover("#b").await;
    assert_eq!(p.transitions_running(), 1);
    assert_eq!(bg(&p, "#b"), Color::rgb(0, 0, 0));
    p.wait(ms(500)).await;
    assert_eq!(bg(&p, "#b"), Color::rgb(100, 50, 25));
    // The compositor sees the interpolated value.
    let r = node(&p, "#b").layout.region;
    let canvas = p.canvas();
    assert_eq!(canvas.get(r.x, r.y).unwrap().bg, Color::rgb(100, 50, 25));
    p.wait(ms(500)).await;
    assert_eq!(bg(&p, "#b"), Color::rgb(200, 100, 50));
    assert_eq!(p.transitions_running(), 0);
    // Idle afterwards.
    p.wait(ms(500)).await;
    assert_eq!(bg(&p, "#b"), Color::rgb(200, 100, 50));
}

#[tokio::test(start_paused = true)]
async fn retarget_mid_transition_starts_from_current_value() {
    let mut p = pilot().await;
    p.hover("#b").await;
    p.wait(ms(500)).await;
    assert_eq!(bg(&p, "#b"), Color::rgb(100, 50, 25));
    // Leave: animate back to black from the current (mid) color.
    p.hover_at(55, 11).await;
    assert_eq!(bg(&p, "#b"), Color::rgb(100, 50, 25));
    p.wait(ms(500)).await;
    assert_eq!(bg(&p, "#b"), Color::rgb(50, 25, 12));
    p.wait(ms(500)).await;
    assert_eq!(bg(&p, "#b"), Color::rgb(0, 0, 0));
}

#[tokio::test(start_paused = true)]
async fn offset_transition_on_class_change() {
    let mut p = pilot().await;
    let x0 = node(&p, "#m").layout.region.x;
    class(&mut p, "#m", "-active", true).await;
    assert_eq!(node(&p, "#m").layout.region.x, x0);
    p.wait(ms(500)).await;
    assert_eq!(node(&p, "#m").styles.offset_x, Some(Scalar::cells(5.0)));
    assert_eq!(node(&p, "#m").layout.region.x, x0 + 5);
    p.wait(ms(500)).await;
    assert_eq!(node(&p, "#m").styles.offset_x, Some(Scalar::cells(10.0)));
    assert_eq!(node(&p, "#m").layout.region.x, x0 + 10);
    // And back (the base rule declares the transition too).
    class(&mut p, "#m", "-active", false).await;
    p.wait(ms(250)).await;
    assert_eq!(node(&p, "#m").layout.region.x, x0 + 8);
    p.wait(ms(750)).await;
    assert_eq!(node(&p, "#m").layout.region.x, x0);
}

#[tokio::test(start_paused = true)]
async fn delay_is_honored() {
    let mut p = pilot().await;
    class(&mut p, "#f", "-hidden", true).await;
    p.wait(ms(400)).await;
    assert_eq!(node(&p, "#f").styles.get_opacity(), 1.0);
    assert_eq!(p.transitions_running(), 1);
    p.wait(ms(600)).await; // 500ms after the delay ended
    let o = node(&p, "#f").styles.get_opacity();
    assert!((o - 0.5).abs() < 1e-4, "{o}");
    p.wait(ms(500)).await;
    assert_eq!(node(&p, "#f").styles.get_opacity(), 0.0);
    assert_eq!(p.transitions_running(), 0);
}

#[tokio::test(start_paused = true)]
async fn width_transition_with_easing() {
    let mut p = pilot().await;
    class(&mut p, "#w", "-wide", true).await;
    p.wait(ms(250)).await;
    // in_out_cubic(0.25) = 0.0625 → 10 + 20 * 0.0625 = 11.25 → 11
    assert_eq!(node(&p, "#w").layout.region.width, 11);
    p.wait(ms(250)).await;
    assert_eq!(node(&p, "#w").layout.region.width, 20);
    p.wait(ms(500)).await;
    assert_eq!(node(&p, "#w").layout.region.width, 30);
}

#[tokio::test(start_paused = true)]
async fn animation_level_none_jumps() {
    let mut p = pilot_with(AnimationLevel::None).await;
    p.hover("#b").await;
    assert_eq!(bg(&p, "#b"), Color::rgb(200, 100, 50));
    assert_eq!(p.transitions_running(), 0);
    class(&mut p, "#m", "-active", true).await;
    assert_eq!(node(&p, "#m").styles.offset_x, Some(Scalar::cells(10.0)));
    // Basic also skips CSS transitions (they are `full`-level animations, as in Textual).
    let mut p = pilot_with(AnimationLevel::Basic).await;
    p.hover("#b").await;
    assert_eq!(bg(&p, "#b"), Color::rgb(200, 100, 50));
}

#[tokio::test(start_paused = true)]
async fn animate_color() {
    let mut p = pilot().await;
    p.with::<Static, _>("#c", |_w, ctx| {
        let id = ctx.id();
        ctx.animate_color(
            id,
            StyleProp::Background,
            Color::rgb(200, 0, 100),
            ms(1000),
            Easing::Linear,
        );
        ctx.animate_color(
            id,
            StyleProp::Color,
            Color::rgb(0, 0, 0),
            ms(1000),
            Easing::Linear,
        );
    })
    .await;
    assert_eq!(p.animations_running(), 2);
    p.wait(ms(500)).await;
    assert_eq!(bg(&p, "#c"), Color::rgb(100, 0, 50));
    assert_eq!(
        node(&p, "#c").inline_styles.background,
        Some(Color::rgb(100, 0, 50))
    );
    p.wait(ms(500)).await;
    assert_eq!(bg(&p, "#c"), Color::rgb(200, 0, 100));
    assert_eq!(
        node(&p, "#c").styles.color,
        Some(ColorValue::Color(Color::rgb(0, 0, 0)))
    );
    assert_eq!(p.animations_running(), 0);
}

#[tokio::test(start_paused = true)]
async fn animate_color_respects_level() {
    let mut p = pilot_with(AnimationLevel::None).await;
    p.with::<Static, _>("#c", |_w, ctx| {
        let id = ctx.id();
        ctx.animate_color(
            id,
            StyleProp::Tint,
            Color::rgb(9, 9, 9),
            ms(1000),
            Easing::Linear,
        );
    })
    .await;
    p.wait(ms(20)).await;
    assert_eq!(node(&p, "#c").styles.tint, Some(Color::rgb(9, 9, 9)));
}

#[derive(Default)]
struct Probe {
    value: f32,
    done: bool,
}

impl Widget for Probe {
    fn type_names(&self) -> &'static [&'static str] {
        &["Probe"]
    }
}

struct ProbeApp;

impl Widget for ProbeApp {
    fn compose(&mut self) -> Vec<Child> {
        compose![Probe::default().id("p")]
    }
}

#[tokio::test(start_paused = true)]
async fn animate_value_delay_and_speed() {
    let mut p = App::new(ProbeApp).test(Size::new(20, 5)).await;
    p.with::<Probe, _>("#p", |_w, ctx| {
        ctx.animate_value_opts::<Probe>(
            "x",
            0.0,
            100.0,
            AnimateOptions::default()
                .speed(50.0) // 100 units at 50/s → 2s
                .delay(ms(500))
                .easing(Easing::Linear),
            |w, v, _| w.value = v,
            Some(Box::new(|w: &mut Probe, _ctx: &mut Ctx| w.done = true)),
        );
    })
    .await;
    p.wait(ms(400)).await;
    assert_eq!(p.query_one::<Probe>("#p").unwrap().value, 0.0);
    assert_eq!(p.animations_running(), 1);
    p.wait(ms(1100)).await; // 1s into the 2s animation
    let v = p.query_one::<Probe>("#p").unwrap().value;
    assert!((v - 50.0).abs() < 1e-3, "{v}");
    p.wait(ms(1000)).await;
    let w = p.query_one::<Probe>("#p").unwrap();
    assert_eq!((w.value, w.done), (100.0, true));

    assert_eq!(
        AnimateOptions::new(ms(300)).duration_for(0.0, 10.0),
        ms(300)
    );
    assert_eq!(
        AnimateOptions::default()
            .speed(10.0)
            .duration_for(5.0, -5.0),
        ms(1000)
    );
}

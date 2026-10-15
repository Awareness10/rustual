//! Tests for the animator: `ctx.animate_value`, style animation, smooth scrolling, and animated
//! widgets (Switch, Tabs). All run under tokio's paused clock, so values are deterministic.

use rustual::prelude::*;
use std::time::Duration;

fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
}

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-3
}

#[derive(Default)]
struct Probe {
    value: f32,
    frames: usize,
    completed: Vec<&'static str>,
}

impl Widget for Probe {
    fn type_names(&self) -> &'static [&'static str] {
        &["Probe"]
    }
    fn render(&self, _ctx: &RenderCtx) -> Content {
        Content::Text(Text::plain(format!("{:.2}", self.value)))
    }
}

struct ProbeApp;

impl Widget for ProbeApp {
    fn compose(&mut self) -> Vec<Child> {
        compose![
            Probe::default().id("p"),
            Static::new("box").id("box"),
            ScrollableContainer::new()
                .id("sc")
                .with((0..100).map(|i| Child::from(Label::new(format!("line {i}")))))
        ]
    }
}

const CSS: &str = "
#p { height: 1; }
#box { width: 10; height: 1; }
#sc { height: 10; }
";

async fn pilot() -> Pilot {
    App::new(ProbeApp).css(CSS).test(Size::new(40, 20)).await
}

fn probe(p: &Pilot) -> &Probe {
    p.query_one::<Probe>("#p").unwrap()
}

async fn start(p: &mut Pilot, key: &'static str, from: f32, to: f32, d: u64, easing: Easing) {
    p.with::<Probe, _>("#p", move |_w, ctx| {
        ctx.animate_value_with::<Probe>(
            key,
            from,
            to,
            ms(d),
            easing,
            AnimationLevel::Full,
            |w, v, _ctx| {
                w.value = v;
                w.frames += 1;
            },
            Some(Box::new(move |w: &mut Probe, _ctx: &mut Ctx| {
                w.completed.push(key)
            })),
        );
    })
    .await;
}

#[tokio::test(start_paused = true)]
async fn linear_value_start_middle_end() {
    let mut p = pilot().await;
    start(&mut p, "x", 0.0, 100.0, 1000, Easing::Linear).await;
    assert_eq!(p.animations_running(), 1);
    assert_eq!(probe(&p).value, 0.0, "first frame comes one frame later");
    p.wait(ms(250)).await;
    assert!(close(probe(&p).value, 25.0), "{}", probe(&p).value);
    p.wait(ms(250)).await;
    assert!(close(probe(&p).value, 50.0), "{}", probe(&p).value);
    assert!(p.screen_text().contains("50.00"));
    // ~60 fps.
    let frames = probe(&p).frames;
    assert!((25..=35).contains(&frames), "frames: {frames}");
    assert!(probe(&p).completed.is_empty());
    p.wait(ms(500)).await;
    assert_eq!(probe(&p).value, 100.0);
    assert_eq!(probe(&p).completed, vec!["x"]);
    assert_eq!(p.animations_running(), 0);
    // Idle: no more frames.
    let frames = probe(&p).frames;
    p.wait(ms(500)).await;
    assert_eq!(probe(&p).frames, frames);
}

#[tokio::test(start_paused = true)]
async fn easing_is_applied() {
    let mut p = pilot().await;
    start(&mut p, "x", 0.0, 100.0, 1000, Easing::InOutCubic).await;
    p.wait(ms(250)).await;
    assert!(close(probe(&p).value, 6.25), "{}", probe(&p).value);
    p.wait(ms(250)).await;
    assert!(close(probe(&p).value, 50.0), "{}", probe(&p).value);
    p.wait(ms(250)).await;
    assert!(close(probe(&p).value, 93.75), "{}", probe(&p).value);
    p.wait(ms(250)).await;
    assert_eq!(probe(&p).value, 100.0);

    // Values below the start (to < from) and an easing that overshoots.
    start(&mut p, "y", 10.0, 0.0, 1000, Easing::OutBack).await;
    p.wait(ms(500)).await;
    let want = 10.0 - 10.0 * 1.0876975;
    assert!(close(probe(&p).value, want), "{}", probe(&p).value);
}

#[tokio::test(start_paused = true)]
async fn same_key_replaces_animation() {
    let mut p = pilot().await;
    start(&mut p, "x", 0.0, 100.0, 1000, Easing::Linear).await;
    p.wait(ms(500)).await;
    assert!(close(probe(&p).value, 50.0));
    // Replace: the old animation stops (its completion callback still runs), the new one drives.
    start(&mut p, "x", 50.0, 0.0, 500, Easing::Linear).await;
    assert_eq!(p.animations_running(), 1);
    p.wait(ms(250)).await;
    assert!(close(probe(&p).value, 25.0), "{}", probe(&p).value);
    assert_eq!(
        probe(&p).completed,
        vec!["x"],
        "replaced animation completed"
    );
    p.wait(ms(250)).await;
    assert_eq!(probe(&p).value, 0.0);
    assert_eq!(probe(&p).completed, vec!["x", "x"]);
    // A different key runs alongside.
    start(&mut p, "a", 0.0, 1.0, 100, Easing::Linear).await;
    start(&mut p, "b", 0.0, 1.0, 200, Easing::Linear).await;
    assert_eq!(p.animations_running(), 2);
    p.wait(ms(200)).await;
    assert_eq!(probe(&p).completed, vec!["x", "x", "a", "b"]);
}

#[tokio::test(start_paused = true)]
async fn stop_animation_and_level_none() {
    let mut p = pilot().await;
    start(&mut p, "x", 0.0, 100.0, 1000, Easing::Linear).await;
    p.wait(ms(100)).await;
    p.with::<Probe, _>("#p", |_w, ctx| {
        assert!(ctx.is_animating("x"));
        ctx.stop_animation("x", true);
        assert!(!ctx.is_animating("x"));
    })
    .await;
    // The final value is applied (and on_complete runs) on the next frame.
    p.wait(ms(20)).await;
    assert_eq!(probe(&p).value, 100.0);
    assert_eq!(probe(&p).completed, vec!["x"]);

    // With animations off everything jumps to its final value on the next frame.
    let mut p = App::new(ProbeApp)
        .css(CSS)
        .animation_level(AnimationLevel::None)
        .test(Size::new(40, 20))
        .await;
    start(&mut p, "x", 0.0, 100.0, 1000, Easing::Linear).await;
    p.wait(ms(17)).await;
    assert_eq!(probe(&p).value, 100.0);
    assert_eq!(probe(&p).completed, vec!["x"]);
}

#[tokio::test(start_paused = true)]
async fn style_opacity_and_offset() {
    let mut p = pilot().await;
    let id = p.query("#box")[0];
    let x0 = p.dom().get(id).unwrap().layout.region.x;
    p.with::<Probe, _>("#p", move |_w, ctx| {
        ctx.animate_style(id, StyleProp::Opacity, 0.0, ms(1000), Easing::Linear);
        ctx.animate_style(id, StyleProp::OffsetX, 10.0, ms(1000), Easing::Linear);
        ctx.animate_style(id, StyleProp::Width, 20.0, ms(1000), Easing::Linear);
    })
    .await;
    p.wait(ms(500)).await;
    let node = p.dom().get(id).unwrap();
    assert!(close(node.inline_styles.opacity.unwrap(), 0.5));
    assert_eq!(node.layout.region.x, x0 + 5);
    assert_eq!(node.layout.region.width, 15);
    // The half-transparent text is blended with the background.
    let y = node.layout.region.y;
    let c = p.canvas();
    let cell = c.get(x0 + 5, y).unwrap().clone();
    assert_eq!(cell.symbol, "b");
    p.wait(ms(500)).await;
    let node = p.dom().get(id).unwrap();
    assert_eq!(node.inline_styles.opacity, Some(0.0));
    assert_eq!(node.layout.region.x, x0 + 10);
    assert_eq!(node.layout.region.width, 20);
    assert!(!p.screen_text().contains("box"), "fully transparent");
}

#[tokio::test(start_paused = true)]
async fn smooth_scroll_reaches_target() {
    let mut p = pilot().await;
    let sc = p.query("#sc")[0];
    p.with::<ScrollableContainer, _>("#sc", |_w, ctx| {
        ctx.scroll_to_animated(0, 50, Some(ms(500)));
        assert_eq!(ctx.scroll_target(), Offset::new(0, 50));
    })
    .await;
    assert_eq!(p.dom().get(sc).unwrap().scroll_offset.y, 0);
    p.wait(ms(250)).await;
    // out_cubic(0.5) = 0.875
    assert_eq!(p.dom().get(sc).unwrap().scroll_offset.y, 44);
    p.wait(ms(250)).await;
    assert_eq!(p.dom().get(sc).unwrap().scroll_offset.y, 50);
    assert!(p.screen_text().contains("line 50"));

    // Targets are clamped to the max scroll; no duration = Textual's 50 cells/s.
    p.with::<ScrollableContainer, _>("#sc", |_w, ctx| {
        ctx.scroll_to_animated(0, 1000, None);
    })
    .await;
    p.wait(ms(2000)).await;
    let node = p.dom().get(sc).unwrap();
    assert_eq!(node.scroll_offset.y, node.layout.max_scroll().y);

    // A direct scroll cancels a smooth scroll in progress.
    p.with::<ScrollableContainer, _>("#sc", |_w, ctx| {
        ctx.scroll_to_animated(0, 0, Some(ms(500)));
        ctx.scroll_to(0, 7);
    })
    .await;
    p.wait(ms(500)).await;
    assert_eq!(p.dom().get(sc).unwrap().scroll_offset.y, 7);
}

#[tokio::test(start_paused = true)]
async fn keyboard_scrolling_animates_and_wheel_does_not() {
    let mut p = pilot().await;
    let sc = p.query("#sc")[0];
    p.with::<ScrollableContainer, _>("#sc", |_w, ctx| ctx.focus_self())
        .await;
    p.press(&["pagedown"]).await;
    assert_eq!(p.dom().get(sc).unwrap().scroll_offset.y, 0, "not yet");
    p.wait(ms(100)).await;
    let mid = p.dom().get(sc).unwrap().scroll_offset.y;
    assert!(mid > 0 && mid < 10, "mid-scroll: {mid}");
    // Relative scrolls continue from the animation's target.
    p.press(&["pagedown"]).await;
    p.wait(ms(1000)).await;
    assert_eq!(p.dom().get(sc).unwrap().scroll_offset.y, 20);

    // Mouse wheel scrolls immediately (Textual: animate=False).
    let r = p.dom().get(sc).unwrap().layout.content_region;
    p.scroll_down_at(r.x + 1, r.y + 1).await;
    assert_eq!(p.dom().get(sc).unwrap().scroll_offset.y, 23);
}

#[tokio::test(start_paused = true)]
async fn switch_slides() {
    struct S;
    impl Widget for S {
        fn compose(&mut self) -> Vec<Child> {
            compose![
                Switch::new().id("sw"),
                Switch::new().animate(false).id("instant")
            ]
        }
    }
    let mut p = App::new(S).test(Size::new(20, 8)).await;
    p.press(&["enter"]).await;
    let pos = |p: &Pilot, sel: &str| p.query_one::<Switch>(sel).unwrap().slider_position();
    assert_eq!(pos(&p, "#sw"), 0.0);
    p.wait(ms(150)).await;
    assert!(close(pos(&p, "#sw"), 0.5), "{}", pos(&p, "#sw"));
    p.wait(ms(150)).await;
    assert_eq!(pos(&p, "#sw"), 1.0);

    p.press(&["tab", "enter"]).await;
    assert_eq!(pos(&p, "#instant"), 1.0, "animate(false) is instant");
}

#[tokio::test(start_paused = true)]
async fn tabs_underline_slides() {
    struct T;
    impl Widget for T {
        fn compose(&mut self) -> Vec<Child> {
            compose![Tabs::new(["Alpha", "Beta"]).id("tabs")]
        }
    }
    let mut p = App::new(T).test(Size::new(30, 4)).await;
    let bar = |p: &mut Pilot| p.screen_text().lines().nth(1).unwrap().to_string();
    // "Alpha" content cells 1..6, "Beta" 8..12.
    let start = bar(&mut p);
    assert!(start.starts_with("╸━━━━━╺"), "{start}");
    p.press(&["right"]).await;
    assert_eq!(bar(&mut p), start, "slide starts from the old tab");
    p.wait(ms(150)).await;
    let mid = bar(&mut p);
    assert_ne!(mid, start);
    // Half way (in_out_cubic(0.5) = 0.5): highlight 4.5..9
    let cells: Vec<char> = mid.chars().collect();
    assert_eq!(cells[4], '╺', "{mid}");
    assert_eq!(cells[8], '━', "{mid}");
    p.wait(ms(150)).await;
    let end = bar(&mut p);
    assert!(end.starts_with("━━━━━━━╸━━━━╺"), "{end}");
}

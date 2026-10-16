//! Header: title / sub-title, clock, tall toggle.

use rustual::prelude::*;
use rustual::widgets::header::HeaderClock;
use std::time::Duration;

struct T {
    clock: bool,
}
impl Widget for T {
    fn compose(&mut self) -> Vec<Child> {
        compose![
            Header::new().show_clock(self.clock).id("header"),
            Label::new("body")
        ]
    }
}

fn header_height(pilot: &Pilot) -> i32 {
    let id = pilot.query("#header")[0];
    pilot.dom().get(id).unwrap().layout.region.height
}

#[tokio::test]
async fn header_shows_title_and_sub_title() {
    let mut pilot = App::new(T { clock: false })
        .title("My App")
        .sub_title("subtitle")
        .test(Size::new(60, 6))
        .await;
    assert!(pilot.css_errors().is_empty(), "{:?}", pilot.css_errors());
    let text = pilot.screen_text();
    let first = text.lines().next().unwrap().to_string();
    assert!(first.contains("My App — subtitle"), "{text}");
    assert!(first.starts_with(" ⭘"), "{first:?}");
    // The body is laid out below the docked header.
    assert_eq!(text.lines().nth(1).unwrap().trim(), "body");
    // The sub-title is dim: Textual (in truecolor mode) shows `dim` as the text color blended
    // 66% from the background. The header background ($panel) differs from the screen's.
    let c = pilot.canvas();
    let x = first[..first.find("subtitle").unwrap()].chars().count() as i32;
    let cell = c.get(x, 0).unwrap();
    let title = c.get(x - 4, 0).unwrap();
    assert!(!cell.attrs.contains(rustual::render::Attrs::DIM));
    assert_eq!(
        cell.fg,
        cell.bg.blend(title.fg, rustual::filter::DIM_FACTOR)
    );
    assert_ne!(cell.bg, c.get(0, 3).unwrap().bg);
}

#[tokio::test]
async fn header_title_follows_set_title() {
    struct S;
    impl Widget for S {
        fn compose(&mut self) -> Vec<Child> {
            compose![Header::new()]
        }
        fn bindings(&self) -> Vec<Binding> {
            vec![Binding::new("t", "retitle", "Retitle")]
        }
        fn run_action(&mut self, a: &str, _: &[String], ctx: &mut Ctx) -> bool {
            if a == "retitle" {
                ctx.set_title("Renamed");
                return true;
            }
            false
        }
    }
    let mut pilot = App::new(S).test(Size::new(40, 3)).await;
    assert!(!pilot.screen_text().contains("Renamed"));
    pilot.press(&["t"]).await;
    assert!(pilot
        .screen_text()
        .lines()
        .next()
        .unwrap()
        .contains("Renamed"));
}

fn parse_hms(s: &str) -> i64 {
    let p: Vec<i64> = s.split(':').map(|x| x.parse().unwrap()).collect();
    p[0] * 3600 + p[1] * 60 + p[2]
}

#[tokio::test(start_paused = true)]
async fn header_clock_ticks_every_second() {
    let mut pilot = App::new(T { clock: true }).test(Size::new(60, 4)).await;
    let t0 = pilot
        .query_one::<HeaderClock>("HeaderClock")
        .unwrap()
        .time_text();
    assert_eq!(t0.len(), 8);
    let first = pilot.screen_text().lines().next().unwrap().to_string();
    assert!(first.contains(&t0), "{first:?}");
    pilot.wait(Duration::from_secs(3)).await;
    let t1 = pilot
        .query_one::<HeaderClock>("HeaderClock")
        .unwrap()
        .time_text();
    assert_eq!((parse_hms(&t1) - parse_hms(&t0)).rem_euclid(86_400), 3);
    assert!(pilot.screen_text().contains(&t1));
}

#[tokio::test]
async fn clicking_header_toggles_tall() {
    let mut pilot = App::new(T { clock: false }).test(Size::new(40, 8)).await;
    assert_eq!(header_height(&pilot), 1);
    pilot.click("#header").await;
    assert!(pilot.query_one::<Header>("#header").unwrap().is_tall());
    let text = pilot.screen_text();
    assert_eq!(header_height(&pilot), 3);
    assert_eq!(text.lines().nth(3).unwrap().trim(), "body", "{text}");
    pilot.click("#header").await;
    assert_eq!(header_height(&pilot), 1);
}

#[test]
fn format_hms_wraps_days() {
    use rustual::widgets::header::format_hms;
    assert_eq!(format_hms(0), "00:00:00");
    assert_eq!(format_hms(86_400 + 3661), "01:01:01");
    assert_eq!(format_hms(-1), "23:59:59");
}

#[test]
fn format_clock_uses_strftime_and_the_time_zone() {
    use rustual::widgets::header::format_clock;
    // 1971-01-01 01:01:01 UTC.
    let t = Duration::from_secs(86_400 * 365 + 3661);
    assert_eq!(format_clock(t, Some(0), "%X"), "01:01:01");
    assert_eq!(format_clock(t, Some(3600), "%H:%M"), "02:01");
    assert_eq!(format_clock(t, Some(-7200), "%Y-%m-%d %H"), "1970-12-31 23");
    // Default (`None`): the system's local time zone, like Textual's `datetime.now()`.
    let local = chrono::DateTime::from_timestamp(t.as_secs() as i64, 0)
        .unwrap()
        .with_timezone(&chrono::Local);
    assert_eq!(
        format_clock(t, None, "%H:%M:%S %z"),
        local.format("%H:%M:%S %z").to_string()
    );
    // An invalid format is shown literally instead of panicking.
    assert_eq!(format_clock(t, Some(0), "%Q oops"), "%Q oops");
}

struct Formatted;
impl Widget for Formatted {
    fn compose(&mut self) -> Vec<Child> {
        compose![Header::new()
            .show_clock(true)
            .time_format("%H:%M")
            .clock_utc_offset(0)
            .id("header")]
    }
}

#[tokio::test(start_paused = true)]
async fn header_time_format_can_be_set_and_changed() {
    let mut pilot = App::new(Formatted).test(Size::new(60, 4)).await;
    let clock = |p: &mut Pilot| {
        p.query_one::<HeaderClock>("HeaderClock")
            .unwrap()
            .time_text()
    };
    let t = clock(&mut pilot);
    assert_eq!(t.len(), 5, "{t:?}");
    let first = pilot.screen_text().lines().next().unwrap().to_string();
    assert!(first.trim_end().ends_with(&t), "{first:?}");
    assert_eq!(
        pilot
            .query_one::<Header>("#header")
            .unwrap()
            .get_time_format(),
        "%H:%M"
    );

    pilot
        .with::<Header, _>("#header", |h, ctx| h.set_time_format("[%S]", ctx))
        .await;
    let t = clock(&mut pilot);
    assert!(
        t.starts_with('[') && t.ends_with(']') && t.len() == 4,
        "{t:?}"
    );
    assert!(pilot.screen_text().contains(&t));
}

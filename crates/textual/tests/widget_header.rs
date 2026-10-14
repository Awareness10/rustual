//! Header: title / sub-title, clock, tall toggle.

use std::time::Duration;
use textual::prelude::*;
use textual::widgets::header::HeaderClock;

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
    // The sub-title is dimmed; the header background ($panel) differs from the screen's.
    let c = pilot.canvas();
    let x = first[..first.find("subtitle").unwrap()].chars().count() as i32;
    let cell = c.get(x, 0).unwrap();
    assert!(cell.attrs.contains(textual::render::Attrs::DIM));
    assert!(!c
        .get(x - 4, 0)
        .unwrap()
        .attrs
        .contains(textual::render::Attrs::DIM));
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
    use textual::widgets::header::format_hms;
    assert_eq!(format_hms(0), "00:00:00");
    assert_eq!(format_hms(86_400 + 3661), "01:01:01");
    assert_eq!(format_hms(-1), "23:59:59");
}

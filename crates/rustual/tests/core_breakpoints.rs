//! `HORIZONTAL_BREAKPOINTS` / `VERTICAL_BREAKPOINTS`: classes on screens by terminal size.

use rustual::prelude::*;

struct Root;
impl Widget for Root {
    fn compose(&mut self) -> Vec<Child> {
        compose![Static::new("x").id("x")]
    }
}

fn classes(p: &Pilot) -> Vec<String> {
    let screen = p.active_screen().unwrap();
    let mut c: Vec<String> = p
        .dom()
        .get(screen)
        .unwrap()
        .classes
        .iter()
        .filter(|c| c.starts_with('-') && !c.starts_with("--"))
        .cloned()
        .collect();
    c.sort();
    c
}

#[tokio::test]
async fn breakpoint_classes_follow_the_terminal_size() {
    let mut p = App::new(Root)
        .horizontal_breakpoints(&[(0, "-narrow"), (40, "-normal"), (80, "-wide")])
        .vertical_breakpoints(&[(0, "-short"), (30, "-tall")])
        .css("#x { width: 10; } Screen.-wide #x { width: 20; }")
        .test(Size::new(50, 20))
        .await;
    assert_eq!(classes(&p), vec!["-normal", "-short"]);
    p.resize(Size::new(100, 40)).await;
    assert_eq!(classes(&p), vec!["-tall", "-wide"]);
    // The classes drive CSS.
    let x = p.query("#x")[0];
    assert_eq!(p.dom().get(x).unwrap().layout.region.width, 20);
    p.resize(Size::new(10, 40)).await;
    assert_eq!(classes(&p), vec!["-narrow", "-tall"]);

    // A screen's own list replaces the app's.
    p.with::<Root, _>("App", |_, ctx| {
        let screen = ctx.screen();
        ctx.for_node(screen, |c| {
            c.set_screen_breakpoints(Some(vec![(0, "-tiny".into())]), Some(vec![]))
        })
    })
    .await;
    assert_eq!(classes(&p), vec!["-tiny"]);
}

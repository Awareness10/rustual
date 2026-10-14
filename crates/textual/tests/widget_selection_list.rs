//! SelectionList tests.

use textual::prelude::*;
use textual::widgets::selection_list::{
    SelectedChanged, Selection, SelectionHighlighted, SelectionToggled,
};

struct SlApp {
    n: usize,
    events: Vec<String>,
}

impl Widget for SlApp {
    fn compose(&mut self) -> Vec<Child> {
        let mut sl = SelectionList::new()
            .selection(("Falken's Maze", 0))
            .selection(("Black Jack", 1, true))
            .selection(Selection::new("Gin Rummy", 2).id("gin"))
            .selection(Selection::new("Hearts", 3).disabled(true))
            .selection(("Bridge", 4));
        for i in 0..self.n {
            sl = sl.selection((format!("extra {i}"), 100 + i as i32));
        }
        compose![sl.id("sl")]
    }
    fn on_message(&mut self, m: &Message, _ctx: &mut Ctx) -> Flow {
        if let Some(h) = m.get::<SelectionHighlighted<i32>>() {
            self.events
                .push(format!("hl {} {}", h.selection_index, h.value));
        }
        if let Some(t) = m.get::<SelectionToggled<i32>>() {
            self.events.push(format!(
                "toggled {} {} {:?}",
                t.selection_index, t.value, t.selection_id
            ));
        }
        if let Some(c) = m.get::<SelectedChanged<i32>>() {
            self.events.push(format!(
                "changed {:?} {:?}",
                c.selected, c.selection_list_id
            ));
        }
        Flow::Continue
    }
}

async fn pilot(n: usize, size: Size) -> Pilot {
    App::new(SlApp { n, events: vec![] }).test(size).await
}

fn events(p: &Pilot) -> Vec<String> {
    p.app::<SlApp>().unwrap().events.clone()
}

fn list(p: &Pilot) -> &SelectionList<i32> {
    p.query_one::<SelectionList<i32>>("#sl").unwrap()
}

#[tokio::test]
async fn renders_buttons_and_prompts() {
    let mut p = pilot(0, Size::new(30, 10)).await;
    assert!(p.css_errors().is_empty(), "{:?}", p.css_errors());
    let text = p.screen_text();
    let lines: Vec<&str> = text.lines().collect();
    // Border (tall) on row 0, padding 0 1 → button starts at x = 2.
    assert!(lines[1].contains("▐X▌ Falken's Maze"), "{text}");
    assert!(lines[2].contains("▐X▌ Black Jack"), "{text}");
    assert!(lines[5].contains("▐X▌ Bridge"), "{text}");
    // Auto height: 5 options + border.
    assert_eq!(lines.iter().filter(|l| !l.trim().is_empty()).count(), 7);
    let c = p.canvas();
    // Selected button's X is colored differently from an unselected one.
    let unselected = c.get(3, 3).unwrap().fg;
    let selected = c.get(3, 2).unwrap().fg;
    assert_ne!(unselected, selected);
    // The highlighted option has a distinct background.
    assert_ne!(c.get(10, 1).unwrap().bg, c.get(10, 3).unwrap().bg);
    assert_eq!(list(&p).selected(), vec![1]);
    assert_eq!(list(&p).highlighted(), Some(0));
}

#[tokio::test]
async fn keyboard_toggles_and_skips_disabled() {
    let mut p = pilot(0, Size::new(30, 10)).await;
    p.press(&["space"]).await;
    assert_eq!(list(&p).selected(), vec![1, 0]);
    p.press(&["down", "down", "down", "enter"]).await;
    // Hearts (3) is disabled: down x3 = Black Jack, Gin Rummy, Bridge.
    assert_eq!(list(&p).highlighted(), Some(4));
    assert_eq!(list(&p).selected(), vec![1, 0, 4]);
    p.press(&["down"]).await;
    assert_eq!(list(&p).highlighted(), Some(0), "wraps");
    p.press(&["end"]).await;
    assert_eq!(list(&p).highlighted(), Some(4));
    p.press(&["home"]).await;
    assert_eq!(list(&p).highlighted(), Some(0));
    assert_eq!(
        events(&p)[..2],
        [
            "changed [1, 0] Some(\"sl\")".to_string(),
            "toggled 0 0 None".to_string()
        ]
    );
    assert!(events(&p).contains(&"hl 2 2".to_string()));
    assert!(!events(&p).contains(&"hl 3 3".to_string()));
}

#[tokio::test]
async fn click_toggles() {
    let mut p = pilot(0, Size::new(30, 10)).await;
    p.click_at(10, 3).await; // Gin Rummy
    assert_eq!(list(&p).highlighted(), Some(2));
    assert_eq!(list(&p).selected(), vec![1, 2]);
    assert!(events(&p).contains(&"toggled 2 2 Some(\"gin\")".to_string()));
    p.click_at(10, 3).await;
    assert_eq!(list(&p).selected(), vec![1]);
    p.click_at(10, 4).await; // disabled
    assert_eq!(list(&p).selected(), vec![1]);
}

#[tokio::test]
async fn bulk_operations_post_one_changed() {
    let mut p = pilot(0, Size::new(30, 10)).await;
    p.with::<SelectionList<i32>, _>("#sl", |sl, ctx| sl.select_all(ctx))
        .await;
    assert_eq!(list(&p).selected(), vec![1, 0, 2, 3, 4]);
    assert_eq!(
        events(&p),
        vec!["changed [1, 0, 2, 3, 4] Some(\"sl\")".to_string()]
    );
    p.with::<SelectionList<i32>, _>("#sl", |sl, ctx| sl.deselect_all(ctx))
        .await;
    assert!(list(&p).selected().is_empty());
    p.with::<SelectionList<i32>, _>("#sl", |sl, ctx| sl.toggle_all(ctx))
        .await;
    assert_eq!(list(&p).selected(), vec![0, 1, 2, 3, 4]);
    let ev = events(&p);
    let toggled = ev.iter().filter(|e| e.starts_with("toggled")).count();
    let changed = ev.iter().filter(|e| e.starts_with("changed")).count();
    assert_eq!((toggled, changed), (5, 3));
    // select / deselect by value; no message when nothing changes.
    let n = events(&p).len();
    let changed = p
        .with::<SelectionList<i32>, _>("#sl", |sl, ctx| sl.select(&2, ctx))
        .await;
    assert_eq!(changed, Some(false));
    assert_eq!(events(&p).len(), n);
    p.with::<SelectionList<i32>, _>("#sl", |sl, ctx| {
        sl.deselect(&2, ctx);
        sl.toggle(&4, ctx);
    })
    .await;
    assert_eq!(list(&p).selected(), vec![0, 1, 3]);
    assert!(list(&p).is_selected(&3));
}

#[tokio::test]
async fn add_remove_and_clear() {
    let mut p = pilot(0, Size::new(30, 12)).await;
    p.with::<SelectionList<i32>, _>("#sl", |sl, _ctx| {
        sl.add_option(("Chess", 5, true)).unwrap();
        assert!(sl.add_option(Selection::new("Dup", 6).id("gin")).is_err());
    })
    .await;
    assert!(p.screen_text().contains("Chess"));
    assert_eq!(list(&p).selected(), vec![1, 5]);
    p.with::<SelectionList<i32>, _>("#sl", |sl, ctx| {
        sl.remove_option_at_index(1, ctx).unwrap();
        assert_eq!(sl.get_option("gin").unwrap().value(), &2);
        assert_eq!(sl.get_option_index("gin"), Ok(1));
    })
    .await;
    assert_eq!(list(&p).selected(), vec![5]);
    assert!(!p.screen_text().contains("Black Jack"));
    assert!(events(&p).contains(&"changed [5] Some(\"sl\")".to_string()));
    p.with::<SelectionList<i32>, _>("#sl", |sl, ctx| sl.clear_options(ctx))
        .await;
    assert_eq!(list(&p).option_count(), 0);
    assert!(list(&p).selected().is_empty());
}

#[tokio::test]
async fn scrolls_to_highlight() {
    let mut p = pilot(30, Size::new(30, 10)).await;
    for _ in 0..12 {
        p.press(&["down"]).await;
    }
    let h = list(&p).highlighted().unwrap();
    let text = p.screen_text();
    assert!(!text.contains("Falken"), "{text}");
    assert!(text.contains(&format!("extra {}", h - 5)), "{text}");
    p.press(&["pagedown"]).await;
    assert!(list(&p).highlighted().unwrap() > h + 4);
    p.press(&["home"]).await;
    assert!(p.screen_text().contains("Falken"));
}

#[tokio::test]
async fn long_prompts_are_truncated_with_ellipsis() {
    struct Long;
    impl Widget for Long {
        fn compose(&mut self) -> Vec<Child> {
            compose![SelectionList::new()
                .selection(("A very long prompt that will not fit in the width", "x"))
                .compact(true)
                .id("sl")]
        }
    }
    let mut p = App::new(Long).test(Size::new(20, 5)).await;
    let text = p.screen_text();
    let first = text.lines().next().unwrap();
    assert!(first.starts_with("▐X▌ A very long"), "{text}");
    assert!(first.trim_end().ends_with('…'), "{text}");
}

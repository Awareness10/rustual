//! Tests for `Select`.

use rustual::prelude::*;
use rustual::widgets::select::{self, SelectOverlay};

struct SApp {
    select: Option<Select<i32>>,
    log: Vec<String>,
}

impl Widget for SApp {
    fn compose(&mut self) -> Vec<Child> {
        compose![
            self.select.take().unwrap().id("sel"),
            Label::new("under").id("under")
        ]
    }
    fn on_message(&mut self, msg: &Message, _ctx: &mut Ctx) -> Flow {
        if let Some(c) = msg.get::<select::Changed<i32>>() {
            self.log
                .push(format!("changed {:?} {:?}", c.value, c.select_id));
        }
        Flow::Continue
    }
}

fn colours() -> Select<i32> {
    Select::new([("Red", 1), ("Green", 2), ("Blue", 3)]).prompt("Colour")
}

async fn pilot(s: Select<i32>) -> Pilot {
    App::new(SApp {
        select: Some(s),
        log: vec![],
    })
    .test(Size::new(30, 12))
    .await
}

fn log(p: &Pilot) -> Vec<String> {
    p.app::<SApp>().unwrap().log.clone()
}

fn value(p: &Pilot) -> Option<i32> {
    p.query_one::<Select<i32>>("#sel")
        .unwrap()
        .get_value()
        .copied()
}

#[tokio::test]
async fn shows_prompt_and_arrow() {
    let mut p = pilot(colours()).await;
    assert!(p.css_errors().is_empty(), "{:?}", p.css_errors());
    let text = p.screen_text();
    let line = text.lines().nth(1).unwrap();
    assert!(line.contains("Colour") && line.contains('▼'), "{text}");
    assert!(!text.contains('▲'));
    assert!(text.lines().nth(3).unwrap().contains("under"), "{text}");
    assert_eq!(value(&p), None);
    assert!(log(&p).is_empty());
}

#[tokio::test]
async fn keyboard_open_choose_and_close() {
    let mut p = pilot(colours()).await;
    assert_eq!(p.focused_id().as_deref(), Some("sel"));
    p.press(&["enter"]).await;
    assert_eq!(p.screen_count(), 2, "overlay screen pushed");
    let text = p.screen_text();
    assert!(text.contains('▲'), "{text}");
    // The overlay sits below the SelectCurrent (rows 3..) and lists prompt + options.
    let rows: Vec<&str> = text.lines().collect();
    assert!(rows[4].contains("Colour"), "{text}");
    assert!(
        rows[5].contains("Red") && rows[7].contains("Blue"),
        "{text}"
    );
    let o = p.query_one::<SelectOverlay>("SelectOverlay").unwrap();
    assert_eq!(o.highlighted(), Some(0));
    p.press(&["down", "down", "enter"]).await;
    assert_eq!(p.screen_count(), 1);
    assert_eq!(value(&p), Some(2));
    assert_eq!(log(&p), vec!["changed Some(2) Some(\"sel\")"]);
    assert_eq!(p.focused_id().as_deref(), Some("sel"), "focus restored");
    let text = p.screen_text();
    assert!(text.lines().nth(1).unwrap().contains("Green"), "{text}");
    // Re-open: the current value is highlighted; escape dismisses without change.
    p.press(&["down"]).await;
    let o = p.query_one::<SelectOverlay>("SelectOverlay").unwrap();
    assert_eq!(o.highlighted(), Some(2));
    p.press(&["escape"]).await;
    assert_eq!(p.screen_count(), 1);
    assert_eq!(value(&p), Some(2));
    assert_eq!(log(&p).len(), 1);
}

#[tokio::test]
async fn mouse_open_and_pick() {
    let mut p = pilot(colours()).await;
    assert!(p.click("SelectCurrent").await);
    assert_eq!(p.screen_count(), 2);
    // Rows: 3 = overlay top border, 4 = blank, 5 Red, 6 Green, 7 Blue.
    p.click_at(5, 7).await;
    assert_eq!(p.screen_count(), 1);
    assert_eq!(value(&p), Some(3));
    // Click outside the list dismisses.
    p.click("SelectCurrent").await;
    p.click_at(5, 11).await;
    assert_eq!(p.screen_count(), 1);
    assert_eq!(value(&p), Some(3));
    // Picking the blank entry clears the value.
    p.click("SelectCurrent").await;
    p.click_at(5, 4).await;
    assert_eq!(value(&p), None);
    assert!(p.screen_text().lines().nth(1).unwrap().contains("Colour"));
}

#[tokio::test]
async fn type_to_search() {
    let mut p = pilot(colours()).await;
    p.press(&["enter"]).await;
    p.type_text("bl").await;
    let o = p.query_one::<SelectOverlay>("SelectOverlay").unwrap();
    assert_eq!(o.highlighted(), Some(3));
    p.press(&["enter"]).await;
    assert_eq!(value(&p), Some(3));
}

#[tokio::test]
async fn initial_value_no_blank_and_set_value() {
    let mut p = pilot(colours().allow_blank(false)).await;
    assert_eq!(value(&p), Some(1), "first option when blank not allowed");
    assert_eq!(log(&p), vec!["changed Some(1) Some(\"sel\")"]);
    p.with::<Select<i32>, _>("#sel", |s, ctx| {
        assert!(!s.clear(ctx));
        assert!(!s.set_value(Some(42), ctx));
        assert!(s.set_value(Some(3), ctx));
    })
    .await;
    assert_eq!(value(&p), Some(3));
    assert!(p.screen_text().contains("Blue"));

    let p2 = pilot(colours().value(2)).await;
    assert_eq!(value(&p2), Some(2));
}

#[tokio::test]
async fn from_values_and_set_options() {
    let mut p = App::new(FV).test(Size::new(30, 10)).await;
    p.press(&["enter"]).await;
    assert!(p.screen_text().contains("beta"));
    p.press(&["escape"]).await;
    p.with::<Select<String>, _>("#s", |s, ctx| {
        s.set_options([("One", "1".to_string())], ctx);
        assert_eq!(s.option_prompts(), vec!["One"]);
    })
    .await;
}

struct FV;
impl Widget for FV {
    fn compose(&mut self) -> Vec<Child> {
        compose![Select::from_values(["alpha".to_string(), "beta".to_string()]).id("s")]
    }
}

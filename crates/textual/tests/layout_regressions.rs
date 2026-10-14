use textual::prelude::*;

struct Row;
impl Widget for Row {
    fn compose(&mut self) -> Vec<Child> {
        compose![HorizontalGroup::new().with(compose![Label::new("Name:").id("l"), Input::new().id("i")])]
    }
}

#[tokio::test]
async fn percent_width_after_auto_sibling_does_not_overlap() {
    let mut pilot = App::new(Row).test(Size::new(40, 5)).await;
    let text = pilot.screen_text();
    println!("{text}");
    let l = pilot.query("#l")[0];
    let i = pilot.query("#i")[0];
    let lr = pilot.dom().get(l).unwrap().layout.region;
    let ir = pilot.dom().get(i).unwrap().layout.region;
    println!("label {lr:?} input {ir:?}");
    assert!(ir.x >= lr.right(), "input overlaps label: {lr:?} {ir:?}");
}

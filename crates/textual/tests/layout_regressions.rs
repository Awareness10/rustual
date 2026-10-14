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

struct Base;
impl Widget for Base {
    fn compose(&mut self) -> Vec<Child> {
        compose![Label::new("UNDERNEATH")]
    }
    fn bindings(&self) -> Vec<Binding> {
        vec![Binding::new("m", "modal", "Modal")]
    }
    fn run_action(&mut self, action: &str, _a: &[String], ctx: &mut Ctx) -> bool {
        if action == "modal" {
            ctx.push_modal(Vertical::new().with(compose![Static::new("dialog").style("width: 10; height: 1; margin: 2 20;")]));
            return true;
        }
        false
    }
}

#[tokio::test]
async fn modal_screen_dims_rather_than_erases_screen_below() {
    let mut pilot = App::new(Base).test(Size::new(40, 6)).await;
    let before = pilot.canvas().get(0, 0).unwrap().clone();
    pilot.press(&["m"]).await;
    let canvas = pilot.canvas();
    let text = canvas.to_text();
    assert!(text.contains("UNDERNEATH"), "{text}");
    assert!(text.contains("dialog"), "{text}");
    let after = canvas.get(0, 0).unwrap();
    assert_eq!(after.symbol, "U");
    assert_ne!(after.fg, before.fg, "text below should be dimmed");
}

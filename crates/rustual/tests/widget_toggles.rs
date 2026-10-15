//! Tests for ToggleButton / Checkbox, RadioButton / RadioSet and Switch.

use rustual::prelude::*;

struct Form {
    children: Vec<Child>,
    log: Vec<String>,
}

impl Widget for Form {
    fn compose(&mut self) -> Vec<Child> {
        std::mem::take(&mut self.children)
    }
    fn on_message(&mut self, m: &Message, _ctx: &mut Ctx) -> Flow {
        if let Some(c) = m.get::<checkbox::Changed>() {
            self.log
                .push(format!("checkbox {:?} {}", c.checkbox_id, c.value));
        }
        if let Some(c) = m.get::<toggle_button::Changed>() {
            self.log
                .push(format!("toggle {:?} {}", c.toggle_button_id, c.value));
        }
        if let Some(c) = m.get::<radio_button::Changed>() {
            self.log
                .push(format!("radio {:?} {}", c.radio_button_id, c.value));
        }
        if let Some(c) = m.get::<radio_set::Changed>() {
            self.log.push(format!(
                "set {:?} {} {}",
                c.radio_set_id, c.pressed_index, c.pressed_label
            ));
        }
        if let Some(c) = m.get::<switch::Changed>() {
            self.log
                .push(format!("switch {:?} {}", c.switch_id, c.value));
        }
        Flow::Continue
    }
}

async fn form(children: Vec<Child>, size: Size) -> Pilot {
    App::new(Form {
        children,
        log: vec![],
    })
    .test(size)
    .await
}

fn log(pilot: &Pilot) -> Vec<String> {
    pilot.app::<Form>().unwrap().log.clone()
}

fn has_class(pilot: &Pilot, sel: &str, class: &str) -> bool {
    let id = pilot.query(sel)[0];
    pilot.dom().get(id).unwrap().has_class(class)
}

fn row(pilot: &mut Pilot, y: usize) -> String {
    pilot
        .screen_text()
        .lines()
        .nth(y)
        .unwrap_or_default()
        .to_string()
}

// ---------------------------------------------------------------- Checkbox

#[tokio::test]
async fn checkbox_renders_and_toggles_with_keys() {
    let mut pilot = form(
        compose![Checkbox::new("Accept terms").id("cb")],
        Size::new(40, 4),
    )
    .await;
    assert!(pilot.css_errors().is_empty(), "{:?}", pilot.css_errors());
    let line = row(&mut pilot, 1);
    assert!(line.contains("▐X▌ Accept terms"), "{line:?}");
    assert_eq!(pilot.focused_id().as_deref(), Some("cb"));
    assert!(!has_class(&pilot, "#cb", "-on"));
    let off_fg = pilot.canvas().get(3, 1).unwrap().fg;

    pilot.press(&["enter"]).await;
    assert!(pilot.query_one::<Checkbox>("#cb").unwrap().get_value());
    assert!(has_class(&pilot, "#cb", "-on"));
    // The X changes color when on.
    assert_ne!(pilot.canvas().get(3, 1).unwrap().fg, off_fg);
    pilot.press(&["space"]).await;
    assert!(!pilot.query_one::<Checkbox>("#cb").unwrap().get_value());
    assert_eq!(
        log(&pilot),
        vec![
            "checkbox Some(\"cb\") true".to_string(),
            "checkbox Some(\"cb\") false".to_string()
        ]
    );
}

#[tokio::test]
async fn checkbox_click_and_focus_style() {
    let mut pilot = form(
        compose![
            Checkbox::new("One").id("a"),
            Checkbox::new("Two").value(true).id("b"),
        ],
        Size::new(40, 6),
    )
    .await;
    // Initial value: -on class, no message.
    assert!(has_class(&pilot, "#b", "-on"));
    assert!(log(&pilot).is_empty());
    assert!(pilot.click("#b").await);
    assert_eq!(pilot.focused_id().as_deref(), Some("b"));
    assert!(!pilot.query_one::<Checkbox>("#b").unwrap().get_value());
    assert_eq!(log(&pilot), vec!["checkbox Some(\"b\") false"]);
    // The focused checkbox's label uses the block cursor style; the other doesn't.
    let c = pilot.canvas();
    let focused_label = c.get(6, 4).unwrap().bg; // "T" of "Two"
    let blurred_label = c.get(6, 1).unwrap().bg; // "O" of "One"
    assert_ne!(focused_label, blurred_label);
}

#[tokio::test]
async fn checkbox_button_after_label_and_programmatic() {
    let mut pilot = form(
        compose![Checkbox::new("Later").button_first(false).id("c")],
        Size::new(40, 3),
    )
    .await;
    assert!(row(&mut pilot, 1).contains("Later ▐X▌"));
    pilot
        .with::<Checkbox, _>("#c", |c, ctx| c.set_value(true, ctx))
        .await;
    assert!(has_class(&pilot, "#c", "-on"));
    assert_eq!(log(&pilot), vec!["checkbox Some(\"c\") true"]);
    // Setting the same value again does nothing.
    pilot
        .with::<Checkbox, _>("#c", |c, ctx| c.set_value(true, ctx))
        .await;
    assert_eq!(log(&pilot).len(), 1);
}

#[tokio::test]
async fn disabled_checkbox_ignores_input() {
    let mut pilot = form(
        compose![
            Checkbox::new("Off limits").id("d").disabled(true),
            Checkbox::new("Ok").id("e"),
        ],
        Size::new(40, 6),
    )
    .await;
    assert_eq!(pilot.focused_id().as_deref(), Some("e"));
    pilot.click("#d").await;
    assert!(!pilot.query_one::<Checkbox>("#d").unwrap().get_value());
    assert_eq!(pilot.focused_id().as_deref(), Some("e"));
    assert!(log(&pilot).is_empty());
}

#[tokio::test]
async fn plain_toggle_button_posts_its_own_message() {
    let mut pilot = form(
        compose![ToggleButton::new("Toggle me").id("t")],
        Size::new(40, 3),
    )
    .await;
    assert!(pilot.css_errors().is_empty(), "{:?}", pilot.css_errors());
    pilot.press(&["space"]).await;
    assert_eq!(log(&pilot), vec!["toggle Some(\"t\") true"]);
}

#[tokio::test]
async fn standalone_radio_button() {
    let mut pilot = form(compose![RadioButton::new("Solo").id("r")], Size::new(40, 3)).await;
    assert!(row(&mut pilot, 1).contains("▐●▌ Solo"));
    assert_eq!(pilot.focused_id().as_deref(), Some("r"));
    pilot.press(&["enter"]).await;
    pilot.press(&["enter"]).await;
    assert_eq!(
        log(&pilot),
        vec!["radio Some(\"r\") true", "radio Some(\"r\") false"]
    );
}

// ---------------------------------------------------------------- RadioSet

#[tokio::test]
async fn radio_set_keyboard_navigation() {
    let mut pilot = form(
        compose![
            RadioSet::new(["Red", "Green", "Blue"]).id("colors"),
            Button::new("After").id("after"),
        ],
        Size::new(40, 10),
    )
    .await;
    assert!(pilot.css_errors().is_empty(), "{:?}", pilot.css_errors());
    let text = pilot.screen_text();
    assert!(text.contains("▐●▌ Red"), "{text}");
    assert!(text.contains("▐●▌ Blue"), "{text}");
    // The set takes focus; its buttons don't.
    assert_eq!(pilot.focused_id().as_deref(), Some("colors"));
    let set = pilot.query_one::<RadioSet>("#colors").unwrap();
    assert_eq!(set.selected_index(), Some(0));
    assert_eq!(set.pressed_index(), None);
    let buttons = pilot.query("#colors > RadioButton");
    assert_eq!(buttons.len(), 3);
    assert!(pilot.dom().get(buttons[0]).unwrap().has_class("-selected"));

    pilot.press(&["down", "down"]).await;
    assert_eq!(
        pilot
            .query_one::<RadioSet>("#colors")
            .unwrap()
            .selected_index(),
        Some(2)
    );
    assert!(pilot.dom().get(buttons[2]).unwrap().has_class("-selected"));
    assert!(!pilot.dom().get(buttons[0]).unwrap().has_class("-selected"));
    // Wraps around.
    pilot.press(&["right"]).await;
    assert_eq!(
        pilot
            .query_one::<RadioSet>("#colors")
            .unwrap()
            .selected_index(),
        Some(0)
    );
    pilot.press(&["up"]).await;
    assert_eq!(
        pilot
            .query_one::<RadioSet>("#colors")
            .unwrap()
            .selected_index(),
        Some(2)
    );
    pilot.press(&["left", "enter"]).await;
    assert_eq!(log(&pilot), vec!["set Some(\"colors\") 1 Green"]);
    assert!(pilot.dom().get(buttons[1]).unwrap().has_class("-on"));

    pilot.press(&["down", "space"]).await;
    assert!(!pilot.dom().get(buttons[1]).unwrap().has_class("-on"));
    assert!(pilot.dom().get(buttons[2]).unwrap().has_class("-on"));
    assert_eq!(
        pilot
            .query_one::<RadioSet>("#colors")
            .unwrap()
            .pressed_index(),
        Some(2)
    );
    assert_eq!(log(&pilot).last().unwrap(), "set Some(\"colors\") 2 Blue");
    // Pressing the pressed button again doesn't turn it off, and posts nothing.
    pilot.press(&["enter"]).await;
    assert!(pilot.dom().get(buttons[2]).unwrap().has_class("-on"));
    assert_eq!(log(&pilot).len(), 2);
    // Only one button is on.
    let on = buttons
        .iter()
        .filter(|b| pilot.dom().widget::<RadioButton>(**b).unwrap().get_value())
        .count();
    assert_eq!(on, 1);
    // Radio button messages are consumed by the set.
    assert!(log(&pilot).iter().all(|l| !l.starts_with("radio ")));
    // Tab leaves the set (buttons aren't in the focus chain).
    pilot.press(&["tab"]).await;
    assert_eq!(pilot.focused_id().as_deref(), Some("after"));
}

#[tokio::test]
async fn radio_set_mouse_and_explicit_buttons() {
    let mut pilot = form(
        compose![
            Button::new("Before").id("before"),
            RadioSet::default().id("rs").with(compose![
                RadioButton::new("A").value(true).id("a"),
                RadioButton::new("B").value(true).id("b"),
                RadioButton::new("C").id("c"),
            ]),
        ],
        Size::new(40, 12),
    )
    .await;
    // Only the first initially-on button stays on.
    assert!(pilot.query_one::<RadioButton>("#a").unwrap().get_value());
    assert!(!pilot.query_one::<RadioButton>("#b").unwrap().get_value());
    assert_eq!(
        pilot.query_one::<RadioSet>("#rs").unwrap().pressed_index(),
        Some(0)
    );
    assert_eq!(pilot.focused_id().as_deref(), Some("before"));

    // Clicking a button focuses the set and presses that button.
    assert!(pilot.click("#c").await);
    assert_eq!(pilot.focused_id().as_deref(), Some("rs"));
    assert!(pilot.query_one::<RadioButton>("#c").unwrap().get_value());
    assert!(!pilot.query_one::<RadioButton>("#a").unwrap().get_value());
    assert_eq!(log(&pilot), vec!["set Some(\"rs\") 2 C"]);
    assert_eq!(
        pilot.query_one::<RadioSet>("#rs").unwrap().selected_index(),
        Some(2)
    );
    // Clicking it again keeps it on.
    pilot.click("#c").await;
    assert!(pilot.query_one::<RadioButton>("#c").unwrap().get_value());
    assert_eq!(log(&pilot).len(), 1);
}

#[tokio::test]
async fn radio_set_skips_disabled_buttons() {
    let mut pilot = form(
        compose![RadioSet::default().id("rs").with(compose![
            RadioButton::new("A").id("a"),
            RadioButton::new("B").id("b").disabled(true),
            RadioButton::new("C").id("c"),
        ])],
        Size::new(40, 8),
    )
    .await;
    pilot.press(&["down"]).await;
    assert_eq!(
        pilot.query_one::<RadioSet>("#rs").unwrap().selected_index(),
        Some(2)
    );
}

#[tokio::test]
async fn disabled_radio_set() {
    let mut pilot = form(
        compose![
            RadioSet::new(["X", "Y"]).id("rs").disabled(true),
            Button::new("ok").id("ok"),
        ],
        Size::new(40, 8),
    )
    .await;
    assert_eq!(pilot.focused_id().as_deref(), Some("ok"));
    let b = pilot.query("#rs > RadioButton")[1];
    let r = pilot.dom().get(b).unwrap().layout.region;
    pilot.click_at(r.x + 1, r.y).await;
    assert!(log(&pilot).is_empty());
    assert_eq!(pilot.focused_id().as_deref(), Some("ok"));
}

// ---------------------------------------------------------------- Switch

#[tokio::test(start_paused = true)]
async fn switch_renders_and_toggles() {
    let mut pilot = form(compose![Switch::new().id("sw")], Size::new(20, 3)).await;
    assert!(pilot.css_errors().is_empty(), "{:?}", pilot.css_errors());
    // border (1) + padding (2): the slider is at x = 3..7 on row 1.
    let c = pilot.canvas();
    let left = c.get(3, 1).unwrap().bg;
    let right = c.get(5, 1).unwrap().bg;
    assert_eq!(c.get(4, 1).unwrap().bg, left);
    assert_eq!(c.get(6, 1).unwrap().bg, right);
    assert_ne!(left, right);
    let id = pilot.query("#sw")[0];
    assert_eq!(pilot.dom().get(id).unwrap().layout.region.width, 10);

    pilot.press(&["enter"]).await;
    assert!(pilot.query_one::<Switch>("#sw").unwrap().get_value());
    // The slider slides over 0.3s; `-on` is set when it arrives.
    assert!(!has_class(&pilot, "#sw", "-on"));
    pilot.wait(std::time::Duration::from_millis(300)).await;
    assert!(has_class(&pilot, "#sw", "-on"));
    let c = pilot.canvas();
    // Thumb moved right: the left half now shows the track color.
    assert_eq!(c.get(3, 1).unwrap().bg, c.get(4, 1).unwrap().bg);
    assert_ne!(c.get(3, 1).unwrap().bg, c.get(5, 1).unwrap().bg);
    assert_ne!(
        c.get(5, 1).unwrap().bg,
        left,
        "on thumb uses the success color"
    );

    pilot.press(&["space"]).await;
    pilot.click("#sw").await;
    assert_eq!(
        log(&pilot),
        vec![
            "switch Some(\"sw\") true",
            "switch Some(\"sw\") false",
            "switch Some(\"sw\") true"
        ]
    );
}

#[tokio::test]
async fn switch_initial_value_and_disabled() {
    let mut pilot = form(
        compose![
            Switch::new().value(true).id("on"),
            Switch::new().id("off").disabled(true),
        ],
        Size::new(20, 6),
    )
    .await;
    assert!(has_class(&pilot, "#on", "-on"));
    assert!(log(&pilot).is_empty());
    pilot.click("#off").await;
    assert!(!pilot.query_one::<Switch>("#off").unwrap().get_value());
    assert_eq!(pilot.focused_id().as_deref(), Some("on"));
    pilot.press(&["tab"]).await;
    assert_eq!(pilot.focused_id().as_deref(), Some("on"));
    assert!(log(&pilot).is_empty());
}

//! MaskedInput tests.

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
        if let Some(c) = m.get::<masked_input::Changed>() {
            self.log.push(format!("changed {}", c.value));
        }
        if let Some(s) = m.get::<masked_input::Submitted>() {
            self.log.push(format!("submitted {}", s.value));
        }
        Flow::Continue
    }
}

async fn form(children: Vec<Child>) -> Pilot {
    App::new(Form {
        children,
        log: vec![],
    })
    .test(Size::new(40, 8))
    .await
}

fn mi<'a>(p: &'a Pilot, sel: &str) -> &'a MaskedInput {
    p.query_one::<MaskedInput>(sel).unwrap()
}

fn value(p: &Pilot) -> String {
    mi(p, "#m").get_value().to_string()
}

fn cursor(p: &Pilot) -> usize {
    mi(p, "#m").cursor_position()
}

#[tokio::test]
async fn typing_inserts_separators_and_validates() {
    let mut p = form(compose![MaskedInput::new("9999-9999;_").id("m")]).await;
    assert!(p.css_errors().is_empty(), "{:?}", p.css_errors());
    p.type_text("1234").await;
    assert_eq!(value(&p), "1234-");
    assert_eq!(cursor(&p), 5);
    assert!(!mi(&p, "#m").is_valid());
    let text = p.screen_text();
    assert!(text.contains("1234-____"), "{text}");
    p.type_text("x").await; // rejected
    assert_eq!(value(&p), "1234-");
    assert_eq!(p.bell_count(), 1);
    p.type_text("5678").await;
    assert_eq!(value(&p), "1234-5678");
    assert!(mi(&p, "#m").is_valid());
    assert!(p
        .dom()
        .get(p.focused().unwrap())
        .unwrap()
        .has_class("-valid"));
    p.type_text("9").await; // past the end: ignored
    assert_eq!(value(&p), "1234-5678");
    p.press(&["enter"]).await;
    let log = p.app::<Form>().unwrap().log.clone();
    assert_eq!(log.first().map(String::as_str), Some("changed 1"));
    assert_eq!(log.last().map(String::as_str), Some("submitted 1234-5678"));
}

#[tokio::test]
async fn cursor_skips_separators_and_deletes() {
    let mut p = form(compose![MaskedInput::new("99-99").value("12-34").id("m")]).await;
    p.click("#m").await;
    p.press(&["end"]).await;
    assert_eq!(cursor(&p), 5);
    p.press(&["left", "left"]).await;
    assert_eq!(cursor(&p), 3);
    p.press(&["left"]).await;
    assert_eq!(cursor(&p), 1, "skips the '-'");
    p.press(&["delete"]).await;
    assert_eq!(value(&p), "1 -34");
    assert!(p.screen_text().contains("1 -34"));
    assert!(!mi(&p, "#m").is_valid());
    p.press(&["right"]).await;
    assert_eq!(cursor(&p), 3);
    p.press(&["backspace"]).await;
    // Backspace moves left over the separator and blanks position 1 again.
    assert_eq!(cursor(&p), 1);
    p.type_text("9").await;
    assert_eq!(value(&p), "19-34");
    assert_eq!(cursor(&p), 3);
    p.press(&["home"]).await;
    assert_eq!(cursor(&p), 0);
    p.press(&["ctrl+right"]).await;
    assert_eq!(cursor(&p), 3);
    p.press(&["ctrl+u"]).await;
    assert_eq!(value(&p), "  -34");
    assert_eq!(cursor(&p), 0);
}

#[tokio::test]
async fn case_conversion_and_placeholder() {
    let mut p = form(compose![MaskedInput::new(">AAA-999")
        .placeholder("ABC-123")
        .id("m")])
    .await;
    let text = p.screen_text();
    assert!(text.contains("ABC-123"), "{text}");
    p.click("#m").await;
    p.type_text("xyz42").await;
    assert_eq!(value(&p), "XYZ-42");
    assert!(
        p.screen_text().contains("XYZ-423"),
        "placeholder fills the rest"
    );
    let text = p.screen_text();
    let (row, line) = text
        .lines()
        .enumerate()
        .find(|(_, l)| l.contains("XYZ"))
        .unwrap();
    let chars: Vec<char> = line.chars().collect();
    let x = (0..chars.len())
        .find(|i| chars[*i..].starts_with(&['X', 'Y', 'Z']))
        .unwrap() as i32;
    let c = p.canvas();
    // Placeholder character is dimmer than typed text.
    assert_ne!(
        c.get(x, row as i32).unwrap().fg,
        c.get(x + 6, row as i32).unwrap().fg
    );
}

#[tokio::test]
async fn set_value_clear_and_valid_empty() {
    let mut p = form(compose![MaskedInput::new("(999) 999")
        .valid_empty(true)
        .id("m")])
    .await;
    assert_eq!(value(&p), "(");
    assert_eq!(mi(&p, "#m").mask(), "(   )    ");
    let ok = p
        .with::<MaskedInput, _>("#m", |m, ctx| m.set_value("(123) 456", ctx))
        .await;
    assert_eq!(ok, Some(true));
    assert!(mi(&p, "#m").is_valid());
    let bad = p
        .with::<MaskedInput, _>("#m", |m, ctx| m.set_value("(abc) 456", ctx))
        .await;
    assert_eq!(bad, Some(false));
    assert_eq!(value(&p), "(123) 456");
    p.with::<MaskedInput, _>("#m", |m, ctx| m.clear(ctx)).await;
    assert_eq!(value(&p), "(");
    assert!(mi(&p, "#m").matches_template("(123) 456"));
    assert!(!mi(&p, "#m").matches_template("(123) 45"));
}

#[tokio::test]
async fn paste_follows_mask() {
    let mut p = form(compose![MaskedInput::new("99/99/9999").id("m")]).await;
    p.click("#m").await;
    p.paste("01022024").await;
    assert_eq!(value(&p), "01/02/2024");
    p.press(&["ctrl+a"]).await;
    p.paste("1/").await;
    // Typing a separator early jumps to the next field.
    assert_eq!(value(&p), "1 /02/2024");
}

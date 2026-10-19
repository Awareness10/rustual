//! Tests for the `Input` widget.

use rustual::prelude::*;
use std::time::Duration;

/// Root that mounts the given children and records Input messages.
struct Form {
    children: Vec<Child>,
    log: Vec<String>,
}

impl Widget for Form {
    fn compose(&mut self) -> Vec<Child> {
        std::mem::take(&mut self.children)
    }
    fn on_message(&mut self, m: &Message, _ctx: &mut Ctx) -> Flow {
        if let Some(c) = m.get::<input::Changed>() {
            self.log
                .push(format!("changed {:?} {}", c.input_id, c.value));
        }
        if let Some(s) = m.get::<input::Submitted>() {
            self.log
                .push(format!("submitted {:?} {}", s.input_id, s.value));
        }
        if let Some(b) = m.get::<input::Blurred>() {
            self.log
                .push(format!("blurred {:?} {}", b.input_id, b.value));
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

fn value(pilot: &Pilot, sel: &str) -> String {
    pilot
        .query_one::<Input>(sel)
        .unwrap()
        .get_value()
        .to_string()
}

fn cursor(pilot: &Pilot, sel: &str) -> usize {
    pilot.query_one::<Input>(sel).unwrap().cursor_position()
}

/// Text of the given screen row.
fn row(pilot: &mut Pilot, y: usize) -> String {
    pilot
        .screen_text()
        .lines()
        .nth(y)
        .unwrap_or_default()
        .to_string()
}

/// Characters `a..b` of a line (cells == chars for these tests).
fn cols(line: &str, a: usize, b: usize) -> String {
    line.chars().skip(a).take(b - a).collect()
}

#[tokio::test]
async fn renders_placeholder_and_css_parses() {
    let mut pilot = form(
        compose![
            Input::new().placeholder("Your name").id("name"),
            Input::new().placeholder("Other").id("other"),
        ],
        Size::new(30, 6),
    )
    .await;
    assert!(pilot.css_errors().is_empty(), "{:?}", pilot.css_errors());
    let text = pilot.screen_text();
    assert!(text.contains("Your name"), "{text}");
    assert!(text.contains("Other"), "{text}");
    // Height 3 (tall border) per Input, content on the middle row; padding 0 2 + border.
    assert_eq!(cols(&row(&mut pilot, 1), 3, 12), "Your name");
    assert_eq!(cols(&row(&mut pilot, 4), 3, 8), "Other");
    // Focused input: the cursor is drawn over the first placeholder character.
    assert_eq!(pilot.focused_id().as_deref(), Some("name"));
    let c = pilot.canvas();
    let cur = c.get(3, 1).unwrap();
    let next = c.get(4, 1).unwrap();
    assert_ne!(
        cur.bg, next.bg,
        "cursor cell should have the cursor background"
    );
    // The unfocused input shows no cursor.
    assert_eq!(c.get(3, 4).unwrap().bg, c.get(4, 4).unwrap().bg);
}

#[tokio::test]
async fn typing_posts_changed_and_enter_submits() {
    let mut pilot = form(compose![Input::new().id("name")], Size::new(30, 3)).await;
    pilot.type_text("Hi there").await;
    assert_eq!(value(&pilot, "#name"), "Hi there");
    assert_eq!(cursor(&pilot, "#name"), 8);
    assert!(row(&mut pilot, 1).contains("Hi there"));
    let l = log(&pilot);
    assert_eq!(l.len(), 8);
    assert_eq!(l[0], "changed Some(\"name\") H");
    assert_eq!(l[7], "changed Some(\"name\") Hi there");
    pilot.press(&["enter"]).await;
    assert_eq!(
        log(&pilot).last().unwrap(),
        "submitted Some(\"name\") Hi there"
    );
}

#[tokio::test]
async fn cursor_movement_and_deletion_keys() {
    let mut pilot = form(
        compose![Input::new().value("hello world foo").id("i")],
        Size::new(40, 3),
    )
    .await;
    // Auto-focus selected everything (select_on_focus); move to collapse it.
    pilot.press(&["end"]).await;
    assert_eq!(cursor(&pilot, "#i"), 15);
    pilot.press(&["left", "left"]).await;
    assert_eq!(cursor(&pilot, "#i"), 13);
    pilot.press(&["home"]).await;
    assert_eq!(cursor(&pilot, "#i"), 0);
    pilot.press(&["left"]).await;
    assert_eq!(cursor(&pilot, "#i"), 0);
    pilot.press(&["ctrl+right"]).await;
    assert_eq!(cursor(&pilot, "#i"), 6);
    pilot.press(&["ctrl+right"]).await;
    assert_eq!(cursor(&pilot, "#i"), 12);
    pilot.press(&["ctrl+right"]).await;
    assert_eq!(cursor(&pilot, "#i"), 15);
    pilot.press(&["ctrl+left"]).await;
    assert_eq!(cursor(&pilot, "#i"), 12);
    pilot.press(&["ctrl+e", "ctrl+a"]).await;
    assert_eq!(cursor(&pilot, "#i"), 0);

    pilot.press(&["delete"]).await;
    assert_eq!(value(&pilot, "#i"), "ello world foo");
    pilot.press(&["end", "backspace"]).await;
    assert_eq!(value(&pilot, "#i"), "ello world fo");
    pilot.press(&["ctrl+w"]).await;
    assert_eq!(value(&pilot, "#i"), "ello world ");
    pilot.press(&["ctrl+w"]).await;
    assert_eq!(value(&pilot, "#i"), "ello ");
    pilot.press(&["ctrl+d"]).await; // at end: nothing to delete
    assert_eq!(value(&pilot, "#i"), "ello ");
    pilot.type_text("big world").await;
    pilot.press(&["ctrl+left", "ctrl+left"]).await;
    assert_eq!(cursor(&pilot, "#i"), 5);
    pilot.press(&["ctrl+k"]).await;
    assert_eq!(value(&pilot, "#i"), "ello ");
    pilot.press(&["left", "ctrl+u"]).await;
    assert_eq!(value(&pilot, "#i"), " ");
    assert_eq!(cursor(&pilot, "#i"), 0);
}

#[tokio::test]
async fn backspace_at_start_does_not_post_changed() {
    let mut pilot = form(
        compose![Input::new().select_on_focus(false).id("i")],
        Size::new(30, 3),
    )
    .await;
    pilot.press(&["backspace", "delete"]).await;
    assert!(log(&pilot).is_empty());
}

#[tokio::test]
async fn delete_right_word() {
    let mut pilot = form(
        compose![Input::new()
            .value("one two three")
            .select_on_focus(false)
            .id("i")],
        Size::new(30, 3),
    )
    .await;
    pilot.press(&["home", "alt+backspace"]).await;
    assert_eq!(value(&pilot, "#i"), "two three");
}

#[tokio::test]
async fn password_is_masked() {
    let mut pilot = form(
        compose![Input::new().password(true).id("pw")],
        Size::new(30, 3),
    )
    .await;
    pilot.type_text("secret").await;
    assert_eq!(value(&pilot, "#pw"), "secret");
    let text = pilot.screen_text();
    assert!(text.contains("••••••"), "{text}");
    assert!(!text.contains("secret"), "{text}");
    // Word moves don't reveal word boundaries in password fields.
    pilot.type_text(" more").await;
    pilot.press(&["ctrl+left"]).await;
    assert_eq!(cursor(&pilot, "#pw"), 0);
}

#[tokio::test]
async fn max_length_and_restrict_ring_bell() {
    let mut pilot = form(
        compose![
            Input::new().max_length(3).id("short"),
            Input::new().restrict(|c| c.is_ascii_digit()).id("digits"),
            Input::new().restrict_chars("abc").id("abc"),
        ],
        Size::new(30, 9),
    )
    .await;
    pilot.type_text("abcdef").await;
    assert_eq!(value(&pilot, "#short"), "abc");
    assert_eq!(pilot.bell_count(), 3);
    pilot.press(&["tab"]).await;
    pilot.type_text("1a2b3").await;
    assert_eq!(value(&pilot, "#digits"), "123");
    pilot.press(&["tab"]).await;
    pilot.type_text("abcd").await;
    assert_eq!(value(&pilot, "#abc"), "abc");
    assert_eq!(pilot.bell_count(), 6);
}

#[tokio::test]
async fn integer_and_number_types_validate() {
    let mut pilot = form(
        compose![
            Input::new().input_type(InputType::Integer).id("int"),
            Input::new().input_type(InputType::Number).id("num"),
        ],
        Size::new(30, 6),
    )
    .await;
    pilot.type_text("-12x.5").await;
    assert_eq!(value(&pilot, "#int"), "-125");
    let int_id = pilot.query("#int")[0];
    assert!(pilot.query_one::<Input>("#int").unwrap().is_valid());
    assert!(pilot.dom().get(int_id).unwrap().has_class("-valid"));
    pilot.press(&["ctrl+u"]).await;
    pilot.type_text("-").await;
    assert!(!pilot.query_one::<Input>("#int").unwrap().is_valid());
    assert!(pilot.dom().get(int_id).unwrap().has_class("-invalid"));

    pilot.press(&["tab"]).await;
    pilot.type_text("1.5e-3").await;
    assert_eq!(value(&pilot, "#num"), "1.5e-3");
    assert!(pilot.query_one::<Input>("#num").unwrap().is_valid());
    pilot.type_text(".").await;
    assert_eq!(value(&pilot, "#num"), "1.5e-3");
}

#[tokio::test]
async fn paste_inserts_first_line() {
    let mut pilot = form(compose![Input::new().value("ab").id("i")], Size::new(30, 3)).await;
    pilot.press(&["end", "left"]).await;
    pilot.paste("XY\nignored").await;
    assert_eq!(value(&pilot, "#i"), "aXYb");
    assert_eq!(cursor(&pilot, "#i"), 3);
    // The initial value posts `Changed` on mount (as in Textual), then the paste.
    assert_eq!(
        log(&pilot),
        vec!["changed Some(\"i\") ab", "changed Some(\"i\") aXYb"]
    );
}

#[tokio::test]
async fn long_values_scroll_to_keep_cursor_visible() {
    // Content width = 20 - 2 (border) - 4 (padding) = 14.
    let mut pilot = form(compose![Input::new().id("i")], Size::new(20, 3)).await;
    pilot.type_text("abcdefghijklmnopqrstuvwxyz").await;
    let line = row(&mut pilot, 1);
    // The cursor sits after "z" in the last column, so 13 chars are visible.
    assert!(line.contains("nopqrstuvwxyz"), "{line:?}");
    assert!(!line.contains('m'), "{line:?}");
    pilot.press(&["home"]).await;
    let line = row(&mut pilot, 1);
    assert!(line.contains("abcdefghijklmn"), "{line:?}");
    assert!(!line.contains('o'), "{line:?}");
    pilot.press(&["end"]).await;
    assert!(row(&mut pilot, 1).contains("xyz"));
}

#[tokio::test]
async fn shift_selection_is_replaced_by_typing() {
    let mut pilot = form(
        compose![Input::new().value("hello").select_on_focus(false).id("i")],
        Size::new(30, 3),
    )
    .await;
    pilot.press(&["shift+left", "shift+left"]).await;
    let input = pilot.query_one::<Input>("#i").unwrap();
    assert_eq!(input.selected_text(), "lo");
    assert_eq!(input.selection(), Selection::new(5, 3));
    // Selection is drawn with the input--selection style.
    let c = pilot.canvas();
    let plain = c.get(3, 1).unwrap().bg;
    assert_ne!(c.get(7, 1).unwrap().bg, plain);
    pilot.type_text("p").await;
    assert_eq!(value(&pilot, "#i"), "help");
    pilot.press(&["shift+home"]).await;
    pilot.press(&["backspace"]).await;
    assert_eq!(value(&pilot, "#i"), "");
}

#[tokio::test]
async fn select_on_focus_selects_all() {
    let mut pilot = form(
        compose![
            Input::new().id("a"),
            Input::new().value("replace me").id("b"),
        ],
        Size::new(30, 6),
    )
    .await;
    pilot.press(&["tab"]).await;
    assert_eq!(pilot.focused_id().as_deref(), Some("b"));
    assert_eq!(
        pilot.query_one::<Input>("#b").unwrap().selected_text(),
        "replace me"
    );
    pilot.type_text("new").await;
    assert_eq!(value(&pilot, "#b"), "new");
}

#[tokio::test]
async fn click_focuses_and_places_cursor() {
    let mut pilot = form(
        compose![Input::new().id("a"), Input::new().value("abcdef").id("b"),],
        Size::new(30, 6),
    )
    .await;
    assert_eq!(pilot.focused_id().as_deref(), Some("a"));
    // Content starts at x=3; click on "c" (index 2).
    pilot.click_at(5, 4).await;
    assert_eq!(pilot.focused_id().as_deref(), Some("b"));
    let b = pilot.query_one::<Input>("#b").unwrap();
    assert_eq!(b.cursor_position(), 2);
    assert!(b.selection().is_empty(), "{:?}", b.selection());
    // Clicking past the end puts the cursor at the end.
    pilot.click_at(25, 4).await;
    assert_eq!(cursor(&pilot, "#b"), 6);
    // Leaving the input posts Blurred.
    pilot.press(&["tab"]).await;
    assert_eq!(log(&pilot).last().unwrap(), "blurred Some(\"b\") abcdef");
}

#[tokio::test]
async fn disabled_input_ignores_input() {
    let mut pilot = form(
        compose![
            Input::new().value("fixed").id("d").disabled(true),
            Input::new().id("e"),
        ],
        Size::new(30, 6),
    )
    .await;
    // Not focusable: the enabled input got auto focus.
    assert_eq!(pilot.focused_id().as_deref(), Some("e"));
    pilot.click("#d").await;
    assert_eq!(pilot.focused_id().as_deref(), Some("e"));
    pilot.press(&["tab"]).await;
    assert_eq!(pilot.focused_id().as_deref(), Some("e"));
    assert_eq!(value(&pilot, "#d"), "fixed");
    assert!(pilot.screen_text().contains("fixed"));
}

#[tokio::test(start_paused = true)]
async fn cursor_blinks_while_focused() {
    let mut pilot = form(compose![Input::new().id("i")], Size::new(30, 3)).await;
    assert!(pilot.query_one::<Input>("#i").unwrap().cursor_visible());
    let c = pilot.canvas();
    let on = c.get(3, 1).unwrap().bg;
    pilot.wait(Duration::from_millis(510)).await;
    assert!(!pilot.query_one::<Input>("#i").unwrap().cursor_visible());
    let c = pilot.canvas();
    assert_ne!(c.get(3, 1).unwrap().bg, on);
    pilot.wait(Duration::from_millis(500)).await;
    assert!(pilot.query_one::<Input>("#i").unwrap().cursor_visible());
    pilot.wait(Duration::from_millis(500)).await;
    assert!(!pilot.query_one::<Input>("#i").unwrap().cursor_visible());
    // A key press restarts the blink with the cursor visible.
    pilot.type_text("a").await;
    assert!(pilot.query_one::<Input>("#i").unwrap().cursor_visible());
}

#[tokio::test(start_paused = true)]
async fn no_blink_when_disabled_by_option() {
    let mut pilot = form(
        compose![Input::new().cursor_blink(false).id("i")],
        Size::new(30, 3),
    )
    .await;
    pilot.wait(Duration::from_millis(1600)).await;
    assert!(pilot.query_one::<Input>("#i").unwrap().cursor_visible());
}

#[tokio::test]
async fn compact_has_no_border() {
    let mut pilot = form(
        compose![Input::new().value("tiny").compact(true).id("i")],
        Size::new(30, 3),
    )
    .await;
    let line = row(&mut pilot, 0);
    assert!(line.starts_with("tiny"), "{line:?}");
    let id = pilot.query("#i")[0];
    assert_eq!(pilot.dom().get(id).unwrap().layout.region.height, 1);
}

#[tokio::test]
async fn programmatic_set_value() {
    let mut pilot = form(compose![Input::new().id("i")], Size::new(30, 3)).await;
    pilot
        .with::<Input, _>("#i", |i, ctx| i.set_value("set", ctx))
        .await;
    assert_eq!(value(&pilot, "#i"), "set");
    assert_eq!(log(&pilot), vec!["changed Some(\"i\") set"]);
    assert!(pilot.screen_text().contains("set"));
    pilot.with::<Input, _>("#i", |i, ctx| i.clear(ctx)).await;
    assert_eq!(value(&pilot, "#i"), "");
}

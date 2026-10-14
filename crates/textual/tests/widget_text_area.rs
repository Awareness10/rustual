//! Tests for the `TextArea` widget.

use std::time::{Duration, Instant};
use textual::prelude::*;
use textual::widgets::text_area::{self, Selection};

/// Root that mounts the given children and records TextArea messages.
struct Root {
    children: Vec<Child>,
    log: Vec<String>,
}

impl Widget for Root {
    fn compose(&mut self) -> Vec<Child> {
        std::mem::take(&mut self.children)
    }
    fn on_message(&mut self, m: &Message, _ctx: &mut Ctx) -> Flow {
        if let Some(c) = m.get::<text_area::Changed>() {
            self.log.push(format!("changed {:?}", c.text_area_id));
        }
        if let Some(s) = m.get::<text_area::SelectionChanged>() {
            self.log.push(format!("selection {:?}", s.selection));
        }
        Flow::Continue
    }
}

/// No border / padding, so content starts at (0, 0).
const PLAIN: &str = "border: none; padding: 0;";

async fn app(children: Vec<Child>, size: Size) -> Pilot {
    App::new(Root {
        children,
        log: vec![],
    })
    .test(size)
    .await
}

async fn plain(ta: TextArea, size: Size) -> Pilot {
    app(compose![ta.id("ta").style(PLAIN)], size).await
}

fn ta(pilot: &Pilot) -> &TextArea {
    pilot.query_one::<TextArea>("#ta").unwrap()
}

fn text(pilot: &Pilot) -> String {
    ta(pilot).text()
}

fn cursor(pilot: &Pilot) -> (usize, usize) {
    ta(pilot).cursor_location()
}

fn row(pilot: &mut Pilot, y: usize) -> String {
    pilot
        .screen_text()
        .lines()
        .nth(y)
        .unwrap_or_default()
        .to_string()
}

fn scroll(pilot: &Pilot) -> Offset {
    let id = pilot.query("#ta")[0];
    pilot.dom().get(id).unwrap().scroll_offset
}

fn log(pilot: &Pilot) -> Vec<String> {
    pilot.app::<Root>().unwrap().log.clone()
}

#[tokio::test]
async fn renders_text_with_default_css() {
    let mut pilot = app(
        compose![TextArea::new("hello\nworld").id("ta")],
        Size::new(20, 6),
    )
    .await;
    assert!(pilot.css_errors().is_empty(), "{:?}", pilot.css_errors());
    // Tall border (1 cell) + padding 0 1: text starts at (2, 1).
    let sub = |r: String| r.chars().skip(2).take(5).collect::<String>();
    assert_eq!(sub(row(&mut pilot, 1)), "hello");
    assert_eq!(sub(row(&mut pilot, 2)), "world");
    // Auto-focused: the cursor cell differs from the next one.
    assert_eq!(pilot.focused_id().as_deref(), Some("ta"));
    let c = pilot.canvas();
    assert_ne!(c.get(2, 1).unwrap().bg, c.get(3, 1).unwrap().bg);
}

#[tokio::test]
async fn typing_enter_backspace_and_messages() {
    let mut pilot = plain(TextArea::new(""), Size::new(20, 5)).await;
    pilot.type_text("ab").await;
    pilot.press(&["enter"]).await;
    pilot.type_text("cd").await;
    assert_eq!(text(&pilot), "ab\ncd");
    assert_eq!(cursor(&pilot), (1, 2));
    pilot.press(&["backspace", "backspace", "backspace"]).await;
    assert_eq!(text(&pilot), "ab");
    assert_eq!(cursor(&pilot), (0, 2));
    pilot.press(&["left", "delete"]).await;
    assert_eq!(text(&pilot), "a");
    let l = log(&pilot);
    assert!(l.contains(&"changed Some(\"ta\")".to_string()), "{l:?}");
    assert!(
        l.contains(&format!("selection {:?}", Selection::cursor((1, 2)))),
        "{l:?}"
    );
    assert_eq!(row(&mut pilot, 0).trim_end(), "a");
}

#[tokio::test]
async fn keyboard_selection_and_replace() {
    let mut pilot = plain(TextArea::new("hello world"), Size::new(20, 5)).await;
    pilot.press(&["shift+right", "shift+right"]).await;
    assert_eq!(ta(&pilot).selected_text(), "he");
    pilot.type_text("J").await;
    assert_eq!(text(&pilot), "Jllo world");
    pilot.press(&["ctrl+shift+right"]).await;
    assert_eq!(ta(&pilot).selected_text(), "llo");
    pilot.press(&["f7"]).await;
    assert_eq!(ta(&pilot).selected_text(), "Jllo world");
    // Left collapses a selection to its start.
    pilot.press(&["left"]).await;
    assert_eq!(cursor(&pilot), (0, 0));
    pilot.press(&["shift+end"]).await;
    assert_eq!(ta(&pilot).selection(), Selection::new((0, 0), (0, 10)));
    // The selection is highlighted.
    let c = pilot.canvas();
    assert_ne!(c.get(3, 0).unwrap().bg, c.get(15, 0).unwrap().bg);
}

#[tokio::test]
async fn undo_redo_batches() {
    let mut pilot = plain(TextArea::new(""), Size::new(30, 5)).await;
    pilot.type_text("hello world").await;
    pilot.press(&["ctrl+z"]).await;
    assert_eq!(text(&pilot), "");
    pilot.press(&["ctrl+y"]).await;
    assert_eq!(text(&pilot), "hello world");
    assert_eq!(cursor(&pilot), (0, 11));
    // Newlines get their own checkpoint.
    pilot.press(&["enter"]).await;
    pilot.type_text("x").await;
    pilot.press(&["ctrl+z"]).await;
    assert_eq!(text(&pilot), "hello world\n");
    pilot.press(&["ctrl+z"]).await;
    assert_eq!(text(&pilot), "hello world");
    // Cursor movement starts a new batch.
    pilot.press(&["left"]).await;
    pilot.type_text("!").await;
    pilot.press(&["ctrl+z"]).await;
    assert_eq!(text(&pilot), "hello world");
    // Typing after an undo clears the redo stack.
    assert!(ta(&pilot).history().can_redo());
    pilot.type_text("?").await;
    assert!(!ta(&pilot).history().can_redo());
}

#[tokio::test(start_paused = true)]
async fn checkpoint_timer_splits_batches() {
    let mut pilot = plain(TextArea::new(""), Size::new(30, 5)).await;
    pilot.type_text("ab").await;
    pilot.wait(Duration::from_secs(3)).await;
    pilot.type_text("cd").await;
    pilot.press(&["ctrl+z"]).await;
    assert_eq!(text(&pilot), "ab");
}

#[tokio::test]
async fn max_checkpoints_limits_undo() {
    let mut pilot = plain(TextArea::new("").max_checkpoints(2), Size::new(30, 5)).await;
    for _ in 0..4 {
        pilot.press(&["enter"]).await;
    }
    pilot.press(&["ctrl+z", "ctrl+z", "ctrl+z", "ctrl+z"]).await;
    assert_eq!(text(&pilot), "\n\n");
}

#[tokio::test]
async fn soft_wrap_rendering_and_navigation() {
    // Width 11: wrap width = 11 - 1 (cursor cell) = 10 ("aaaa bbbb " incl. trailing space).
    let mut pilot = plain(TextArea::new("aaaa bbbb cccc\nz"), Size::new(11, 5)).await;
    assert_eq!(row(&mut pilot, 0).trim_end(), "aaaa bbbb");
    assert_eq!(row(&mut pilot, 1).trim_end(), "cccc");
    assert_eq!(row(&mut pilot, 2).trim_end(), "z");
    pilot.press(&["right", "right", "down"]).await;
    assert_eq!(cursor(&pilot), (0, 12));
    pilot.press(&["down"]).await;
    assert_eq!(cursor(&pilot), (1, 1));
    pilot.press(&["up", "up"]).await;
    assert_eq!(cursor(&pilot), (0, 2));
    // End goes to the end of the wrapped section, not the line.
    pilot.press(&["end"]).await;
    assert_eq!(cursor(&pilot), (0, 9));
    // No horizontal scrolling in soft-wrap mode.
    assert_eq!(scroll(&pilot).x, 0);
}

#[tokio::test]
async fn line_numbers_gutter() {
    let mut pilot = plain(
        TextArea::new("one\ntwo\nthree").show_line_numbers(true),
        Size::new(20, 5),
    )
    .await;
    assert_eq!(row(&mut pilot, 0).trim_end(), "1  one");
    assert_eq!(row(&mut pilot, 2).trim_end(), "3  three");
    // The cursor line's gutter uses `text-area--cursor-gutter`, others `--gutter`.
    let c = pilot.canvas();
    assert_ne!(c.get(0, 0).unwrap().bg, c.get(0, 1).unwrap().bg);
    // Clicking in the text area accounts for the gutter.
    pilot.click_at(5, 1).await;
    assert_eq!(cursor(&pilot), (1, 2));
}

#[tokio::test]
async fn scrolling_keeps_cursor_visible() {
    let lines: Vec<String> = (0..100).map(|i| format!("line {i}")).collect();
    let mut pilot = plain(TextArea::new(lines.join("\n")), Size::new(30, 5)).await;
    pilot.press(&["ctrl+end"]).await;
    assert_eq!(cursor(&pilot), (99, 7));
    assert_eq!(scroll(&pilot).y, 95);
    assert!(row(&mut pilot, 4).starts_with("line 99"));
    pilot.press(&["pageup"]).await;
    assert_eq!(cursor(&pilot).0, 94);
    assert!(scroll(&pilot).y <= 94 && scroll(&pilot).y + 5 > 94);
    pilot.press(&["ctrl+home"]).await;
    assert_eq!(scroll(&pilot).y, 0);
    pilot.press(&["pagedown"]).await;
    assert_eq!(cursor(&pilot).0, 5);
    // Mouse wheel scrolls without moving the cursor.
    pilot.scroll_down_at(3, 2).await;
    assert!(scroll(&pilot).y > 5);
    assert_eq!(cursor(&pilot).0, 5);
}

#[tokio::test]
async fn horizontal_scroll_without_soft_wrap() {
    let long = "x".repeat(50) + "END";
    let mut pilot = plain(
        TextArea::new(format!("{long}\nshort"))
            .soft_wrap(false)
            .show_line_numbers(true),
        Size::new(20, 5),
    )
    .await;
    pilot.press(&["end"]).await;
    assert_eq!(cursor(&pilot), (0, 53));
    assert!(scroll(&pilot).x > 0);
    let r = row(&mut pilot, 0);
    // The gutter stays fixed while the text scrolls.
    assert!(r.starts_with("1  "), "{r:?}");
    assert!(r.contains("END"), "{r:?}");
    pilot.press(&["home"]).await;
    assert_eq!(scroll(&pilot).x, 0);
}

#[tokio::test]
async fn mouse_click_drag_and_double_click() {
    let mut pilot = plain(TextArea::new("alpha beta\ngamma delta"), Size::new(20, 5)).await;
    pilot.click_at(3, 1).await;
    assert_eq!(cursor(&pilot), (1, 3));
    // Click past the end of a line goes to its end.
    pilot.click_at(15, 0).await;
    assert_eq!(cursor(&pilot), (0, 10));
    // Drag from (1,0) to (8,1).
    pilot.mouse_down(1, 0).await;
    pilot.mouse_drag_to(4, 0).await;
    pilot.mouse_drag_to(8, 1).await;
    pilot.mouse_up(8, 1).await;
    assert_eq!(ta(&pilot).selection(), Selection::new((0, 1), (1, 8)));
    assert_eq!(ta(&pilot).selected_text(), "lpha beta\ngamma de");
    // Moving without the button doesn't extend the selection.
    pilot.hover("#ta").await;
    assert_eq!(ta(&pilot).selection(), Selection::new((0, 1), (1, 8)));
    // Double click selects a word, triple click the line.
    pilot.click_at(7, 0).await;
    pilot.click_at(7, 0).await;
    assert_eq!(ta(&pilot).selected_text(), "beta");
    pilot.click_at(7, 0).await;
    assert_eq!(ta(&pilot).selected_text(), "alpha beta");
}

#[tokio::test]
async fn read_only_mode() {
    let mut pilot = plain(TextArea::new("fixed").read_only(true), Size::new(20, 5)).await;
    let id = pilot.query("#ta")[0];
    assert!(pilot.dom().get(id).unwrap().has_class("-read-only"));
    pilot.type_text("xy").await;
    pilot.press(&["backspace", "enter", "ctrl+k"]).await;
    pilot.paste("pasted").await;
    assert_eq!(text(&pilot), "fixed");
    pilot.press(&["right", "shift+right"]).await;
    assert_eq!(ta(&pilot).selected_text(), "i");
    // But the API can still edit.
    pilot
        .with::<TextArea, _>("#ta", |t, ctx| {
            t.insert("!", Some((0, 5)), ctx);
        })
        .await;
    assert_eq!(text(&pilot), "fixed!");
}

#[tokio::test]
async fn paste_and_clipboard() {
    let mut pilot = plain(TextArea::new("one"), Size::new(20, 5)).await;
    pilot.press(&["end"]).await;
    pilot.paste("\ntwo\nthree").await;
    assert_eq!(text(&pilot), "one\ntwo\nthree");
    assert_eq!(cursor(&pilot), (2, 5));
    // Paste is a single undo step.
    pilot.press(&["ctrl+z"]).await;
    assert_eq!(text(&pilot), "one");
    // Cut with no selection cuts the line; paste it back.
    pilot.press(&["ctrl+y", "ctrl+home", "ctrl+x"]).await;
    assert_eq!(text(&pilot), "two\nthree");
    pilot.press(&["ctrl+end", "enter", "ctrl+v"]).await;
    assert_eq!(text(&pilot), "two\nthree\none\n");
    // Copy + paste a selection.
    pilot
        .press(&["ctrl+home", "shift+end", "ctrl+c", "ctrl+end", "ctrl+v"])
        .await;
    assert_eq!(text(&pilot), "two\nthree\none\ntwo");
}

#[tokio::test]
async fn deletion_bindings() {
    let mut pilot = plain(
        TextArea::new("foo bar baz\nsecond line\nthird"),
        Size::new(30, 5),
    )
    .await;
    pilot.press(&["end", "ctrl+w"]).await;
    assert_eq!(text(&pilot), "foo bar \nsecond line\nthird");
    pilot.press(&["ctrl+u"]).await;
    assert_eq!(text(&pilot), "\nsecond line\nthird");
    // ctrl+k on an empty line deletes the line.
    pilot.press(&["ctrl+k"]).await;
    assert_eq!(text(&pilot), "second line\nthird");
    pilot.press(&["right", "right", "ctrl+k"]).await;
    assert_eq!(text(&pilot), "se\nthird");
    // ctrl+k at the end of a line joins the next.
    pilot.press(&["ctrl+k"]).await;
    assert_eq!(text(&pilot), "sethird");
    pilot.press(&["home", "alt+delete"]).await;
    assert_eq!(text(&pilot), "");
    pilot.paste("a\nb\nc").await;
    pilot.press(&["up", "ctrl+shift+k"]).await;
    assert_eq!(text(&pilot), "a\nc");
    assert_eq!(cursor(&pilot).0, 1);
    pilot.press(&["ctrl+shift+k"]).await;
    assert_eq!(text(&pilot), "a");
}

#[tokio::test]
async fn tab_behaviors() {
    let mut pilot = app(
        compose![
            TextArea::code_editor("", Some("python"))
                .id("ta")
                .style(PLAIN),
            Input::new().id("next"),
        ],
        Size::new(30, 10),
    )
    .await;
    pilot.type_text("ab").await;
    pilot.press(&["tab"]).await;
    assert_eq!(text(&pilot), "ab  ");
    pilot.press(&["escape"]).await;
    assert_eq!(pilot.focused_id().as_deref(), Some("next"));

    let mut pilot = app(
        compose![
            TextArea::new("").id("ta").style(PLAIN),
            Input::new().id("next"),
        ],
        Size::new(30, 10),
    )
    .await;
    pilot.press(&["tab"]).await;
    assert_eq!(text(&pilot), "");
    assert_eq!(pilot.focused_id().as_deref(), Some("next"));
}

#[tokio::test]
async fn unicode_widths() {
    let mut pilot = plain(TextArea::new("日本語x\n🎉ab\n👨‍👩‍👧z"), Size::new(20, 5)).await;
    assert_eq!(row(&mut pilot, 0).trim_end(), "日本語x");
    pilot.press(&["right", "right", "down"]).await;
    // Column 2 of "日本語" is at x=4; on "🎉ab" x=4 is column 3 ("b").
    assert_eq!(cursor(&pilot), (1, 3));
    pilot.press(&["down"]).await;
    // The ZWJ family is one 2-cell grapheme of 5 chars; x=4 is past "👨‍👩‍👧z" (3 cells).
    assert_eq!(cursor(&pilot), (2, 6));
    pilot.press(&["left", "left"]).await;
    assert_eq!(cursor(&pilot), (2, 0));
    pilot.press(&["delete"]).await;
    assert_eq!(text(&pilot), "日本語x\n🎉ab\nz");
    // Clicking the right half of a wide character lands on it.
    pilot.click_at(3, 0).await;
    assert_eq!(cursor(&pilot), (0, 1));
    // Typing CJK.
    pilot.type_text("中").await;
    assert_eq!(row(&mut pilot, 0).trim_end(), "日中本語x");
    // The cursor cell is drawn over "本" at x=4.
    let c = pilot.canvas();
    assert_ne!(c.get(4, 0).unwrap().bg, c.get(8, 0).unwrap().bg);
}

#[tokio::test]
async fn placeholder_and_tabs() {
    let mut pilot = plain(TextArea::new("").placeholder("Type here"), Size::new(20, 5)).await;
    assert_eq!(row(&mut pilot, 0).trim_end(), "Type here");
    // Tab characters in the text are expanded to the next tab stop.
    pilot
        .with::<TextArea, _>("#ta", |t, ctx| {
            t.set_text("a\tb", ctx);
        })
        .await;
    assert_eq!(row(&mut pilot, 0).trim_end(), "a   b");
    pilot.press(&["end"]).await;
    assert_eq!(cursor(&pilot), (0, 3));
}

#[tokio::test]
async fn matching_bracket_and_api() {
    let mut pilot = plain(TextArea::new("f(x)"), Size::new(20, 5)).await;
    pilot.press(&["right"]).await;
    assert_eq!(ta(&pilot).matching_bracket_location(), Some((0, 3)));
    let c = pilot.canvas();
    let plain_bg = c.get(2, 0).unwrap().bg;
    assert_ne!(c.get(3, 0).unwrap().bg, plain_bg);
    pilot
        .with::<TextArea, _>("#ta", |t, ctx| {
            t.replace("y", (0, 2), (0, 3), ctx);
            t.insert("g", Some((0, 0)), ctx);
            t.move_cursor((0, 4), false, ctx);
            t.set_selection(Selection::new((0, 0), (0, 1)), ctx);
        })
        .await;
    assert_eq!(text(&pilot), "gf(y)");
    assert_eq!(ta(&pilot).selected_text(), "g");
    pilot
        .with::<TextArea, _>("#ta", |t, ctx| {
            t.delete((0, 0), (0, 2), ctx);
        })
        .await;
    assert_eq!(text(&pilot), "(y)");
    pilot
        .with::<TextArea, _>("#ta", |t, ctx| {
            t.undo(ctx);
            t.clear(ctx);
        })
        .await;
    assert_eq!(text(&pilot), "");
}

#[tokio::test]
async fn themes() {
    let mut pilot = plain(
        TextArea::code_editor("def f(): pass", Some("python")),
        Size::new(30, 5),
    )
    .await;
    let c = pilot.canvas();
    // Monokai background, keyword colored.
    assert_eq!(c.get(10, 2).unwrap().bg, Color::parse("#272822").unwrap());
    // Gutter "1  " then "def" from x=3 (cursor on "d").
    assert_eq!(c.get(4, 0).unwrap().fg, Color::parse("#F92672").unwrap());
    let ok = pilot
        .with::<TextArea, _>("#ta", |t, ctx| t.set_theme("css", ctx))
        .await;
    assert_eq!(ok, Some(true));
    let c = pilot.canvas();
    assert_ne!(c.get(10, 2).unwrap().bg, Color::parse("#272822").unwrap());
}

#[tokio::test]
async fn large_document_stays_responsive() {
    let text: String = (0..10_000)
        .map(|i| format!("line {i} lorem ipsum dolor sit amet, consectetur adipiscing elit\n"))
        .collect();
    let start = Instant::now();
    let mut pilot = plain(TextArea::new(&text), Size::new(80, 24)).await;
    for wrap in [true, false] {
        pilot
            .with::<TextArea, _>("#ta", |t, ctx| t.set_soft_wrap(wrap, ctx))
            .await;
        pilot.press(&["ctrl+end"]).await;
        assert!(cursor(&pilot).0 >= 10_000);
        pilot.type_text("hello").await;
        pilot
            .press(&["enter", "up", "up", "pageup", "pageup"])
            .await;
        pilot.press(&["ctrl+home"]).await;
        pilot.type_text("abc").await;
        pilot.press(&["ctrl+z"]).await;
        assert!(pilot.screen_text().contains("line 0 lorem"));
    }
    let elapsed = start.elapsed();
    // ~40 frames over a 10k-line document; generous bound for debug builds.
    assert!(elapsed < Duration::from_secs(10), "took {elapsed:?}");
}

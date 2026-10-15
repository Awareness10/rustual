//! OSC 52 clipboard (Textual: `App.copy_to_clipboard`), from the app and from Input / TextArea.

use rustual::prelude::*;

struct Root;

impl Widget for Root {
    fn compose(&mut self) -> Vec<Child> {
        compose![
            Input::new().value("hello world").id("inp"),
            TextArea::new("line one\nline two").id("ta"),
        ]
    }
}

#[tokio::test]
async fn copy_to_clipboard_queues_osc52() {
    let mut p = App::new(Root).test(Size::new(40, 12)).await;
    assert_eq!(p.clipboard(), "");
    let got = p
        .with::<Root, _>("App", |_, ctx| {
            ctx.copy_to_clipboard("hi");
            ctx.clipboard().to_string()
        })
        .await
        .unwrap();
    assert_eq!(got, "hi");
    assert_eq!(p.clipboard(), "hi");
    assert_eq!(
        p.pending_terminal_writes().last().map(String::as_str),
        Some("\x1b]52;c;aGk=\x07")
    );
    // UTF-8 is base64-encoded byte-wise.
    p.with::<Root, _>("App", |_, ctx| ctx.copy_to_clipboard("é"))
        .await;
    assert_eq!(
        p.pending_terminal_writes().last().map(String::as_str),
        Some("\x1b]52;c;w6k=\x07")
    );
}

#[tokio::test]
async fn input_copy_and_cut_use_the_system_clipboard() {
    let mut p = App::new(Root).test(Size::new(40, 12)).await;
    assert_eq!(p.focused_id().as_deref(), Some("inp"));
    p.with::<Input, _>("#inp", |w, _| w.select_all()).await;
    p.press(&["ctrl+c"]).await;
    assert_eq!(p.clipboard(), "hello world");
    assert!(!p.has_exited(), "copy consumed ctrl+c");
    p.press(&["end", "shift+home"]).await;
    p.press(&["ctrl+x"]).await;
    assert_eq!(p.clipboard(), "hello world");
    assert_eq!(p.query_one::<Input>("#inp").unwrap().get_value(), "");
    // Paste still works from the (shared) local clipboard.
    p.press(&["ctrl+v"]).await;
    assert_eq!(
        p.query_one::<Input>("#inp").unwrap().get_value(),
        "hello world"
    );
}

#[tokio::test]
async fn text_area_copy_and_cut_use_the_system_clipboard() {
    let mut p = App::new(Root).test(Size::new(40, 12)).await;
    p.press(&["tab"]).await;
    assert_eq!(p.focused_id().as_deref(), Some("ta"));
    p.press(&["f7", "ctrl+c"]).await;
    assert_eq!(p.clipboard(), "line one\nline two");
    p.press(&["ctrl+x"]).await;
    assert_eq!(p.clipboard(), "line one\nline two");
    assert_eq!(p.query_one::<TextArea>("#ta").unwrap().text(), "");
    assert_eq!(p.pending_terminal_writes().len(), 2);
}

//! Log and RichLog tests.

use textual::prelude::*;

struct LogApp;

impl Widget for LogApp {
    fn compose(&mut self) -> Vec<Child> {
        compose![
            Log::new().id("log").style("height: 8;"),
            RichLog::new()
                .markup(true)
                .wrap(true)
                .min_width(1)
                .max_lines(Some(50))
                .id("rich")
                .style("height: 8;"),
        ]
    }
}

async fn pilot() -> Pilot {
    App::new(LogApp).test(Size::new(40, 16)).await
}

#[tokio::test]
async fn log_writes_and_auto_scrolls() {
    let mut p = pilot().await;
    assert!(p.css_errors().is_empty(), "{:?}", p.css_errors());
    p.with::<Log, _>("#log", |log, ctx| {
        log.write_line("first", ctx);
        log.write_lines(["a\tb", "second"], ctx);
    })
    .await;
    let text = p.screen_text();
    assert!(
        text.contains("first") && text.contains("a       b"),
        "{text}"
    );
    for i in 0..30 {
        p.with::<Log, _>("#log", |log, ctx| log.write_line(&format!("line {i}"), ctx))
            .await;
    }
    let text = p.screen_text();
    assert!(text.contains("line 29"), "auto scrolled: {text}");
    assert!(!text.contains("first"), "{text}");
    assert_eq!(p.query_one::<Log>("#log").unwrap().line_count(), 33);
    // Scrolled up: new lines don't scroll.
    p.scroll_up_at(5, 3).await;
    p.with::<Log, _>("#log", |log, ctx| log.write_line("line 30", ctx))
        .await;
    assert!(!p.screen_text().contains("line 30"));
    // Wheel back to the end.
    for _ in 0..5 {
        p.scroll_down_at(5, 3).await;
    }
    assert!(p.screen_text().contains("line 30"));
}

#[tokio::test]
async fn log_partial_writes_max_lines_clear() {
    let mut p = pilot().await;
    p.with::<Log, _>("#log", |log, ctx| {
        log.set_max_lines(Some(3));
        log.write("hel", ctx);
        log.write("lo\nworld\n", ctx);
        log.write_lines(["x", "y"], ctx);
    })
    .await;
    let log = p.query_one::<Log>("#log").unwrap();
    let lines: Vec<&str> = log.lines().collect();
    assert_eq!(lines, vec!["", "x", "y"]);
    p.with::<Log, _>("#log", |log, ctx| {
        log.clear(ctx);
        log.write("hello\nworld", ctx);
    })
    .await;
    let log = p.query_one::<Log>("#log").unwrap();
    assert_eq!(log.lines().collect::<Vec<_>>(), vec!["hello", "world"]);
    assert_eq!(log.line_count(), 2);
}

#[tokio::test(start_paused = true)]
async fn log_horizontal_scroll() {
    let mut p = pilot().await;
    p.with::<Log, _>("#log", |log, ctx| {
        log.write_line(&format!("{}END", "x".repeat(60)), ctx)
    })
    .await;
    assert!(!p.screen_text().contains("END"));
    p.click_at(5, 0).await;
    p.press(&["right"; 30]).await;
    // Keyboard scrolling is animated (Textual's `scroll_right(animate=True)`).
    p.wait(std::time::Duration::from_millis(500)).await;
    assert!(p.screen_text().contains("END"), "{}", p.screen_text());
}

#[tokio::test]
async fn rich_log_markup_wrap_and_max_lines() {
    let mut p = pilot().await;
    p.with::<RichLog, _>("#rich", |log, ctx| {
        log.write("[b]bold[/] text", ctx);
        log.write(&"word ".repeat(15), ctx);
    })
    .await;
    let text = p.screen_text();
    assert!(text.contains("bold text"), "{text}");
    // 75 chars wrap within ~38 columns: 2+ lines.
    let word_lines = text.lines().filter(|l| l.contains("word")).count();
    assert!(word_lines >= 2, "{text}");
    let c = p.canvas();
    let y = text.lines().position(|l| l.contains("bold text")).unwrap() as i32;
    assert!(c
        .get(0, y)
        .unwrap()
        .attrs
        .contains(textual::render::Attrs::BOLD));
    assert!(!c
        .get(6, y)
        .unwrap()
        .attrs
        .contains(textual::render::Attrs::BOLD));
    for i in 0..60 {
        p.with::<RichLog, _>("#rich", |log, ctx| log.write(&format!("entry {i}"), ctx))
            .await;
    }
    let log = p.query_one::<RichLog>("#rich").unwrap();
    assert_eq!(log.line_count(), 50);
    let text = p.screen_text();
    assert!(text.contains("entry 59"), "auto scroll: {text}");
    p.with::<RichLog, _>("#rich", |log, ctx| log.clear(ctx))
        .await;
    assert!(!p.screen_text().contains("entry"));
}

#[tokio::test]
async fn rich_log_no_wrap_and_text() {
    struct App2;
    impl Widget for App2 {
        fn compose(&mut self) -> Vec<Child> {
            compose![RichLog::new().id("r")]
        }
    }
    let mut p = App::new(App2).test(Size::new(20, 5)).await;
    p.with::<RichLog, _>("#r", |log, ctx| {
        log.write("[b]not markup[/] and a long line here", ctx);
        log.write_text(
            Text::styled("red", Style::new().fg(Color::rgb(255, 0, 0))),
            ctx,
        );
    })
    .await;
    let text = p.screen_text();
    assert!(text.contains("[b]not markup"), "{text}");
    assert!(!text.contains("long line here"), "no wrap: {text}");
    let c = p.canvas();
    assert_eq!(c.get(0, 1).unwrap().fg, Color::rgb(255, 0, 0));
}

#[tokio::test]
async fn rich_log_highlight() {
    use textual::widgets::pretty::{repr_style, ReprToken};
    struct App3;
    impl Widget for App3 {
        fn compose(&mut self) -> Vec<Child> {
            compose![
                RichLog::new().highlight(true).id("on").style("height: 4;"),
                RichLog::new().id("off").style("height: 4;"),
            ]
        }
    }
    let mut p = App::new(App3).test(Size::new(40, 10)).await;
    for sel in ["#on", "#off"] {
        p.with::<RichLog, _>(sel, |log, ctx| log.write("n=42 ok=True", ctx))
            .await;
    }
    let on = p.query("#on")[0];
    let off = p.query("#off")[0];
    let (ro, rf) = (
        p.dom().get(on).unwrap().layout.content_region,
        p.dom().get(off).unwrap().layout.content_region,
    );
    let c = p.canvas();
    let number = repr_style(ReprToken::Number, true).fg.unwrap();
    // "n=42": the number is at x = 2.
    assert_eq!(c.get(ro.x + 2, ro.y).unwrap().fg, number);
    assert!(c
        .get(ro.x + 2, ro.y)
        .unwrap()
        .attrs
        .contains(textual::render::Attrs::BOLD));
    assert_ne!(c.get(rf.x + 2, rf.y).unwrap().fg, number);
    assert!(p.query_one::<RichLog>("#on").unwrap().is_highlighting());
}

#[tokio::test]
async fn large_log_renders_fast() {
    let mut p = pilot().await;
    let lines: Vec<String> = (0..10_000).map(|i| format!("log line {i}")).collect();
    p.with::<Log, _>("#log", |log, ctx| {
        log.write_lines(lines.iter().map(String::as_str), ctx)
    })
    .await;
    let t = std::time::Instant::now();
    let text = p.screen_text();
    assert!(text.contains("log line 9999"), "{text}");
    assert!(t.elapsed().as_millis() < 100);
}

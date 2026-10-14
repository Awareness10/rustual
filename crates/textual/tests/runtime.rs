//! End-to-end runtime tests driven by the headless Pilot.

use std::time::Duration;
use textual::prelude::*;

// ---------------------------------------------------------------- helpers

/// A focusable widget that counts key presses and posts `Pressed` on enter / click.
#[derive(Default)]
struct Knob {
    label: String,
    presses: usize,
}

#[derive(Debug, Clone, PartialEq)]
struct Pressed(String);

impl Knob {
    fn new(label: &str) -> Self {
        Knob {
            label: label.into(),
            presses: 0,
        }
    }
}

impl Widget for Knob {
    fn type_names(&self) -> &'static [&'static str] {
        &["Knob"]
    }
    fn default_css(&self) -> &'static str {
        "Knob { height: 1; width: auto; } Knob:focus { text-style: bold; }"
    }
    fn can_focus(&self) -> bool {
        true
    }
    fn render(&self, _ctx: &RenderCtx) -> Content {
        Content::plain(&format!("[{}:{}]", self.label, self.presses))
    }
    fn on_event(&mut self, event: &Event, ctx: &mut Ctx) -> Flow {
        match event {
            Event::Key(k) if k.key == "enter" => {
                self.presses += 1;
                ctx.post_message(Pressed(self.label.clone()));
                Flow::Stop
            }
            Event::Click(_) => {
                self.presses += 1;
                ctx.post_message(Pressed(self.label.clone()));
                Flow::Stop
            }
            _ => Flow::Continue,
        }
    }
}

#[derive(Default)]
struct KnobApp {
    log: Vec<String>,
}

impl Widget for KnobApp {
    fn compose(&mut self) -> Vec<Child> {
        compose![
            Label::new("status").id("status"),
            Knob::new("a").id("a"),
            Knob::new("b").id("b"),
            Knob::new("c").id("c"),
        ]
    }
    fn bindings(&self) -> Vec<Binding> {
        vec![
            Binding::new("x", "mark", "Mark"),
            Binding::new("q", "quit", "Quit"),
        ]
    }
    fn run_action(&mut self, action: &str, _args: &[String], ctx: &mut Ctx) -> bool {
        if action == "mark" {
            self.log.push("mark".into());
            if let Some(l) = ctx.query_one::<Label>("#status") {
                l.update("marked");
            }
            return true;
        }
        false
    }
    fn on_message(&mut self, msg: &Message, ctx: &mut Ctx) -> Flow {
        if let Some(Pressed(label)) = msg.get::<Pressed>() {
            self.log
                .push(format!("pressed {label} from {:?}", msg.sender_id()));
            if let Some(l) = ctx.query_one::<Label>("#status") {
                l.update(format!("pressed {label}"));
            }
        }
        Flow::Continue
    }
}

async fn knob_app() -> Pilot {
    App::new(KnobApp::default()).test(Size::new(40, 10)).await
}

// ---------------------------------------------------------------- tests

#[tokio::test]
async fn renders_composed_widgets() {
    let mut pilot = knob_app().await;
    let text = pilot.screen_text();
    assert!(text.contains("status"), "{text}");
    assert!(text.contains("[a:0]"), "{text}");
    assert!(text.contains("[c:0]"), "{text}");
    assert!(pilot.css_errors().is_empty(), "{:?}", pilot.css_errors());
}

#[tokio::test]
async fn first_focusable_is_auto_focused_and_tab_cycles() {
    let mut pilot = knob_app().await;
    assert_eq!(pilot.focused_id().as_deref(), Some("a"));
    pilot.press(&["tab"]).await;
    assert_eq!(pilot.focused_id().as_deref(), Some("b"));
    pilot.press(&["tab", "tab"]).await;
    assert_eq!(pilot.focused_id().as_deref(), Some("a"));
    pilot.press(&["shift+tab"]).await;
    assert_eq!(pilot.focused_id().as_deref(), Some("c"));
}

#[tokio::test]
async fn key_events_and_messages_bubble_to_app() {
    let mut pilot = knob_app().await;
    pilot.press(&["tab", "enter"]).await;
    assert_eq!(pilot.query_one::<Knob>("#b").unwrap().presses, 1);
    let app = pilot.app::<KnobApp>().unwrap();
    assert_eq!(app.log, vec!["pressed b from Some(\"b\")".to_string()]);
    assert!(pilot.screen_text().contains("pressed b"));
}

#[tokio::test]
async fn app_bindings_run_actions() {
    let mut pilot = knob_app().await;
    pilot.press(&["x"]).await;
    assert_eq!(pilot.app::<KnobApp>().unwrap().log, vec!["mark"]);
    assert!(pilot.screen_text().contains("marked"));
    assert!(!pilot.has_exited());
    pilot.press(&["q"]).await;
    assert!(pilot.has_exited());
}

#[tokio::test]
async fn ctrl_q_quits() {
    let mut pilot = knob_app().await;
    pilot.press(&["ctrl+q"]).await;
    assert!(pilot.has_exited());
}

#[tokio::test]
async fn click_focuses_and_clicks() {
    let mut pilot = knob_app().await;
    assert!(pilot.click("#c").await);
    assert_eq!(pilot.focused_id().as_deref(), Some("c"));
    assert_eq!(pilot.query_one::<Knob>("#c").unwrap().presses, 1);
    assert!(pilot.screen_text().contains("pressed c"));
}

// ---------------------------------------------------------------- timers & workers

#[derive(Default)]
struct Ticker {
    ticks: u32,
    done: Option<String>,
}

#[derive(Debug)]
struct WorkDone(String);

impl Widget for Ticker {
    fn type_names(&self) -> &'static [&'static str] {
        &["Ticker"]
    }
    fn on_mount(&mut self, ctx: &mut Ctx) {
        ctx.set_interval(Duration::from_millis(100), |w: &mut Ticker, ctx| {
            w.ticks += 1;
            if w.ticks == 3 {
                ctx.run_worker(async { WorkDone("finished".into()) });
            }
        });
    }
    fn on_message(&mut self, msg: &Message, _ctx: &mut Ctx) -> Flow {
        if let Some(WorkDone(s)) = msg.get::<WorkDone>() {
            self.done = Some(s.clone());
        }
        Flow::Continue
    }
    fn render(&self, _ctx: &RenderCtx) -> Content {
        Content::plain(&format!("ticks={}", self.ticks))
    }
}

#[tokio::test(start_paused = true)]
async fn intervals_and_workers() {
    struct TickApp;
    impl Widget for TickApp {
        fn compose(&mut self) -> Vec<Child> {
            compose![Ticker::default().id("t")]
        }
    }
    let mut pilot = App::new(TickApp).test(Size::new(20, 3)).await;
    assert_eq!(pilot.query_one::<Ticker>("#t").unwrap().ticks, 0);
    pilot.wait(Duration::from_millis(250)).await;
    assert_eq!(pilot.query_one::<Ticker>("#t").unwrap().ticks, 2);
    pilot.wait(Duration::from_millis(100)).await;
    assert_eq!(pilot.query_one::<Ticker>("#t").unwrap().ticks, 3);
    pilot.wait(Duration::from_millis(10)).await;
    assert_eq!(
        pilot.query_one::<Ticker>("#t").unwrap().done.as_deref(),
        Some("finished")
    );
    assert!(pilot.screen_text().contains("ticks=3"));
}

// ---------------------------------------------------------------- screens

struct Dialog;

#[derive(Debug, PartialEq)]
struct Answer(bool);

impl Widget for Dialog {
    fn type_names(&self) -> &'static [&'static str] {
        &["Dialog"]
    }
    fn compose(&mut self) -> Vec<Child> {
        compose![Label::new("Are you sure?"), Knob::new("yes").id("yes")]
    }
    fn on_message(&mut self, msg: &Message, ctx: &mut Ctx) -> Flow {
        if msg.is::<Pressed>() {
            ctx.dismiss(Answer(true));
            return Flow::Stop;
        }
        Flow::Continue
    }
}

#[derive(Default)]
struct ScreenApp {
    answer: Option<bool>,
}

impl Widget for ScreenApp {
    fn compose(&mut self) -> Vec<Child> {
        compose![Label::new("main screen"), Knob::new("open").id("open")]
    }
    fn on_message(&mut self, msg: &Message, ctx: &mut Ctx) -> Flow {
        if msg.is::<Pressed>() {
            ctx.push_modal(Dialog);
        }
        if let Some(Answer(a)) = msg.get::<Answer>() {
            self.answer = Some(*a);
        }
        Flow::Continue
    }
}

#[tokio::test]
async fn modal_screen_push_and_dismiss() {
    let mut pilot = App::new(ScreenApp::default()).test(Size::new(40, 10)).await;
    pilot.press(&["enter"]).await;
    assert_eq!(pilot.screen_count(), 2);
    assert_eq!(pilot.focused_id().as_deref(), Some("yes"));
    let text = pilot.screen_text();
    assert!(text.contains("Are you sure?"), "{text}");
    pilot.press(&["enter"]).await;
    assert_eq!(pilot.screen_count(), 1);
    assert_eq!(pilot.app::<ScreenApp>().unwrap().answer, Some(true));
    assert_eq!(pilot.focused_id().as_deref(), Some("open"));
}

// ---------------------------------------------------------------- mounting & removal

#[derive(Default)]
struct ListApp;

impl Widget for ListApp {
    fn compose(&mut self) -> Vec<Child> {
        compose![VerticalScroll::new().id("list")]
    }
    fn bindings(&self) -> Vec<Binding> {
        vec![
            Binding::new("a", "add", "Add"),
            Binding::new("d", "del", "Delete"),
        ]
    }
    fn run_action(&mut self, action: &str, _args: &[String], ctx: &mut Ctx) -> bool {
        let list = ctx.query_id("#list").unwrap();
        match action {
            "add" => {
                let n = ctx.dom().children(list).len();
                ctx.mount_to(list, Label::new(format!("item {n}")).classes("item"), None);
                true
            }
            "del" => {
                if let Some(first) = ctx.query(".item").first().copied() {
                    ctx.remove(first);
                }
                true
            }
            _ => false,
        }
    }
}

#[tokio::test]
async fn mount_and_remove() {
    let mut pilot = App::new(ListApp).test(Size::new(30, 5)).await;
    pilot.press(&["a", "a", "a"]).await;
    assert_eq!(pilot.query(".item").len(), 3);
    assert!(pilot.screen_text().contains("item 2"));
    pilot.press(&["d"]).await;
    assert_eq!(pilot.query(".item").len(), 2);
    let text = pilot.screen_text();
    assert!(!text.contains("item 0"), "{text}");
}

// ---------------------------------------------------------------- css

#[tokio::test]
async fn app_css_and_theme_variables() {
    struct Styled;
    impl Widget for Styled {
        fn compose(&mut self) -> Vec<Child> {
            compose![Label::new("hi").id("hi")]
        }
    }
    let mut pilot = App::new(Styled)
        .css("#hi { background: red; color: $text; padding: 0 1; }")
        .test(Size::new(10, 2))
        .await;
    let canvas = pilot.canvas();
    assert!(pilot.css_errors().is_empty(), "{:?}", pilot.css_errors());
    assert_eq!(canvas.get(1, 0).unwrap().symbol, "h");
    assert_eq!(canvas.get(0, 0).unwrap().bg, Color::parse("red").unwrap());
}

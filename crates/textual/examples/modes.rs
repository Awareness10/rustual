//! Modes, named screens, maximize, the loading overlay and a worker with state messages.
//! Run with `cargo run -p textual --example modes`.
//!
//! Keys: f4 / f5 switch modes (dashboard / log), f1 pushes the named help screen,
//! f2 reloads (loading overlay + worker), f3 maximizes the focused widget (escape restores),
//! `ctrl+s` saves a screenshot, `ctrl+z` suspends (unix), ctrl+q quits.

use std::time::Duration;
use textual::prelude::*;

const CSS: &str = "
#cards { height: 1fr; }
.card { width: 1fr; height: 1fr; border: round $accent; padding: 1 2; }
#status { height: 1; padding: 0 1; background: $panel; }
HelpScreen { align: center middle; }
#help { width: 50; height: auto; border: thick $primary; padding: 1 2; background: $surface; }
";

/// The dashboard mode's screen.
struct Dashboard;

impl Widget for Dashboard {
    fn type_names(&self) -> &'static [&'static str] {
        &["Dashboard"]
    }
    fn compose(&mut self) -> Vec<Child> {
        compose![
            Header::new(),
            Horizontal::new().id("cards").with([
                TextArea::new("Notes: press f3 to maximize me.")
                    .id("notes")
                    .classes("card"),
                Static::new("Data: press f2 to reload")
                    .id("data")
                    .classes("card"),
            ]),
            Label::new("idle").id("status"),
            Footer::new(),
        ]
    }
}

/// The log mode's screen.
struct LogView;

impl Widget for LogView {
    fn type_names(&self) -> &'static [&'static str] {
        &["LogView"]
    }
    fn compose(&mut self) -> Vec<Child> {
        compose![Header::new(), Log::new().id("log"), Footer::new()]
    }
}

/// A named screen (registered with `App::screen`, reused when pushed again).
struct HelpScreen;

impl Widget for HelpScreen {
    fn type_names(&self) -> &'static [&'static str] {
        &["HelpScreen"]
    }
    fn compose(&mut self) -> Vec<Child> {
        compose![Static::new(
            "[b]Help[/]\n\nf4 / f5: switch mode\nf2: reload data\nf3: maximize, escape: restore\n\nescape to close"
        )
        .id("help")]
    }
    fn bindings(&self) -> Vec<Binding> {
        vec![Binding::new("escape", "app.pop_screen", "Close")]
    }
}

#[derive(Debug)]
struct Loaded(u32);

#[derive(Default)]
struct ModesApp {
    reloads: u32,
}

impl Widget for ModesApp {
    fn bindings(&self) -> Vec<Binding> {
        vec![
            Binding::new("f4", "switch_mode('dashboard')", "Dashboard"),
            Binding::new("f5", "switch_mode('log')", "Log"),
            Binding::new("f1", "push_screen('help')", "Help"),
            Binding::new("f2", "reload", "Reload").id("app.reload"),
            Binding::new("f3", "maximize", "Maximize").id("app.maximize"),
            Binding::new("ctrl+s", "screenshot", "Screenshot"),
            Binding::new("ctrl+z", "suspend_process", "Suspend").hidden(),
        ]
    }

    fn run_action(&mut self, action: &str, _args: &[String], ctx: &mut Ctx) -> bool {
        if action != "reload" {
            return false;
        }
        let Some(data) = ctx.query_id("#data") else {
            return true;
        };
        ctx.set_loading(data, true);
        self.reloads += 1;
        let n = self.reloads;
        ctx.run_worker_with(
            async move {
                tokio::time::sleep(Duration::from_secs(2)).await;
                Loaded(n)
            },
            WorkerOptions::new()
                .name("reload")
                .group("data")
                .exclusive(),
        );
        true
    }

    fn on_message(&mut self, m: &Message, ctx: &mut Ctx) -> Flow {
        if let Some(e) = m.get::<worker::StateChanged>() {
            let line = format!("worker {} ({}): {:?}", e.name, e.group, e.state);
            if let Some(status) = ctx.query_one::<Label>("#status") {
                status.update(&line);
            }
            ctx.with::<Log, _>("#log", |log, ctx| log.write_line(&line, ctx));
        }
        if let Some(Loaded(n)) = m.get::<Loaded>() {
            if let Some(data) = ctx.query_id("#data") {
                ctx.set_loading(data, false);
            }
            if let Some(s) = ctx.query_one::<Static>("#data") {
                s.update(format!("Data: loaded {n} time(s)"));
            }
        }
        Flow::Continue
    }
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    App::new(ModesApp::default())
        .title("Modes")
        .css(CSS)
        .mode("dashboard", || Dashboard)
        .mode("log", || LogView)
        .default_mode("dashboard")
        .screen("help", || HelpScreen)
        .run()
        .await
}

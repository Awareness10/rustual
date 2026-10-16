//! A dashboard of display widgets: Header (with clock), Footer, ProgressBar, Sparkline,
//! Digits, Rule, Placeholders in a Grid, and toasts.
//!
//! Run with `cargo run -p rustual --example dashboard`.
//! Keys: `n` notify, `r` reset progress, `d` toggle dark, `ctrl+q` quit (or click the footer).

use rustual::prelude::*;
use std::time::Duration;

const CSS: &str = "
#stats {
    height: auto;
    padding: 0 1;
}
#counter-box {
    width: 24;
    height: auto;
    border: round $accent;
    border-title-color: $accent;
}
#counter {
    width: auto;
    color: $accent;
    text-style: bold;
}
#meters {
    height: auto;
    border: round $primary;
    padding: 0 1;
}
#meters Label {
    color: $text-muted;
}
#progress {
    margin-bottom: 1;
}
#spark {
    height: 3;
}
#grid {
    grid-size: 3 2;
    grid-gutter: 1 2;
    padding: 0 1;
}
";

struct Dashboard {
    ticks: u64,
}

impl Widget for Dashboard {
    fn compose(&mut self) -> Vec<Child> {
        compose![
            Header::new().show_clock(true),
            Horizontal::new().id("stats").with(compose![
                Vertical::new()
                    .id("counter-box")
                    .border_title("Uptime (s)")
                    .with(compose![Digits::new("0").id("counter")]),
                Vertical::new()
                    .id("meters")
                    .border_title("Activity")
                    .with(compose![
                        Label::new("Job progress"),
                        ProgressBar::new(Some(100.0)).id("progress"),
                        Label::new("Load"),
                        Sparkline::new(vec![0.0; 40]).id("spark"),
                    ]),
            ]),
            Rule::horizontal().line_style(LineStyle::Heavy),
            Grid::new().id("grid").with(compose![
                Placeholder::with_label("Inbox"),
                Placeholder::with_label("Calendar"),
                Placeholder::with_label("Tasks"),
                Placeholder::new().id("metrics"),
                Placeholder::new().variant(PlaceholderVariant::Size),
                Placeholder::new().variant(PlaceholderVariant::Text),
            ]),
            Footer::new(),
        ]
    }

    fn on_mount(&mut self, ctx: &mut Ctx) {
        ctx.set_interval(Duration::from_millis(250), |app: &mut Dashboard, ctx| {
            app.ticks += 1;
            let t = app.ticks as f64;
            let load = 50.0 + 30.0 * (t / 5.0).sin() + 15.0 * (t / 1.7).cos();
            if let Some(s) = ctx.query_one::<Sparkline>("#spark") {
                s.push(load, 40);
            }
            if let Some(pb) = ctx.query_one::<ProgressBar>("#progress") {
                if pb.percentage() < Some(1.0) {
                    pb.advance(0.5);
                }
            }
            if app.ticks % 4 == 0 {
                let secs = app.ticks / 4;
                if let Some(d) = ctx.query_one::<Digits>("#counter") {
                    d.update(secs.to_string());
                }
            }
        });
    }

    fn bindings(&self) -> Vec<Binding> {
        vec![
            Binding::new("n", "notify_me", "Notify"),
            Binding::new("r", "reset", "Reset progress"),
            Binding::new("d", "app.toggle_dark", "Dark mode"),
            Binding::new("ctrl+q", "quit", "Quit").priority(),
        ]
    }

    fn run_action(&mut self, action: &str, _args: &[String], ctx: &mut Ctx) -> bool {
        match action {
            "notify_me" => {
                let secs = self.ticks / 4;
                ctx.notify_with(
                    &format!("The dashboard has been up for [b]{secs}[/b] seconds."),
                    "Hello",
                    Severity::Information,
                    None,
                );
            }
            "reset" => {
                if let Some(pb) = ctx.query_one::<ProgressBar>("#progress") {
                    pb.update(Some(100.0), 0.0);
                }
                ctx.notify_with("Progress reset", "", Severity::Warning, None);
            }
            _ => return false,
        }
        true
    }
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    App::new(Dashboard { ticks: 0 })
        .title("Dashboard")
        .sub_title("rustual widgets")
        .css(CSS)
        .run()
        .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(start_paused = true)]
    async fn dashboard_renders() {
        let mut pilot = App::new(Dashboard { ticks: 0 })
            .title("Dashboard")
            .sub_title("rustual widgets")
            .css(CSS)
            .test(Size::new(100, 30))
            .await;
        assert!(pilot.css_errors().is_empty(), "{:?}", pilot.css_errors());
        pilot.wait(Duration::from_secs(3)).await;
        pilot.press(&["n"]).await;
        let text = pilot.screen_text();
        println!("{text}");
        assert!(text.contains("Dashboard — rustual widgets"));
        assert!(text.contains("Notify"));
        assert!(text.contains("Hello"));
        assert!(text.contains("Inbox"));
    }
}

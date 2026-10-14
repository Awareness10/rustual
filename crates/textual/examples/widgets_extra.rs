//! Extra widgets: SelectionList, MaskedInput, DirectoryTree, Pretty, a gradient ProgressBar,
//! and the Welcome screen.
//!
//! Run with `cargo run -p textual --example widgets_extra`.
//! Keys: `tab` to move focus, `w` to show the Welcome screen (its OK button quits),
//! `ctrl+q` to quit.

use textual::prelude::*;
use textual::render::Gradient;
use textual::widgets::directory_tree::{DirectorySelected, FileSelected};
use textual::widgets::selection_list::SelectedChanged;

const CSS: &str = "
#tabs {
    height: 1fr;
}
#games {
    width: 1fr;
    height: 1fr;
}
#card, #date {
    width: 40;
}
#files {
    height: 1fr;
}
#pretty {
    padding: 0 1;
}
#status {
    dock: bottom;
    height: 1;
    background: $panel;
    padding: 0 1;
}
#progress {
    margin: 1 0;
}
";

/// A screen showing the Welcome widget.
struct WelcomeScreen;

impl Widget for WelcomeScreen {
    fn type_names(&self) -> &'static [&'static str] {
        &["WelcomeScreen", "Screen"]
    }
    fn compose(&mut self) -> Vec<Child> {
        compose![Welcome::new()]
    }
}

#[derive(Debug)]
#[allow(dead_code)]
struct Settings {
    games: Vec<&'static str>,
    card: String,
    date: String,
    last_file: Option<String>,
}

struct Extra {
    settings: Settings,
}

impl Extra {
    fn new() -> Self {
        Extra {
            settings: Settings {
                games: vec!["black_jack"],
                card: String::new(),
                date: String::new(),
                last_file: None,
            },
        }
    }

    fn refresh_pretty(&self, ctx: &mut Ctx) {
        if let Some(p) = ctx.query_one::<Pretty>("#pretty") {
            p.update(&self.settings);
        }
    }

    fn status(ctx: &mut Ctx, s: String) {
        if let Some(l) = ctx.query_one::<Label>("#status") {
            l.update(s);
        }
    }
}

impl Widget for Extra {
    fn compose(&mut self) -> Vec<Child> {
        let gradient = Gradient::from_colors(&[
            Color::parse("#881177").unwrap(),
            Color::parse("#aa3355").unwrap(),
            Color::parse("#ee8800").unwrap(),
            Color::parse("#aacc22").unwrap(),
            Color::parse("#00aabb").unwrap(),
        ])
        .unwrap();
        let mut progress = ProgressBar::new(Some(100.0)).gradient(gradient);
        progress.set_progress(70.0);
        let games = SelectionList::new()
            .selection(("Falken's Maze", "secret_back_door"))
            .selection(("Black Jack", "black_jack", true))
            .selection(("Gin Rummy", "gin_rummy"))
            .selection(("Hearts", "hearts"))
            .selection(("Bridge", "bridge"))
            .selection(("Checkers", "checkers"))
            .selection(("Chess", "a_nice_game_of_chess"))
            .selection(("Poker", "poker"))
            .selection(("Fighter Combat", "fighter_combat"));
        let files = DirectoryTree::new(".").filter_paths(|p| {
            !p.file_name()
                .is_some_and(|n| n.to_string_lossy().starts_with('.'))
        });
        compose![
            TabbedContent::new().id("tabs").with(compose![
                TabPane::new("Selections")
                    .id("selections-pane")
                    .with(compose![games
                        .id("games")
                        .border_title("Shall we play some games?")]),
                TabPane::new("Masked input")
                    .id("masked-pane")
                    .with(compose![
                        Label::new("Credit card:"),
                        MaskedInput::new("9999-9999-9999-9999;0").id("card"),
                        Label::new("Date (dd/mm/yyyy):"),
                        MaskedInput::new("99/99/9999")
                            .placeholder("dd/mm/yyyy")
                            .id("date"),
                        progress.id("progress"),
                    ]),
                TabPane::new("Files")
                    .id("files-pane")
                    .with(compose![files.id("files")]),
                TabPane::new("Pretty")
                    .id("pretty-pane")
                    .with(compose![Pretty::new(&self.settings).id("pretty")]),
            ]),
            Label::new("Press w for the welcome screen").id("status"),
        ]
    }

    fn bindings(&self) -> Vec<Binding> {
        vec![
            Binding::new("w", "welcome", "Welcome"),
            Binding::new("ctrl+q", "app.quit", "Quit"),
        ]
    }

    fn run_action(&mut self, action: &str, _args: &[String], ctx: &mut Ctx) -> bool {
        if action == "welcome" {
            ctx.push_screen(WelcomeScreen);
            return true;
        }
        false
    }

    fn on_message(&mut self, msg: &Message, ctx: &mut Ctx) -> Flow {
        if let Some(c) = msg.get::<SelectedChanged<&'static str>>() {
            self.settings.games = c.selected.clone();
            self.refresh_pretty(ctx);
            Self::status(ctx, format!("Selected: {}", c.selected.join(", ")));
        } else if let Some(c) = msg.get::<masked_input::Changed>() {
            let valid = ctx
                .query_one::<MaskedInput>(match c.input_id.as_deref() {
                    Some("card") => "#card",
                    _ => "#date",
                })
                .map(|m| m.is_valid())
                .unwrap_or(false);
            match c.input_id.as_deref() {
                Some("card") => self.settings.card = c.value.clone(),
                _ => self.settings.date = c.value.clone(),
            }
            self.refresh_pretty(ctx);
            Self::status(
                ctx,
                format!(
                    "{} ({})",
                    c.value,
                    if valid { "valid" } else { "incomplete" }
                ),
            );
        } else if let Some(f) = msg.get::<FileSelected>() {
            self.settings.last_file = Some(f.path.display().to_string());
            self.refresh_pretty(ctx);
            Self::status(ctx, format!("File: {}", f.path.display()));
        } else if let Some(d) = msg.get::<DirectorySelected>() {
            Self::status(ctx, format!("Directory: {}", d.path.display()));
        }
        Flow::Continue
    }
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    App::new(Extra::new())
        .css(CSS)
        .title("Extra widgets")
        .run()
        .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn widgets_extra_renders() {
        let mut p = App::new(Extra::new())
            .css(CSS)
            .test(Size::new(100, 30))
            .await;
        assert!(p.css_errors().is_empty(), "{:?}", p.css_errors());
        let text = p.screen_text();
        assert!(text.contains("Falken's Maze"), "{text}");
        p.press(&["w"]).await;
        assert!(p.screen_text().contains("Welcome!"));
    }
}

use rustual::command::{FuzzySearch, Matcher};
use rustual::prelude::*;
use rustual::widgets::command_palette::{self, CommandList, CommandPalette};

#[derive(Default)]
struct PaletteApp {
    events: Vec<String>,
}

impl Widget for PaletteApp {
    fn compose(&mut self) -> Vec<Child> {
        compose![Label::new("main").id("main"), Button::new("B").id("b")]
    }

    fn commands(&self) -> Vec<Command> {
        vec![Command::new(
            "Ring bell",
            "Ring the terminal bell",
            "app.bell",
        )]
    }

    fn on_message(&mut self, m: &Message, _ctx: &mut Ctx) -> Flow {
        if m.is::<command_palette::Opened>() {
            self.events.push("opened".into());
        }
        if let Some(c) = m.get::<command_palette::Closed>() {
            self.events.push(format!("closed:{}", c.option_selected));
        }
        Flow::Continue
    }
}

async fn pilot() -> Pilot {
    App::new(PaletteApp::default())
        .command_provider(|_ctx: &mut Ctx| {
            vec![Command::callback(
                "Say hello",
                "Show a notification",
                |ctx| ctx.notify("hello from palette"),
            )]
        })
        .test(Size::new(80, 24))
        .await
}

fn hits(p: &Pilot) -> Vec<String> {
    p.query_one::<CommandPalette>("CommandPalette")
        .map(|c| c.hit_names().into_iter().map(String::from).collect())
        .unwrap_or_default()
}

#[tokio::test]
async fn ctrl_p_opens_palette_with_discovery_list() {
    let mut p = pilot().await;
    assert_eq!(p.screen_count(), 1);
    p.press(&["ctrl+p"]).await;
    assert_eq!(p.screen_count(), 2);
    assert!(p.css_errors().is_empty(), "{:?}", p.css_errors());
    let names = hits(&p);
    for n in ["Ring bell", "Say hello", "Theme", "Quit", "Keys"] {
        assert!(names.iter().any(|x| x == n), "{n} missing from {names:?}");
    }
    // The input is focused.
    assert_eq!(p.focused_id().as_deref(), Some("--command-input"));
    let text = p.screen_text();
    assert!(text.contains("Ring the terminal bell"), "{text}");
    assert_eq!(p.app::<PaletteApp>().unwrap().events, vec!["opened"]);
    // A second ctrl+p does not stack palettes.
    p.press(&["ctrl+p"]).await;
    assert_eq!(p.screen_count(), 2);
}

#[tokio::test]
async fn typing_filters_and_enter_runs_action() {
    let mut p = pilot().await;
    p.press(&["ctrl+p"]).await;
    p.type_text("bell").await;
    let names = hits(&p);
    assert_eq!(names.first().map(String::as_str), Some("Ring bell"));
    assert!(!names.iter().any(|n| n == "Quit"));
    p.press(&["enter"]).await;
    assert_eq!(p.screen_count(), 1);
    assert_eq!(p.bell_count(), 1);
    assert_eq!(
        p.app::<PaletteApp>().unwrap().events,
        vec!["opened", "closed:true"]
    );
}

#[tokio::test]
async fn callback_command_runs_after_close() {
    let mut p = pilot().await;
    p.press(&["ctrl+p"]).await;
    p.type_text("hello").await;
    p.press(&["enter"]).await;
    assert_eq!(p.screen_count(), 1);
    assert_eq!(p.notifications(), vec!["hello from palette".to_string()]);
}

#[tokio::test]
async fn escape_closes_without_running() {
    let mut p = pilot().await;
    p.press(&["ctrl+p"]).await;
    p.press(&["escape"]).await;
    assert_eq!(p.screen_count(), 1);
    assert_eq!(p.bell_count(), 0);
    assert_eq!(
        p.app::<PaletteApp>().unwrap().events,
        vec!["opened", "closed:false"]
    );
}

#[tokio::test]
async fn no_matches_and_navigation() {
    let mut p = pilot().await;
    p.press(&["ctrl+p"]).await;
    p.type_text("zzzz").await;
    assert!(hits(&p).is_empty());
    assert!(p.screen_text().contains("No matches found"));
    p.press(&["enter"]).await;
    assert_eq!(p.screen_count(), 2);
    // Clear and navigate down then up.
    p.press(&["ctrl+u"]).await;
    let first = hits(&p)[0].clone();
    p.press(&["down", "down", "up"]).await;
    let list = p.query_one::<CommandList>("CommandList").unwrap();
    assert_eq!(list.options().highlighted(), Some(1));
    assert_ne!(first, "");
}

#[tokio::test]
async fn click_on_command_runs_it_and_background_closes() {
    let mut p = pilot().await;
    p.press(&["ctrl+p"]).await;
    p.type_text("ring").await;
    // Click the first option row (inside the list's content region).
    let id = p.query("CommandList")[0];
    let r = p.dom().get(id).unwrap().layout.content_region;
    p.click_at(r.x + 3, r.y).await;
    assert_eq!(p.screen_count(), 1);
    assert_eq!(p.bell_count(), 1);

    p.press(&["ctrl+p"]).await;
    p.click_at(5, 23).await;
    assert_eq!(p.screen_count(), 1);
}

#[tokio::test]
async fn theme_command_opens_theme_list() {
    let mut p = pilot().await;
    p.press(&["ctrl+p"]).await;
    p.type_text("theme").await;
    assert_eq!(hits(&p)[0], "Theme");
    p.press(&["enter"]).await;
    // A second palette with theme names.
    assert_eq!(p.screen_count(), 2);
    assert!(hits(&p).iter().any(|h| h == "nord"));
    p.type_text("nord").await;
    assert_eq!(hits(&p)[0], "nord");
    p.press(&["enter"]).await;
    assert_eq!(p.screen_count(), 1);
    assert_eq!(p.theme(), "nord");
}

#[tokio::test]
async fn keys_command_toggles_key_panel() {
    let mut p = pilot().await;
    p.press(&["ctrl+p"]).await;
    p.type_text("keys").await;
    p.press(&["enter"]).await;
    assert_eq!(p.query("KeyPanel").len(), 1);
}

#[tokio::test]
async fn disabled_palette() {
    let mut p = App::new(PaletteApp::default())
        .enable_command_palette(false)
        .test(Size::new(80, 24))
        .await;
    p.press(&["ctrl+p"]).await;
    assert_eq!(p.screen_count(), 1);
}

#[tokio::test]
async fn header_icon_opens_palette() {
    struct H;
    impl Widget for H {
        fn compose(&mut self) -> Vec<Child> {
            compose![Header::new()]
        }
    }
    let mut p = App::new(H).test(Size::new(80, 24)).await;
    assert!(p.click("HeaderIcon").await);
    assert_eq!(p.screen_count(), 2);
}

#[test]
fn fuzzy_ranking() {
    let m = Matcher::new("tm");
    // First letters of words beat letters in the middle.
    assert!(m.score("Toggle mode") > m.score("Item"));
    let m = Matcher::new("quit");
    assert!(m.score("Quit") > m.score("Quit the app"));
    assert!(m.score("Quit the app") > m.score("q u i t"));
    assert_eq!(m.score("Theme"), 0.0);
    // Case-insensitive by default; case-sensitive opt-in.
    assert!(Matcher::new("QUIT").score("quit") > 0.0);
    assert_eq!(Matcher::new("QUIT").case_sensitive(true).score("quit"), 0.0);
    let (_, offsets) = FuzzySearch::default().match_offsets("fb", "foo bar");
    assert_eq!(offsets, vec![0, 4]);
}

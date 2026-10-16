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

/// Search hook log: events, and the palette / generation of the latest query.
#[derive(Default)]
struct SearchLog {
    events: Vec<String>,
    palette: Option<NodeId>,
    generation: u64,
}

#[tokio::test]
async fn search_hooks_get_queries_and_deliver_hits_later() {
    use rustual::command::SearchEvent;
    use std::sync::{Arc, Mutex};

    let log: Arc<Mutex<SearchLog>> = Default::default();
    let hook_log = log.clone();
    let mut p = App::new(PaletteApp::default())
        .command_search(move |palette, event, _ctx: &mut Ctx| {
            let mut l = hook_log.lock().unwrap();
            l.palette = Some(palette);
            match event {
                SearchEvent::Opened => l.events.push("opened".into()),
                SearchEvent::Query { query, generation } => {
                    l.events.push(format!("query {query:?} #{generation}"));
                    l.generation = generation;
                }
                SearchEvent::Closed => l.events.push("closed".into()),
            }
        })
        .test(Size::new(70, 24))
        .await;
    p.press(&["ctrl+p"]).await;
    // Discovery: hits arrive after the query, in order, after the built-in commands.
    let (palette, generation) = {
        let l = log.lock().unwrap();
        assert_eq!(l.events, vec!["opened", "query \"\" #1"]);
        (l.palette.unwrap(), l.generation)
    };
    let deliver = |hits: Vec<Command>, generation: u64| {
        move |_: &mut PaletteApp, ctx: &mut Ctx| ctx.add_command_hits(palette, generation, hits)
    };
    p.with::<PaletteApp, _>(
        "App",
        deliver(vec![Command::new("Remote one", "From a provider", "app.bell")], generation),
    )
    .await;
    assert_eq!(hits(&p).last().map(String::as_str), Some("Remote one"));

    // A search: scored hits merge with the palette's own matches by score.
    p.type_text("re").await;
    let generation = log.lock().unwrap().generation;
    assert_eq!(generation, 3, "{:?}", log.lock().unwrap().events);
    p.with::<PaletteApp, _>(
        "App",
        deliver(
            vec![
                // Scores are on the matcher's scale (Textual's): "re" in "Focus previous" is 6.
                Command::callback("Remote best", "", |ctx| ctx.notify("ran remote"))
                    .score(100.0)
                    .display(Text::from_markup("[b]Re[/]mote best")),
                Command::new("Remote worst", "", "app.bell").score(0.01),
            ],
            generation,
        ),
    )
    .await;
    let names = hits(&p);
    assert_eq!(names.first().map(String::as_str), Some("Remote best"), "{names:?}");
    assert_eq!(names.last().map(String::as_str), Some("Remote worst"), "{names:?}");
    // Stale hits (an older generation) are ignored.
    p.with::<PaletteApp, _>(
        "App",
        deliver(vec![Command::new("Stale", "", "app.bell").score(1.0)], 1),
    )
    .await;
    assert!(!hits(&p).iter().any(|n| n == "Stale"));
    // Running a delivered hit; the hook hears the palette close.
    p.press(&["enter"]).await;
    assert_eq!(p.notifications(), vec!["ran remote".to_string()]);
    assert_eq!(log.lock().unwrap().events.last().map(String::as_str), Some("closed"));
}

#[tokio::test]
async fn system_commands_can_be_turned_off() {
    let mut p = App::new(PaletteApp::default())
        .system_commands(false)
        .test(Size::new(70, 24))
        .await;
    p.press(&["ctrl+p"]).await;
    let names = hits(&p);
    assert!(names.iter().any(|n| n == "Ring bell"), "{names:?}");
    assert!(!names.iter().any(|n| n == "Quit"), "{names:?}");
}

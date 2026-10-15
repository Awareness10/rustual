//! The `screenshot` action and "Screenshot" system command (Textual: `action_screenshot`),
//! and the `suspend_process` action (headless: only recorded).

use rustual::prelude::*;
use rustual::widgets::command_palette::CommandPalette;

struct Root;

impl Widget for Root {
    fn compose(&mut self) -> Vec<Child> {
        compose![Label::new("snapshot me")]
    }
}

fn temp_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("rustual-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[tokio::test]
async fn screenshot_action_saves_svg_and_notifies() {
    let dir = temp_dir("shot");
    let mut p = App::new(Root)
        .title("Shot App")
        .test(Size::new(40, 8))
        .await;
    let action = format!("app.screenshot('one.svg', '{}')", dir.display());
    p.with::<Root, _>("App", |_, ctx| ctx.run_action(&action))
        .await;
    let svg = std::fs::read_to_string(dir.join("one.svg")).unwrap();
    assert!(svg.starts_with("<svg"), "{}", &svg[..40.min(svg.len())]);
    assert!(svg.contains("snapshot"));
    let notes = p.notifications();
    assert!(
        notes.iter().any(|n| n.starts_with("Saved screenshot to")),
        "{notes:?}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn screenshot_default_filename_uses_title_and_time() {
    let dir = temp_dir("auto");
    let mut p = App::new(Root).title("My App").test(Size::new(40, 8)).await;
    let d = dir.to_str().unwrap().to_string();
    let path = p
        .with::<Root, _>("App", |_, ctx| ctx.save_screenshot(None, Some(&d)))
        .await
        .unwrap()
        .unwrap();
    let name = path.file_name().unwrap().to_str().unwrap().to_string();
    assert!(name.starts_with("My_App_20"), "{name}");
    assert!(name.ends_with(".svg") && !name.contains(':'), "{name}");
    assert!(path.exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn palette_lists_screenshot_command() {
    let mut p = App::new(Root).test(Size::new(60, 20)).await;
    p.press(&["ctrl+p"]).await;
    let hits: Vec<String> = p
        .query_one::<CommandPalette>("CommandPalette")
        .map(|c| c.hit_names().into_iter().map(String::from).collect())
        .unwrap();
    assert!(hits.contains(&"Screenshot".to_string()), "{hits:?}");
}

#[tokio::test]
async fn suspend_process_is_requested_not_bound_by_default() {
    struct Bound;
    impl Widget for Bound {
        fn bindings(&self) -> Vec<Binding> {
            vec![Binding::new("ctrl+z", "suspend_process", "Suspend")]
        }
    }
    // Not bound by default (like Textual 8).
    let mut p = App::new(Root).test(Size::new(40, 8)).await;
    p.press(&["ctrl+z"]).await;
    assert!(!p.suspend_requested());
    // Bound by the app: headless apps only record the request.
    let mut p = App::new(Bound).test(Size::new(40, 8)).await;
    p.press(&["ctrl+z"]).await;
    assert_eq!(p.suspend_requested(), cfg!(unix));
    assert!(!p.has_exited());
}

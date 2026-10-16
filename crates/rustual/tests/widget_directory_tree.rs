//! DirectoryTree tests (each test uses its own temp dir).

use rustual::prelude::*;
use rustual::widgets::directory_tree::{DirectorySelected, FileSelected};
use std::fs;
use std::path::{Path, PathBuf};

struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> Self {
        let dir =
            std::env::temp_dir().join(format!("rustual-dirtree-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("src/nested")).unwrap();
        fs::create_dir_all(dir.join("Docs")).unwrap();
        fs::write(dir.join("b.txt"), "b").unwrap();
        fs::write(dir.join("A.md"), "a").unwrap();
        fs::write(dir.join(".hidden"), "h").unwrap();
        fs::write(dir.join("src/main.rs"), "fn main() {}").unwrap();
        TempDir(dir)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct DtApp {
    path: PathBuf,
    hide: bool,
    events: Vec<String>,
}

impl Widget for DtApp {
    fn compose(&mut self) -> Vec<Child> {
        let mut dt = DirectoryTree::new(self.path.clone());
        if self.hide {
            dt = dt.filter_paths(|p: &Path| {
                !p.file_name()
                    .is_some_and(|n| n.to_string_lossy().starts_with('.'))
            });
        }
        compose![dt.id("dt")]
    }
    fn on_message(&mut self, m: &Message, _ctx: &mut Ctx) -> Flow {
        let name = |p: &Path| p.file_name().unwrap().to_string_lossy().into_owned();
        if let Some(f) = m.get::<FileSelected>() {
            self.events.push(format!("file {}", name(&f.path)));
        }
        if let Some(d) = m.get::<DirectorySelected>() {
            self.events.push(format!("dir {}", name(&d.path)));
        }
        if m.get::<tree::NodeSelected>().is_some() || m.get::<tree::NodeExpanded>().is_some() {
            self.events.push("leaked tree message".into());
        }
        Flow::Continue
    }
}

async fn pilot(dir: &TempDir, hide: bool) -> Pilot {
    App::new(DtApp {
        path: dir.0.clone(),
        hide,
        events: vec![],
    })
    .test(Size::new(60, 20))
    .await
}

fn labels(p: &Pilot) -> Vec<String> {
    let dt = p.query_one::<DirectoryTree>("#dt").unwrap();
    (0..dt.line_count() as i32)
        .map(|l| dt.node(dt.node_at_line(l).unwrap()).unwrap().label_text())
        .collect()
}

fn events(p: &Pilot) -> Vec<String> {
    p.app::<DtApp>().unwrap().events.clone()
}

#[tokio::test]
async fn loads_root_sorted_folders_first() {
    let dir = TempDir::new("sorted");
    let mut p = pilot(&dir, false).await;
    assert!(p.css_errors().is_empty(), "{:?}", p.css_errors());
    let l = labels(&p);
    assert_eq!(
        &l[1..],
        ["Docs", "src", ".hidden", "A.md", "b.txt"],
        "{l:?}"
    );
    let text = p.screen_text();
    assert!(text.contains("📂 "), "{text}");
    assert!(text.contains("📁 Docs"), "{text}");
    assert!(text.contains("📄 A.md"), "{text}");
    // Folders are bold, hidden files dim, extensions italic.
    let c = p.canvas();
    let lines: Vec<&str> = text.lines().collect();
    let col = |row: usize, s: &str| {
        let chars: Vec<char> = lines[row].chars().collect();
        let needle: Vec<char> = s.chars().collect();
        // Emoji are 2 cells wide: count cells up to the match.
        let i = (0..chars.len())
            .find(|i| chars[*i..].starts_with(&needle))
            .unwrap();
        chars[..i]
            .iter()
            .map(|c| rustual::render::cells::cell_len(&c.to_string()))
            .sum::<i32>()
    };
    let attrs = |row: usize, s: &str| c.get(col(row, s), row as i32).unwrap().attrs;
    use rustual::render::Attrs;
    assert!(attrs(1, "Docs").contains(Attrs::BOLD));
    // Dotfiles are dim: rendered (as in Textual's truecolor mode) as a color blended 66% from
    // the background towards the text color, dimmer than a normal file name.
    let cell = |row: usize, s: &str| c.get(col(row, s), row as i32).unwrap().clone();
    let (hidden, normal) = (cell(3, ".hidden"), cell(4, "A.md"));
    assert!(!hidden.attrs.contains(Attrs::DIM));
    assert_eq!(
        hidden.fg,
        hidden.bg.blend(normal.fg, rustual::filter::DIM_FACTOR)
    );
    assert!(!attrs(4, "A.md").contains(Attrs::ITALIC));
    assert!(attrs(4, ".md").contains(Attrs::ITALIC));
}

#[tokio::test]
async fn filter_hides_paths() {
    let dir = TempDir::new("filter");
    let p = pilot(&dir, true).await;
    assert_eq!(&labels(&p)[1..], ["Docs", "src", "A.md", "b.txt"]);
}

#[tokio::test]
async fn expanding_loads_lazily_and_selection_messages() {
    let dir = TempDir::new("lazy");
    let mut p = pilot(&dir, true).await;
    // Cursor to "src" (line 2) and select it: expands + DirectorySelected.
    p.press(&["down", "down", "down", "enter"]).await;
    let l = labels(&p);
    assert_eq!(&l[1..5], ["Docs", "src", "nested", "main.rs"], "{l:?}");
    assert!(p.screen_text().contains("📂 src"));
    assert_eq!(events(&p), vec!["dir src"]);
    let dt = p.query_one::<DirectoryTree>("#dt").unwrap();
    assert_eq!(dt.cursor_path().unwrap(), dir.0.join("src"));
    // Select main.rs → FileSelected.
    p.press(&["down", "down", "enter"]).await;
    assert_eq!(events(&p), vec!["dir src", "file main.rs"]);
    // Collapse src again with a second select on it.
    p.press(&["up", "up", "enter"]).await;
    assert_eq!(labels(&p).len(), 5);
}

#[tokio::test]
async fn click_selects_file() {
    let dir = TempDir::new("click");
    let mut p = pilot(&dir, true).await;
    let text = p.screen_text();
    let row = text.lines().position(|l| l.contains("b.txt")).unwrap();
    p.click_at(12, row as i32).await;
    assert_eq!(events(&p), vec!["file b.txt"]);
}

#[tokio::test]
async fn reload_keeps_expanded_and_cursor() {
    let dir = TempDir::new("reload");
    let mut p = pilot(&dir, true).await;
    p.press(&["down", "down", "down", "enter", "down", "down"])
        .await;
    let dt = p.query_one::<DirectoryTree>("#dt").unwrap();
    assert_eq!(dt.cursor_path().unwrap(), dir.0.join("src/main.rs"));
    fs::write(dir.0.join("src/lib.rs"), "").unwrap();
    fs::write(dir.0.join("c.txt"), "").unwrap();
    p.with::<DirectoryTree, _>("#dt", |dt, ctx| dt.reload(ctx))
        .await;
    let l = labels(&p);
    assert_eq!(
        &l[1..],
        ["Docs", "src", "nested", "lib.rs", "main.rs", "A.md", "b.txt", "c.txt"],
        "{l:?}"
    );
    let dt = p.query_one::<DirectoryTree>("#dt").unwrap();
    assert_eq!(dt.cursor_path().unwrap(), dir.0.join("src/main.rs"));
    // Removing the highlighted file moves the cursor to its folder.
    fs::remove_file(dir.0.join("src/main.rs")).unwrap();
    p.with::<DirectoryTree, _>("#dt", |dt, ctx| dt.reload(ctx))
        .await;
    let dt = p.query_one::<DirectoryTree>("#dt").unwrap();
    assert_eq!(dt.cursor_path().unwrap(), dir.0.join("src"));
}

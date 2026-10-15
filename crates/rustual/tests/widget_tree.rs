//! Tests for `Tree`.

use rustual::prelude::*;
use rustual::widgets::tree::{self, TreeNodeId};

struct TreeApp {
    tree: Option<Tree<u32>>,
    log: Vec<String>,
}

impl Widget for TreeApp {
    fn compose(&mut self) -> Vec<Child> {
        compose![self.tree.take().unwrap().id("tree")]
    }
    fn on_message(&mut self, msg: &Message, ctx: &mut Ctx) -> Flow {
        if let Some(m) = msg.get::<tree::NodeSelected>() {
            let data = ctx
                .get::<Tree<u32>>(msg.sender)
                .and_then(|t| t.data(m.node).copied());
            self.log.push(format!("selected {} {:?}", m.label, data));
        }
        if let Some(m) = msg.get::<tree::NodeExpanded>() {
            self.log.push(format!("expanded {}", m.label));
        }
        if let Some(m) = msg.get::<tree::NodeCollapsed>() {
            self.log.push(format!("collapsed {}", m.label));
        }
        if let Some(m) = msg.get::<tree::NodeHighlighted>() {
            self.log.push(format!("highlighted {}", m.label));
        }
        Flow::Continue
    }
}

/// Root
/// ├── Fruit (expanded)
/// │   ├── Apple
/// │   └── Banana
/// └── Veg (collapsed)
///     └── Carrot
fn sample() -> Tree<u32> {
    let mut t = Tree::new("Root");
    let root = t.root();
    t.set_expanded(root, true);
    let fruit = t.add(root, "Fruit", Some(1));
    t.set_expanded(fruit, true);
    t.add_leaf(fruit, "Apple", Some(2));
    t.add_leaf(fruit, "[b]Banana[/]", Some(3));
    let veg = t.add(root, "Veg", Some(4));
    t.add_leaf(veg, "Carrot", Some(5));
    t
}

async fn pilot(t: Tree<u32>) -> Pilot {
    App::new(TreeApp {
        tree: Some(t),
        log: vec![],
    })
    .test(Size::new(30, 8))
    .await
}

fn log(p: &Pilot) -> Vec<String> {
    p.app::<TreeApp>().unwrap().log.clone()
}

fn tree(p: &Pilot) -> &Tree<u32> {
    p.query_one::<Tree<u32>>("#tree").unwrap()
}

fn lines(p: &mut Pilot) -> Vec<String> {
    let mut l: Vec<String> = p.screen_text().lines().map(String::from).collect();
    while l.last().map(|s| s.is_empty()).unwrap_or(false) {
        l.pop();
    }
    l
}

#[tokio::test]
async fn renders_with_guides() {
    let mut p = pilot(sample()).await;
    assert!(p.css_errors().is_empty(), "{:?}", p.css_errors());
    let l = lines(&mut p);
    assert_eq!(l[0], "▼ Root");
    assert_eq!(l[1], "├── ▼ Fruit");
    assert_eq!(l[2], "│   ├── Apple");
    assert_eq!(l[3], "│   └── Banana");
    assert_eq!(l[4], "└── ▶ Veg");
    assert_eq!(l.len(), 5);
    assert_eq!(tree(&p).line_count(), 5);
    assert_eq!(tree(&p).cursor_line(), -1);
}

#[tokio::test]
async fn hidden_root_and_guide_depth() {
    let mut p = pilot(sample().show_root(false).guide_depth(2)).await;
    let l = lines(&mut p);
    assert_eq!(l[0], "▼ Fruit");
    assert_eq!(l[1], "├ Apple");
    assert_eq!(l[2], "└ Banana");
    assert_eq!(l[3], "▶ Veg");
}

#[tokio::test]
async fn cursor_navigation_and_highlight_messages() {
    let mut p = pilot(sample()).await;
    assert_eq!(p.focused_id().as_deref(), Some("tree"));
    p.press(&["down"]).await;
    assert_eq!(tree(&p).cursor_line(), 0);
    p.press(&["down", "down"]).await;
    assert_eq!(tree(&p).cursor_line(), 2);
    p.press(&["up"]).await;
    assert_eq!(tree(&p).cursor_line(), 1);
    p.press(&["end"]).await;
    assert_eq!(tree(&p).cursor_line(), 4);
    p.press(&["down"]).await;
    assert_eq!(tree(&p).cursor_line(), 4, "clamped");
    p.press(&["home"]).await;
    assert_eq!(
        log(&p),
        vec![
            "highlighted Root",
            "highlighted Fruit",
            "highlighted Apple",
            "highlighted Fruit",
            "highlighted Veg",
            "highlighted Root",
        ]
    );
    // Cursor line is drawn with the cursor style.
    let c = p.canvas();
    assert_ne!(c.get(2, 0).unwrap().bg, c.get(2, 1).unwrap().bg);
}

#[tokio::test]
async fn enter_selects_and_toggles() {
    let mut p = pilot(sample()).await;
    p.press(&["end", "enter"]).await;
    let l = lines(&mut p);
    assert_eq!(l[4], "└── ▼ Veg");
    assert_eq!(l[5], "    └── Carrot");
    assert!(log(&p).ends_with(&[
        "selected Veg Some(4)".to_string(),
        "expanded Veg".to_string()
    ]));
    p.press(&["down", "enter"]).await;
    assert!(
        log(&p).ends_with(&["selected Carrot Some(5)".to_string()]),
        "leaf: no toggle"
    );
    p.press(&["up", "space"]).await;
    assert_eq!(log(&p).last().unwrap(), "collapsed Veg");
    assert_eq!(tree(&p).line_count(), 5);
}

#[tokio::test]
async fn left_right_collapse_expand_and_move() {
    let mut p = pilot(sample()).await;
    p.press(&["down", "down"]).await; // Fruit
    p.press(&["left"]).await;
    assert!(!tree(&p).node(TreeNodeId(1)).unwrap().is_expanded());
    p.press(&["left"]).await;
    assert_eq!(tree(&p).cursor_line(), 0, "moved to parent");
    p.press(&["down", "right"]).await;
    assert!(tree(&p).node(TreeNodeId(1)).unwrap().is_expanded());
    p.press(&["right"]).await;
    assert_eq!(tree(&p).cursor_line(), 2, "moved to first child");
    p.press(&["shift+down"]).await;
    assert_eq!(tree(&p).cursor_line(), 3, "next sibling");
    p.press(&["shift+down"]).await;
    assert_eq!(tree(&p).cursor_line(), 4, "parent's next sibling");
    p.press(&["shift+up"]).await;
    assert_eq!(tree(&p).cursor_line(), 1, "previous sibling");
    p.press(&["down", "shift+left"]).await;
    assert_eq!(tree(&p).cursor_line(), 1, "parent");
}

#[tokio::test]
async fn click_toggle_icon_and_label() {
    let mut p = pilot(sample()).await;
    // Click the ▶ of "Veg" (guide width 4) — toggles without selecting.
    p.click_at(4, 4).await;
    assert_eq!(log(&p), vec!["expanded Veg"]);
    assert_eq!(tree(&p).cursor_line(), -1);
    // Click the label of "Apple": moves the cursor and selects.
    p.click_at(9, 2).await;
    assert_eq!(tree(&p).cursor_line(), 2);
    assert!(log(&p).ends_with(&[
        "highlighted Apple".to_string(),
        "selected Apple Some(2)".to_string()
    ]));
    // Clicking the label of an expandable node selects and (auto-expand) toggles it.
    p.click_at(8, 1).await;
    assert_eq!(log(&p).last().unwrap(), "collapsed Fruit");
}

#[tokio::test]
async fn hover_highlights_line() {
    let mut p = pilot(sample()).await;
    let before = p.canvas().get(20, 2).unwrap().bg;
    p.hover("#tree").await; // center: y = 3
    let t = tree(&p);
    assert!(t.hover_line() >= 0);
    let y = t.hover_line();
    let after = p.canvas().get(20, y).unwrap().bg;
    assert_ne!(before, after);
}

#[tokio::test]
async fn expand_all_collapse_all_and_mutation() {
    let mut p = pilot(sample()).await;
    p.with::<Tree<u32>, _>("#tree", |t, ctx| {
        let root = t.root();
        t.collapse_all(root, ctx);
    })
    .await;
    assert_eq!(lines(&mut p), vec!["▶ Root"]);
    p.with::<Tree<u32>, _>("#tree", |t, ctx| {
        let root = t.root();
        t.expand_all(root, ctx);
    })
    .await;
    assert_eq!(tree(&p).line_count(), 6);
    assert!(log(&p).contains(&"expanded Carrot".to_string()));
    p.with::<Tree<u32>, _>("#tree", |t, ctx| {
        assert!(t.remove(TreeNodeId(4)));
        assert!(!t.remove(t.root()));
        let root = t.root();
        let n = t.add_leaf(root, "New", Some(9));
        t.select_node(Some(n), ctx);
    })
    .await;
    let l = lines(&mut p);
    assert_eq!(l[4], "└── New", "{l:?}");
    assert_eq!(tree(&p).cursor_line(), 4);
    assert_eq!(log(&p).last().unwrap(), "selected New Some(9)");
    p.with::<Tree<u32>, _>("#tree", |t, _ctx| t.clear()).await;
    assert_eq!(tree(&p).line_count(), 1);
}

#[tokio::test]
async fn scrolls_to_cursor() {
    let mut t: Tree<u32> = Tree::new("Root");
    let root = t.root();
    t.set_expanded(root, true);
    for i in 0..30 {
        t.add_leaf(root, format!("item {i}"), Some(i));
    }
    let mut p = pilot(t).await;
    p.press(&["end"]).await;
    let text = p.screen_text();
    assert!(text.contains("item 29"), "{text}");
    assert!(!text.contains("item 0\n"), "{text}");
    p.press(&["home"]).await;
    assert!(p.screen_text().contains("Root"));
}

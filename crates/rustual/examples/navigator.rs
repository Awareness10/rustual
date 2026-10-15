//! Navigation & structure widgets: a `TabbedContent` with a `Tree`, `Collapsible`s and a `Select`.
//! Run with `cargo run -p rustual --example navigator`. ctrl+q quits.

use rustual::prelude::*;
use rustual::widgets::{select, tabbed_content, tree};

const CSS: &str = "
TabbedContent {
    height: 1fr;
}
TabbedContent ContentSwitcher {
    height: 1fr;
}
TabPane {
    height: 1fr;
    padding: 1 2;
}
#files {
    height: 1fr;
}
#status {
    dock: bottom;
    height: 1;
    background: $panel;
    padding: 0 1;
}
#colour {
    width: 30;
}
";

/// A tiny nested data set for the tree: (name, children).
struct Entry {
    name: &'static str,
    children: Vec<Entry>,
}

fn e(name: &'static str, children: Vec<Entry>) -> Entry {
    Entry { name, children }
}

fn data() -> Vec<Entry> {
    vec![
        e(
            "src",
            vec![
                e("app.rs", vec![]),
                e("widgets", vec![e("tree.rs", vec![]), e("tabs.rs", vec![])]),
            ],
        ),
        e("docs", vec![e("guide.md", vec![])]),
        e("Cargo.toml", vec![]),
    ]
}

fn fill(tree: &mut Tree<String>, parent: TreeNodeId, entries: Vec<Entry>, path: &str) {
    for entry in entries {
        let full = format!("{path}/{}", entry.name);
        if entry.children.is_empty() {
            tree.add_leaf(parent, entry.name, Some(full));
        } else {
            let node = tree.add(parent, format!("[b]{}[/]", entry.name), Some(full.clone()));
            fill(tree, node, entry.children, &full);
        }
    }
}

struct Navigator;

impl Widget for Navigator {
    fn compose(&mut self) -> Vec<Child> {
        let mut files: Tree<String> = Tree::new("project");
        let root = files.root();
        fill(&mut files, root, data(), "");
        files.set_expanded(root, true);

        compose![
            TabbedContent::new().id("tabs").with(compose![
                TabPane::new("Files")
                    .id("files-pane")
                    .with(compose![files.id("files")]),
                TabPane::new("Sections").id("sections").with(compose![
                    Collapsible::new("Introduction")
                        .collapsed(false)
                        .with(compose![Label::new("rustual re-creates Textual in Rust.")]),
                    Collapsible::new("Widgets").with(compose![Label::new(
                        "Tabs, TabbedContent, Tree, Collapsible, Select"
                    )]),
                    Collapsible::new("Nested")
                        .with(compose![Collapsible::new("Inner")
                            .with(compose![Label::new("Collapsibles nest.")])]),
                ]),
                TabPane::new("Settings").id("settings").with(compose![
                    Label::new("Favourite colour:"),
                    Select::new([("Red", "red"), ("Green", "green"), ("Blue", "blue")])
                        .prompt("Pick a colour")
                        .id("colour"),
                ]),
            ]),
            Label::new("Ready").id("status"),
        ]
    }

    fn on_message(&mut self, msg: &Message, ctx: &mut Ctx) -> Flow {
        let status = if let Some(a) = msg.get::<tabbed_content::TabActivated>() {
            Some(format!("Tab: {}", a.pane_id))
        } else if let Some(s) = msg.get::<tree::NodeSelected>() {
            let path = ctx
                .get::<Tree<String>>(msg.sender)
                .and_then(|t| t.data(s.node).cloned())
                .unwrap_or_default();
            Some(format!("Selected {} ({path})", s.label))
        } else if let Some(c) = msg.get::<select::Changed<&'static str>>() {
            Some(format!("Colour: {}", c.value.unwrap_or("none")))
        } else {
            msg.get::<rustual::widgets::collapsible::Expanded>()
                .map(|c| format!("Expanded {:?}", c.collapsible_id))
        };
        if let Some(s) = status {
            if let Some(l) = ctx.query_one::<Label>("#status") {
                l.update(s);
            }
        }
        Flow::Continue
    }
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    App::new(Navigator).css(CSS).title("Navigator").run().await
}

//! `DirectoryTree` (port of `textual/widgets/_directory_tree.py`): a [`Tree`] of files and
//! folders that loads directory contents lazily when a folder is expanded.
//!
//! ```
//! # use rustual::prelude::*;
//! # let _ =
//! DirectoryTree::new("./")
//!     .filter_paths(|p| !p.file_name().is_some_and(|n| n.to_string_lossy().starts_with('.')))
//!     .id("files")
//! # ;
//! ```
//!
//! The tree API is available through `Deref` to `Tree<DirEntry>` (`cursor_node()`, `data(id)`,
//! `expand(id, ctx)`, ...). Selecting a file posts [`FileSelected`], selecting a folder posts
//! [`DirectorySelected`] (and toggles it). `NodeHighlighted` / `NodeCollapsed` bubble as for
//! `Tree`; `NodeSelected` and `NodeExpanded` are consumed like in Textual.
//!
//! Deviations from Textual: directories are read synchronously with `std::fs` on the UI thread
//! (Textual uses a thread worker); `filter_paths` is a per-path predicate closure instead of an
//! overridable method.

use crate::binding::Binding;
use crate::context::Ctx;
use crate::events::Event;
use crate::message::{Flow, Message};
use crate::widget::{Content, RenderCtx, Widget};
use crate::widgets::tree::{
    partial, LabelHook, NodeExpanded, NodeSelected, Tree, TreeNode, TreeNodeId,
};
use rustual_render::markup::escape;
use rustual_render::{Size, Text};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::OnceLock;

const DIRECTORY_TREE_CSS: &str = r#"
DirectoryTree {
    & > .directory-tree--folder {
        text-style: bold;
    }
    & > .directory-tree--extension {
        text-style: italic;
    }
    & > .directory-tree--hidden {
        text-style: dim;
    }
}

/* ANSI themes (Textual `:ansi`). */
DirectoryTree {
    &:ansi {
        & > .tree--guides {
           color: ansi_bright_black;
        }

        & > .directory-tree--folder {
            text-style: bold;
        }

        & > .directory-tree--extension {
            text-style: italic;
        }

        & > .directory-tree--hidden {
            color: ansi_default;
            text-style: dim;
        }
    }
}
"#;

/// Icon of a collapsed folder.
pub const ICON_NODE: &str = "📁 ";
/// Icon of an expanded folder.
pub const ICON_NODE_EXPANDED: &str = "📂 ";
/// Icon of a file.
pub const ICON_FILE: &str = "📄 ";

/// Node data: the entry's path and whether its contents were loaded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirEntry {
    pub path: PathBuf,
    pub loaded: bool,
}

impl DirEntry {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        DirEntry {
            path: path.into(),
            loaded: false,
        }
    }
}

/// Posted when a file is selected (Textual: `DirectoryTree.FileSelected`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileSelected {
    pub node: TreeNodeId,
    pub path: PathBuf,
}

/// Posted when a directory is selected (Textual: `DirectoryTree.DirectorySelected`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectorySelected {
    pub node: TreeNodeId,
    pub path: PathBuf,
}

type PathFilter = Rc<dyn Fn(&Path) -> bool>;

/// A tree view of a directory.
#[derive(Clone)]
pub struct DirectoryTree {
    tree: Tree<DirEntry>,
    path: PathBuf,
    filter: Option<PathFilter>,
}

impl std::fmt::Debug for DirectoryTree {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DirectoryTree")
            .field("path", &self.path)
            .field("tree", &self.tree)
            .finish_non_exhaustive()
    }
}

fn is_dir(path: &Path) -> bool {
    path.is_dir()
}

fn node_icon(n: &TreeNode<DirEntry>) -> &'static str {
    if !n.allow_expand() {
        ICON_FILE
    } else if n.is_expanded() {
        ICON_NODE_EXPANDED
    } else {
        ICON_NODE
    }
}

fn node_label(n: &TreeNode<DirEntry>, ctx: &RenderCtx) -> Text {
    let name = n.label_text();
    let base = ctx.base_style();
    let comp = |class: &str| partial(ctx.component_style(class), &base);
    let mut text = Text::new();
    if n.allow_expand() {
        text.push(name.clone(), comp("directory-tree--folder"));
    } else {
        let file = comp("directory-tree--file");
        // Rich's `highlight_regex(r"\..+$")`: from the first dot after the first char.
        match name.char_indices().skip(1).find(|(_, c)| *c == '.') {
            Some((dot, _)) if dot + 1 < name.len() => {
                text.push(name[..dot].to_string(), file.clone());
                let ext = file.combine(&comp("directory-tree--extension"));
                text.push(name[dot..].to_string(), ext);
            }
            _ => {
                text.push(name.clone(), file);
            }
        }
    }
    if name.starts_with('.') {
        text.stylize_base(&comp("directory-tree--hidden"));
    }
    text
}

/// The root's label once loaded: the last path component, like Python's `Path.name`
/// (empty for `.`, `./` and `/`).
fn root_label(path: &Path) -> String {
    use std::path::Component;
    match path.components().next_back() {
        Some(Component::Normal(name)) => name.to_string_lossy().into_owned(),
        Some(Component::ParentDir) => "..".into(),
        _ => String::new(),
    }
}

impl DirectoryTree {
    /// A tree rooted at `path` (loaded when mounted).
    pub fn new(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let mut tree = Tree::with_root_data(
            escape(&path.to_string_lossy()),
            Some(DirEntry::new(path.clone())),
        );
        tree.label_hook = Some(LabelHook {
            icon: node_icon,
            label: node_label,
        });
        DirectoryTree {
            tree,
            path,
            filter: None,
        }
    }

    /// Only show paths for which `keep` returns true (Textual: override `filter_paths`).
    pub fn filter_paths(mut self, keep: impl Fn(&Path) -> bool + 'static) -> Self {
        self.filter = Some(Rc::new(keep));
        self
    }

    /// The root path.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The inner tree.
    pub fn tree(&self) -> &Tree<DirEntry> {
        &self.tree
    }

    pub fn tree_mut(&mut self) -> &mut Tree<DirEntry> {
        &mut self.tree
    }

    /// The path of a node.
    pub fn node_path(&self, id: TreeNodeId) -> Option<&Path> {
        self.tree.data(id).map(|d| d.path.as_path())
    }

    /// The path under the cursor.
    pub fn cursor_path(&self) -> Option<&Path> {
        self.tree.cursor_node().and_then(|n| self.node_path(n))
    }

    /// Change the root path and reload.
    pub fn set_path(&mut self, path: impl Into<PathBuf>, ctx: &mut Ctx) {
        self.path = path.into();
        let had_cursor = self.tree.cursor_node().is_some();
        self.tree.reset(
            escape(&self.path.to_string_lossy()),
            Some(DirEntry::new(self.path.clone())),
        );
        self.load_root();
        if had_cursor {
            self.tree.set_cursor_line(0, ctx);
        }
        ctx.scroll_to(0, 0);
    }

    /// The sorted, filtered contents of `dir` (folders first, then case-insensitive by name).
    fn directory_content(&self, dir: &Path) -> Vec<PathBuf> {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return Vec::new();
        };
        let mut paths: Vec<PathBuf> = entries
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| self.filter.as_ref().is_none_or(|f| f(p)))
            .collect();
        paths.sort_by_cached_key(|p| {
            (
                !is_dir(p),
                p.file_name()
                    .map(|n| n.to_string_lossy().to_lowercase())
                    .unwrap_or_default(),
            )
        });
        paths
    }

    /// Load (or re-load) the contents of a folder node. Returns false if it isn't a folder.
    fn load_node(&mut self, id: TreeNodeId) -> bool {
        let Some(path) = self.node_path(id).map(Path::to_path_buf) else {
            return false;
        };
        if !is_dir(&path) {
            return false;
        }
        let content = self.directory_content(&path);
        self.tree.remove_children(id);
        for p in content {
            let name = p
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| p.to_string_lossy().into_owned());
            let allow_expand = is_dir(&p);
            self.tree.insert(
                id,
                None,
                escape(&name),
                Some(DirEntry::new(p)),
                false,
                allow_expand,
            );
        }
        if let Some(d) = self.tree.data_mut(id) {
            d.loaded = true;
        }
        self.tree.set_expanded(id, true);
        true
    }

    fn load_root(&mut self) {
        let root = self.tree.root();
        // Textual relabels loaded nodes with `Path.name` (so `./` shows an empty label).
        self.tree.set_label(root, escape(&root_label(&self.path)));
        self.load_node(root);
    }

    /// Re-read the whole tree from disk, keeping expanded folders expanded and the cursor on
    /// the same path (or its closest surviving ancestor).
    pub fn reload(&mut self, ctx: &mut Ctx) {
        let root = self.tree.root();
        self.reload_node(root, ctx);
    }

    /// Re-read the subtree under `node` from disk, preserving expanded state and cursor.
    pub fn reload_node(&mut self, node: TreeNodeId, ctx: &mut Ctx) {
        // Remember expanded folders and the highlighted path.
        let mut open: HashSet<PathBuf> = HashSet::new();
        let mut stack = vec![node];
        while let Some(id) = stack.pop() {
            let Some(n) = self.tree.node(id) else {
                continue;
            };
            if n.allow_expand() && n.is_expanded() {
                if let Some(d) = n.data() {
                    open.insert(d.path.clone());
                }
                stack.extend(n.children().iter().copied());
            }
        }
        let highlighted = self.cursor_path().map(Path::to_path_buf);

        // Re-load, re-opening previously open folders.
        let mut reopen = vec![node];
        while let Some(id) = reopen.pop() {
            let Some(n) = self.tree.node(id) else {
                continue;
            };
            let Some(path) = n.data().map(|d| d.path.clone()) else {
                continue;
            };
            if n.allow_expand() && (id == node || open.contains(&path)) && self.load_node(id) {
                if let Some(n) = self.tree.node(id) {
                    reopen.extend(n.children().iter().copied());
                }
            }
        }

        // Restore the cursor to the same path or its closest ancestor.
        let Some(highlighted) = highlighted else {
            return;
        };
        let mut best: Option<(usize, TreeNodeId)> = None;
        let mut look = vec![node];
        while let Some(id) = look.pop() {
            let Some(n) = self.tree.node(id) else {
                continue;
            };
            let Some(p) = n.data().map(|d| d.path.clone()) else {
                continue;
            };
            if highlighted.starts_with(&p) {
                let depth = p.components().count();
                if best.is_none_or(|(d, _)| depth > d) {
                    best = Some((depth, id));
                }
                if n.is_expanded() {
                    look.extend(n.children().iter().copied());
                }
            }
        }
        if let Some((_, id)) = best {
            self.tree.move_cursor(Some(id), ctx);
        }
    }
}

impl std::ops::Deref for DirectoryTree {
    type Target = Tree<DirEntry>;
    fn deref(&self) -> &Self::Target {
        &self.tree
    }
}

impl std::ops::DerefMut for DirectoryTree {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.tree
    }
}

impl Widget for DirectoryTree {
    fn type_names(&self) -> &'static [&'static str] {
        &["DirectoryTree", "Tree"]
    }

    fn default_css(&self) -> &'static str {
        static CSS: OnceLock<String> = OnceLock::new();
        CSS.get_or_init(|| format!("{}{}", self.tree.default_css(), DIRECTORY_TREE_CSS))
    }

    fn can_focus(&self) -> bool {
        true
    }

    fn on_mount(&mut self, ctx: &mut Ctx) {
        self.load_root();
        // Puts the cursor on the first line, as Textual does.
        self.tree.on_mount(ctx);
    }

    fn bindings(&self) -> Vec<Binding> {
        self.tree.bindings()
    }

    fn run_action(&mut self, action: &str, args: &[String], ctx: &mut Ctx) -> bool {
        self.tree.run_action(action, args, ctx)
    }

    fn on_event(&mut self, event: &Event, ctx: &mut Ctx) -> Flow {
        self.tree.on_event(event, ctx)
    }

    fn on_message(&mut self, msg: &Message, ctx: &mut Ctx) -> Flow {
        if msg.sender != ctx.id() {
            return Flow::Continue;
        }
        if let Some(e) = msg.get::<NodeExpanded>() {
            let node = e.node;
            if let Some(entry) = self.tree.data(node).cloned() {
                if is_dir(&entry.path) {
                    if !entry.loaded {
                        self.load_node(node);
                    }
                } else {
                    ctx.post_message(FileSelected {
                        node,
                        path: entry.path,
                    });
                }
            }
            return Flow::Stop;
        }
        if let Some(s) = msg.get::<NodeSelected>() {
            let node = s.node;
            // Auto-expand (toggle) like Tree does.
            self.tree.on_message(msg, ctx);
            if let Some(entry) = self.tree.data(node).cloned() {
                if is_dir(&entry.path) {
                    ctx.post_message(DirectorySelected {
                        node,
                        path: entry.path,
                    });
                } else {
                    ctx.post_message(FileSelected {
                        node,
                        path: entry.path,
                    });
                }
            }
            return Flow::Stop;
        }
        self.tree.on_message(msg, ctx)
    }

    fn virtual_size(&self, ctx: &RenderCtx) -> Option<Size> {
        self.tree.virtual_size(ctx)
    }

    fn content_width(&self, ctx: &RenderCtx) -> Option<i32> {
        Widget::content_width(&self.tree, ctx)
    }

    fn content_height(&self, ctx: &RenderCtx, width: i32) -> Option<i32> {
        self.tree.content_height(ctx, width)
    }

    fn render(&self, ctx: &RenderCtx) -> Content {
        self.tree.render(ctx)
    }
}

#[cfg(test)]
mod tests {
    use super::root_label;
    use std::path::Path;

    #[test]
    fn root_label_follows_path_name() {
        assert_eq!(root_label(Path::new("./")), "");
        assert_eq!(root_label(Path::new(".")), "");
        assert_eq!(root_label(Path::new("/")), "");
        assert_eq!(root_label(Path::new("/tmp")), "tmp");
        assert_eq!(root_label(Path::new("../textual")), "textual");
        assert_eq!(root_label(Path::new("sub/dir/")), "dir");
        assert_eq!(root_label(Path::new("..")), "..");
    }
}

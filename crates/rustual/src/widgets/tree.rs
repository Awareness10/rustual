//! `Tree` (port of `textual/widgets/_tree.py`): a self-scrolling tree control.
//!
//! ```
//! # use rustual::prelude::*;
//! let mut tree: Tree<u32> = Tree::new("Root");
//! let root = tree.root();
//! tree.set_expanded(root, true);
//! let fruit = tree.add(root, "Fruit", None);
//! tree.add_leaf(fruit, "[red]Apple[/]", Some(1));
//! let children = compose![tree.id("tree")];
//! ```
//!
//! Nodes live inside the widget and are addressed by [`TreeNodeId`]. Messages are not generic:
//! they carry the node id and plain label; get the payload with
//! `ctx.get::<Tree<T>>(msg.sender).and_then(|t| t.data(node))`.
//!
//! Methods that would post messages in Textual (`expand`, `collapse`, `toggle`,
//! `expand_all`, `collapse_all`, `move_cursor`, `select_node`) take a `Ctx`. For building a
//! tree before it is mounted use [`Tree::set_expanded`] (no messages).
//!
//! Deviations from Textual:
//! - `left` / `right` collapse / expand the cursor node (or move to the parent / first child);
//!   Textual binds only the `shift+` variants by default.
//! - Guides cannot be hidden by making their color transparent; use [`Tree::show_guides`].
//! - The cursor stays on the same node across rebuilds when that node is still visible;
//!   otherwise it stays on the same line (no `NodeHighlighted` is posted for that move).

use crate::binding::Binding;
use crate::context::Ctx;
use crate::events::Event;
use crate::message::{Flow, Message};
use crate::widget::{Content, RenderCtx, Widget};
use rustual_render::{cells::cell_len, Region, Size, Span, Strip, Style, Text};

const DEFAULT_CSS: &str = r#"
Tree {
    width: 1fr;
    height: 1fr;
    background: $surface;
    color: $foreground;
    overflow: auto auto;

    & > .tree--label {}
    & > .tree--guides {
        color: $surface-lighten-2;
    }
    & > .tree--guides-hover {
        color: $surface-lighten-2;
    }
    & > .tree--guides-selected {
        color: $block-cursor-blurred-background;
    }
    & > .tree--cursor {
        text-style: $block-cursor-blurred-text-style;
        background: $block-cursor-blurred-background;
    }
    & > .tree--highlight {}
    & > .tree--highlight-line {
        background: $block-hover-background;
    }

    &:focus {
        background-tint: $foreground 5%;
        & > .tree--cursor {
            color: $block-cursor-foreground;
            background: $block-cursor-background;
            text-style: $block-cursor-text-style;
        }
        & > .tree--guides {
            color: $surface-lighten-3;
        }
        & > .tree--guides-hover {
            color: $surface-lighten-3;
        }
        & > .tree--guides-selected {
            color: $block-cursor-background;
        }
    }

    &:light {
        /* In light mode the guides are darker*/
        & > .tree--guides {
            color: $surface-darken-1;
        }
        & > .tree--guides-hover {
            color: $block-cursor-background;
        }
        & > .tree--guides-selected {
            color: $block-cursor-background;
        }
    }
}
"#;

/// Icon for an expandable (collapsed) node.
pub const ICON_NODE: &str = "▶ ";
/// Icon for an expanded node.
pub const ICON_NODE_EXPANDED: &str = "▼ ";

/// Guide characters: (space, vertical, terminator, cross).
const LINES_DEFAULT: [&str; 4] = ["  ", "│ ", "└─", "├─"];
const LINES_BOLD: [&str; 4] = ["  ", "┃ ", "┗━", "┣━"];

/// Identifies a node within one [`Tree`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TreeNodeId(pub usize);

// ---------------------------------------------------------------- messages

/// Posted when a node is selected (enter / click) (Textual: `Tree.NodeSelected`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeSelected {
    pub node: TreeNodeId,
    /// Plain-text label.
    pub label: String,
}

/// Posted when a node is expanded (Textual: `Tree.NodeExpanded`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeExpanded {
    pub node: TreeNodeId,
    pub label: String,
}

/// Posted when a node is collapsed (Textual: `Tree.NodeCollapsed`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeCollapsed {
    pub node: TreeNodeId,
    pub label: String,
}

/// Posted when the cursor moves to a different node (Textual: `Tree.NodeHighlighted`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeHighlighted {
    pub node: TreeNodeId,
    pub label: String,
}

// ---------------------------------------------------------------- nodes

/// A node of a [`Tree`] (read-only view; mutate through the tree).
#[derive(Debug, Clone)]
pub struct TreeNode<T> {
    id: TreeNodeId,
    parent: Option<TreeNodeId>,
    children: Vec<TreeNodeId>,
    label: String,
    data: Option<T>,
    expanded: bool,
    allow_expand: bool,
    line: i32,
}

impl<T> TreeNode<T> {
    pub fn id(&self) -> TreeNodeId {
        self.id
    }
    pub fn parent(&self) -> Option<TreeNodeId> {
        self.parent
    }
    pub fn children(&self) -> &[TreeNodeId] {
        &self.children
    }
    /// The label (markup).
    pub fn label(&self) -> &str {
        &self.label
    }
    /// The label without markup.
    pub fn label_text(&self) -> String {
        Text::from_markup(&self.label).plain_text()
    }
    pub fn data(&self) -> Option<&T> {
        self.data.as_ref()
    }
    pub fn is_expanded(&self) -> bool {
        self.expanded
    }
    pub fn is_collapsed(&self) -> bool {
        !self.expanded
    }
    pub fn allow_expand(&self) -> bool {
        self.allow_expand
    }
    /// The line this node is displayed on, or -1.
    pub fn line(&self) -> i32 {
        self.line
    }
}

#[derive(Debug, Clone)]
struct TreeLine {
    path: Vec<TreeNodeId>,
    last: bool,
}

impl TreeLine {
    fn node(&self) -> TreeNodeId {
        *self.path.last().expect("non-empty path")
    }

    fn guide_width(&self, guide_depth: i32) -> i32 {
        (self.path.len() as i32 - 1).max(0) * guide_depth
    }
}

/// Only the attributes of `style` that differ from `base` (Textual's `partial=True`).
pub(crate) fn partial(style: Style, base: &Style) -> Style {
    let mut s = style;
    if s.fg == base.fg {
        s.fg = None;
    }
    if s.bg == base.bg {
        s.bg = None;
    }
    macro_rules! attr {
        ($($f:ident),*) => { $( if s.$f == base.$f { s.$f = None; } )* };
    }
    attr!(bold, dim, italic, underline, strike, reverse, blink);
    s
}

// ---------------------------------------------------------------- Tree

/// A tree control with expandable nodes, guide lines and a cursor.
///
/// Keys: `up`/`down` move the cursor, `left`/`right` collapse/expand, `enter` selects
/// (and toggles, with `auto_expand`), `space` toggles, `shift+space` expands/collapses all
/// siblings, `shift+left` parent, `shift+right` parent's next sibling, `shift+up`/`shift+down`
/// previous/next sibling, `pageup`/`pagedown`/`home`/`end`.
#[derive(Debug, Clone)]
pub struct Tree<T: 'static> {
    nodes: Vec<Option<TreeNode<T>>>,
    root: TreeNodeId,
    lines: Vec<TreeLine>,
    show_root: bool,
    show_guides: bool,
    guide_depth: i32,
    auto_expand: bool,
    cursor_line: i32,
    cursor_node: Option<TreeNodeId>,
    hover_line: i32,
    /// Custom icon / label rendering (Textual's overridable `render_label`), used by `DirectoryTree`.
    pub(crate) label_hook: Option<LabelHook<T>>,
}

/// Overrides how a node's icon and label are rendered (crate-internal "subclassing" hook).
pub(crate) struct LabelHook<T: 'static> {
    /// The icon drawn before the label (replaces `▶ ` / `▼ `; also used for leaves).
    pub icon: fn(&TreeNode<T>) -> &'static str,
    /// The styled label (cursor / hover styles are applied on top).
    pub label: fn(&TreeNode<T>, &RenderCtx) -> Text,
}

impl<T> Clone for LabelHook<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for LabelHook<T> {}

impl<T> std::fmt::Debug for LabelHook<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("LabelHook")
    }
}

impl<T: 'static> Tree<T> {
    /// A tree whose (collapsed) root node has the given (markup) label.
    pub fn new(label: impl Into<String>) -> Self {
        Self::with_root_data(label, None)
    }

    /// A tree with data for the root node.
    pub fn with_root_data(label: impl Into<String>, data: Option<T>) -> Self {
        let mut t = Tree {
            nodes: Vec::new(),
            root: TreeNodeId(0),
            lines: Vec::new(),
            show_root: true,
            show_guides: true,
            guide_depth: 4,
            auto_expand: true,
            cursor_line: -1,
            cursor_node: None,
            hover_line: -1,
            label_hook: None,
        };
        t.root = t.new_node(None, label.into(), data);
        t.build();
        t
    }

    // ------------------------------------------------------------ options

    /// Show the root node (default true).
    pub fn show_root(mut self, show: bool) -> Self {
        self.show_root = show;
        self.build();
        self
    }

    /// Show guide lines (default true).
    pub fn show_guides(mut self, show: bool) -> Self {
        self.show_guides = show;
        self
    }

    /// Indent per level in cells, clamped to 2..=10 (default 4).
    pub fn guide_depth(mut self, depth: i32) -> Self {
        self.guide_depth = depth.clamp(2, 10);
        self.build();
        self
    }

    /// Toggle nodes when they are selected (default true).
    pub fn auto_expand(mut self, auto: bool) -> Self {
        self.auto_expand = auto;
        self
    }

    pub fn set_show_root(&mut self, show: bool) {
        self.show_root = show;
        self.cursor_line = -1;
        self.cursor_node = None;
        self.build();
    }

    pub fn set_show_guides(&mut self, show: bool) {
        self.show_guides = show;
    }

    pub fn set_guide_depth(&mut self, depth: i32) {
        self.guide_depth = depth.clamp(2, 10);
        self.build();
    }

    pub fn set_auto_expand(&mut self, auto: bool) {
        self.auto_expand = auto;
    }

    pub fn is_show_root(&self) -> bool {
        self.show_root
    }

    pub fn is_show_guides(&self) -> bool {
        self.show_guides
    }

    pub fn get_guide_depth(&self) -> i32 {
        self.guide_depth
    }

    pub fn is_auto_expand(&self) -> bool {
        self.auto_expand
    }

    // ------------------------------------------------------------ nodes

    fn new_node(
        &mut self,
        parent: Option<TreeNodeId>,
        label: String,
        data: Option<T>,
    ) -> TreeNodeId {
        let id = TreeNodeId(self.nodes.len());
        // Labels are a single line (Textual's `process_label`).
        let label = label.lines().next().unwrap_or("").to_string();
        self.nodes.push(Some(TreeNode {
            id,
            parent,
            children: Vec::new(),
            label,
            data,
            expanded: false,
            allow_expand: true,
            line: -1,
        }));
        id
    }

    /// The root node's id.
    pub fn root(&self) -> TreeNodeId {
        self.root
    }

    pub fn node(&self, id: TreeNodeId) -> Option<&TreeNode<T>> {
        self.nodes.get(id.0).and_then(Option::as_ref)
    }

    fn node_mut(&mut self, id: TreeNodeId) -> Option<&mut TreeNode<T>> {
        self.nodes.get_mut(id.0).and_then(Option::as_mut)
    }

    /// The data of a node.
    pub fn data(&self, id: TreeNodeId) -> Option<&T> {
        self.node(id).and_then(|n| n.data.as_ref())
    }

    pub fn data_mut(&mut self, id: TreeNodeId) -> Option<&mut T> {
        self.node_mut(id).and_then(|n| n.data.as_mut())
    }

    pub fn set_data(&mut self, id: TreeNodeId, data: Option<T>) {
        if let Some(n) = self.node_mut(id) {
            n.data = data;
        }
    }

    /// The (markup) label of a node.
    pub fn label(&self, id: TreeNodeId) -> Option<&str> {
        self.node(id).map(|n| n.label.as_str())
    }

    pub fn set_label(&mut self, id: TreeNodeId, label: impl Into<String>) {
        let label: String = label.into();
        if let Some(n) = self.node_mut(id) {
            n.label = label.lines().next().unwrap_or("").to_string();
        }
        self.build();
    }

    /// Allow / prevent the user from expanding a node.
    pub fn set_allow_expand(&mut self, id: TreeNodeId, allow: bool) {
        if let Some(n) = self.node_mut(id) {
            n.allow_expand = allow;
        }
        self.build();
    }

    /// Add a (collapsed, expandable) child node at the end of `parent`'s children.
    pub fn add(
        &mut self,
        parent: TreeNodeId,
        label: impl Into<String>,
        data: Option<T>,
    ) -> TreeNodeId {
        self.insert(parent, None, label, data, false, true)
    }

    /// Add a leaf (non-expandable) node.
    pub fn add_leaf(
        &mut self,
        parent: TreeNodeId,
        label: impl Into<String>,
        data: Option<T>,
    ) -> TreeNodeId {
        self.insert(parent, None, label, data, false, false)
    }

    /// Add a node at `index` among `parent`'s children (`None` = at the end).
    pub fn insert(
        &mut self,
        parent: TreeNodeId,
        index: Option<usize>,
        label: impl Into<String>,
        data: Option<T>,
        expand: bool,
        allow_expand: bool,
    ) -> TreeNodeId {
        let id = self.new_node(Some(parent), label.into(), data);
        if let Some(n) = self.node_mut(id) {
            n.expanded = expand;
            n.allow_expand = allow_expand;
        }
        if let Some(p) = self.node_mut(parent) {
            let i = index.unwrap_or(p.children.len()).min(p.children.len());
            p.children.insert(i, id);
        }
        self.build();
        id
    }

    /// Remove a node and its descendants (the root cannot be removed). Returns false if not removed.
    pub fn remove(&mut self, id: TreeNodeId) -> bool {
        if id == self.root || self.node(id).is_none() {
            return false;
        }
        self.remove_subtree(id);
        self.build();
        true
    }

    /// Remove all children of a node.
    pub fn remove_children(&mut self, id: TreeNodeId) {
        let children = self
            .node(id)
            .map(|n| n.children.clone())
            .unwrap_or_default();
        for c in children {
            self.remove_subtree(c);
        }
        self.build();
    }

    fn remove_subtree(&mut self, id: TreeNodeId) {
        let children = self
            .node(id)
            .map(|n| n.children.clone())
            .unwrap_or_default();
        for c in children {
            self.remove_subtree(c);
        }
        if let Some(parent) = self.node(id).and_then(|n| n.parent) {
            if let Some(p) = self.node_mut(parent) {
                p.children.retain(|c| *c != id);
            }
        }
        if self.cursor_node == Some(id) {
            self.cursor_node = None;
        }
        if let Some(slot) = self.nodes.get_mut(id.0) {
            *slot = None;
        }
    }

    /// Remove every node under the root.
    pub fn clear(&mut self) {
        let root = self.nodes.get_mut(self.root.0).and_then(Option::take);
        self.nodes.clear();
        self.cursor_line = -1;
        self.cursor_node = None;
        self.hover_line = -1;
        if let Some(r) = root {
            self.root = self.new_node(None, r.label, r.data);
            if let Some(n) = self.node_mut(self.root) {
                n.expanded = r.expanded;
            }
        }
        self.build();
    }

    /// Clear the tree and give the root a new label and data.
    pub fn reset(&mut self, label: impl Into<String>, data: Option<T>) {
        self.clear();
        let root = self.root;
        if let Some(n) = self.node_mut(root) {
            n.label = label.into();
            n.data = data;
        }
        self.build();
    }

    /// Next sibling of a node.
    pub fn next_sibling(&self, id: TreeNodeId) -> Option<TreeNodeId> {
        let siblings = self.siblings(id);
        let i = siblings.iter().position(|s| *s == id)?;
        siblings.get(i + 1).copied()
    }

    /// Previous sibling of a node.
    pub fn previous_sibling(&self, id: TreeNodeId) -> Option<TreeNodeId> {
        let siblings = self.siblings(id);
        let i = siblings.iter().position(|s| *s == id)?;
        i.checked_sub(1).and_then(|i| siblings.get(i)).copied()
    }

    /// Siblings of a node (including itself).
    pub fn siblings(&self, id: TreeNodeId) -> Vec<TreeNodeId> {
        match self.node(id).and_then(|n| n.parent) {
            Some(p) => self.node(p).map(|n| n.children.clone()).unwrap_or_default(),
            None => vec![id],
        }
    }

    fn is_last(&self, id: TreeNodeId) -> bool {
        match self
            .node(id)
            .and_then(|n| n.parent)
            .and_then(|p| self.node(p))
        {
            Some(p) => p.children.last() == Some(&id),
            None => true,
        }
    }

    // ------------------------------------------------------------ expand / collapse

    /// Set the expanded state without posting messages (for building a tree before mounting).
    pub fn set_expanded(&mut self, id: TreeNodeId, expanded: bool) {
        if let Some(n) = self.node_mut(id) {
            n.expanded = expanded;
        }
        self.build();
    }

    fn post_node<M: std::any::Any + Send>(
        &self,
        id: TreeNodeId,
        ctx: &mut Ctx,
        make: impl FnOnce(TreeNodeId, String) -> M,
    ) {
        let label = self.node(id).map(|n| n.label_text()).unwrap_or_default();
        ctx.post_message(make(id, label));
    }

    fn expand_inner(&mut self, id: TreeNodeId, all: bool, ctx: &mut Ctx) {
        let Some(n) = self.node_mut(id) else { return };
        n.expanded = true;
        let children = n.children.clone();
        self.post_node(id, ctx, |node, label| NodeExpanded { node, label });
        if all {
            for c in children {
                self.expand_inner(c, true, ctx);
            }
        }
    }

    fn collapse_inner(&mut self, id: TreeNodeId, all: bool, ctx: &mut Ctx) {
        let Some(n) = self.node_mut(id) else { return };
        n.expanded = false;
        let children = n.children.clone();
        self.post_node(id, ctx, |node, label| NodeCollapsed { node, label });
        if all {
            for c in children {
                self.collapse_inner(c, true, ctx);
            }
        }
    }

    /// Expand a node (posts [`NodeExpanded`]).
    pub fn expand(&mut self, id: TreeNodeId, ctx: &mut Ctx) {
        self.expand_inner(id, false, ctx);
        self.build();
    }

    /// Expand a node and all its descendants.
    pub fn expand_all(&mut self, id: TreeNodeId, ctx: &mut Ctx) {
        self.expand_inner(id, true, ctx);
        self.build();
    }

    /// Collapse a node (posts [`NodeCollapsed`]).
    pub fn collapse(&mut self, id: TreeNodeId, ctx: &mut Ctx) {
        self.collapse_inner(id, false, ctx);
        self.build();
    }

    /// Collapse a node and all its descendants.
    pub fn collapse_all(&mut self, id: TreeNodeId, ctx: &mut Ctx) {
        self.collapse_inner(id, true, ctx);
        self.build();
    }

    /// Toggle a node's expanded state.
    pub fn toggle(&mut self, id: TreeNodeId, ctx: &mut Ctx) {
        match self.node(id).map(|n| n.expanded) {
            Some(true) => self.collapse(id, ctx),
            Some(false) => self.expand(id, ctx),
            None => {}
        }
    }

    /// Toggle a node and all its descendants.
    pub fn toggle_all(&mut self, id: TreeNodeId, ctx: &mut Ctx) {
        match self.node(id).map(|n| n.expanded) {
            Some(true) => self.collapse_all(id, ctx),
            Some(false) => self.expand_all(id, ctx),
            None => {}
        }
    }

    /// Toggle unless the node doesn't allow expansion (Textual's `_toggle_node`).
    fn toggle_node(&mut self, id: TreeNodeId, ctx: &mut Ctx) {
        if self.node(id).map(|n| n.allow_expand).unwrap_or(false) {
            self.toggle(id, ctx);
        }
    }

    // ------------------------------------------------------------ lines

    fn build(&mut self) {
        let mut lines = Vec::new();
        for n in self.nodes.iter_mut().flatten() {
            n.line = -1;
        }
        if self.show_root {
            self.add_lines(&mut lines, Vec::new(), self.root, true);
        } else {
            let children = self
                .node(self.root)
                .map(|n| n.children.clone())
                .unwrap_or_default();
            for c in children {
                self.add_lines(&mut lines, Vec::new(), c, true);
            }
        }
        self.lines = lines;
        if self.cursor_line != -1 {
            let visible = self
                .cursor_node
                .and_then(|c| self.node(c))
                .map(|n| n.line)
                .filter(|l| *l >= 0);
            match visible {
                Some(l) => self.cursor_line = l,
                None => {
                    if self.lines.is_empty() {
                        self.cursor_line = -1;
                    } else {
                        self.cursor_line = self.cursor_line.min(self.last_line());
                    }
                    self.cursor_node = self.node_at_line(self.cursor_line);
                }
            }
        }
        if self.hover_line >= self.lines.len() as i32 {
            self.hover_line = -1;
        }
    }

    fn add_lines(
        &mut self,
        lines: &mut Vec<TreeLine>,
        path: Vec<TreeNodeId>,
        id: TreeNodeId,
        last: bool,
    ) {
        let mut child_path = path;
        child_path.push(id);
        let line_no = lines.len() as i32;
        let Some(node) = self.node_mut(id) else {
            return;
        };
        node.line = line_no;
        let expanded = node.expanded;
        let children = node.children.clone();
        lines.push(TreeLine {
            path: child_path.clone(),
            last,
        });
        if expanded {
            let n = children.len();
            for (i, c) in children.into_iter().enumerate() {
                self.add_lines(lines, child_path.clone(), c, i + 1 == n);
            }
        }
    }

    /// Number of displayed lines.
    pub fn line_count(&self) -> usize {
        self.lines.len()
    }

    /// Index of the last line (-1 if empty).
    pub fn last_line(&self) -> i32 {
        self.lines.len() as i32 - 1
    }

    /// The node displayed at `line`.
    pub fn node_at_line(&self, line: i32) -> Option<TreeNodeId> {
        if line < 0 {
            return None;
        }
        self.lines.get(line as usize).map(TreeLine::node)
    }

    fn label_width(&self, id: TreeNodeId) -> i32 {
        let Some(n) = self.node(id) else { return 0 };
        self.icon_width(n) + cell_len(&n.label_text())
    }

    fn icon_width(&self, n: &TreeNode<T>) -> i32 {
        match &self.label_hook {
            Some(hook) => cell_len((hook.icon)(n)),
            None if n.allow_expand => cell_len(ICON_NODE),
            None => 0,
        }
    }

    fn label_region(&self, line: i32) -> Option<Region> {
        let tl = self.lines.get(usize::try_from(line).ok()?)?;
        let x = tl.guide_width(self.guide_depth);
        Some(Region::new(x, line, self.label_width(tl.node()), 1))
    }

    fn content_width(&self) -> i32 {
        self.lines
            .iter()
            .map(|l| self.label_width(l.node()) + l.guide_width(self.guide_depth))
            .max()
            .unwrap_or(0)
    }

    // ------------------------------------------------------------ cursor

    /// The line with the cursor, or -1.
    pub fn cursor_line(&self) -> i32 {
        self.cursor_line
    }

    /// The node under the cursor.
    pub fn cursor_node(&self) -> Option<TreeNodeId> {
        self.cursor_node
    }

    /// The line under the mouse, or -1.
    pub fn hover_line(&self) -> i32 {
        self.hover_line
    }

    /// Move the cursor to `line` (clamped). Posts [`NodeHighlighted`] if the node changed.
    pub fn set_cursor_line(&mut self, line: i32, ctx: &mut Ctx) {
        let line = if self.lines.is_empty() {
            -1
        } else {
            line.clamp(0, self.last_line())
        };
        let previous = self.cursor_node;
        self.cursor_line = line;
        self.cursor_node = self.node_at_line(line);
        if let Some(node) = self.cursor_node {
            if previous != Some(node) {
                self.post_node(node, ctx, |node, label| NodeHighlighted { node, label });
            }
        }
    }

    /// Hide and reset the cursor (no message).
    pub fn unselect(&mut self) {
        self.cursor_line = -1;
        self.cursor_node = None;
    }

    /// Move the cursor to `node` (or reset it with `None`) and scroll it into view.
    pub fn move_cursor(&mut self, node: Option<TreeNodeId>, ctx: &mut Ctx) {
        let line = node
            .and_then(|n| self.node(n))
            .map(|n| n.line)
            .unwrap_or(-1);
        self.set_cursor_line(line, ctx);
        if node.is_some() {
            self.scroll_to_line(self.cursor_line, ctx);
        }
    }

    /// Move the cursor to `node` and post [`NodeSelected`].
    pub fn select_node(&mut self, node: Option<TreeNodeId>, ctx: &mut Ctx) {
        self.move_cursor(node, ctx);
        if let Some(n) = node.filter(|n| self.node(*n).is_some()) {
            self.post_node(n, ctx, |node, label| NodeSelected { node, label });
        }
    }

    /// Scroll so `line` is visible (vertically only).
    pub fn scroll_to_line(&self, line: i32, ctx: &mut Ctx) {
        if let Some(r) = self.label_region(line) {
            let x = ctx.scroll_offset().x;
            ctx.scroll_to_show(Region::new(x, r.y, 1, 1));
        }
    }

    /// Scroll so `node` is visible.
    pub fn scroll_to_node(&self, node: TreeNodeId, ctx: &mut Ctx) {
        if let Some(l) = self.node(node).map(|n| n.line).filter(|l| *l >= 0) {
            self.scroll_to_line(l, ctx);
        }
    }

    // ------------------------------------------------------------ actions

    fn action(&mut self, action: &str, ctx: &mut Ctx) -> bool {
        let page = (ctx.size().height - 1).max(1);
        match action {
            "cursor_up" => {
                let l = if self.cursor_line == -1 {
                    self.last_line()
                } else {
                    self.cursor_line - 1
                };
                self.set_cursor_line(l, ctx);
                self.scroll_to_line(self.cursor_line, ctx);
            }
            "cursor_down" => {
                let l = if self.cursor_line == -1 {
                    0
                } else {
                    self.cursor_line + 1
                };
                self.set_cursor_line(l, ctx);
                self.scroll_to_line(self.cursor_line, ctx);
            }
            "page_down" => {
                let l = self.cursor_line.max(0) + page;
                self.set_cursor_line(l, ctx);
                self.scroll_to_line(self.cursor_line, ctx);
            }
            "page_up" => {
                let start = if self.cursor_line == -1 {
                    self.last_line()
                } else {
                    self.cursor_line
                };
                self.set_cursor_line(start - page, ctx);
                self.scroll_to_line(self.cursor_line, ctx);
            }
            "scroll_home" => {
                self.set_cursor_line(0, ctx);
                self.scroll_to_line(self.cursor_line, ctx);
            }
            "scroll_end" => {
                self.set_cursor_line(self.last_line(), ctx);
                self.scroll_to_line(self.cursor_line, ctx);
            }
            "toggle_node" => {
                if let Some(n) = self.node_at_line(self.cursor_line) {
                    self.toggle_node(n, ctx);
                }
            }
            "select_cursor" => {
                if let Some(n) = self.node_at_line(self.cursor_line) {
                    self.post_node(n, ctx, |node, label| NodeSelected { node, label });
                }
            }
            "cursor_parent" => {
                if let Some(p) = self
                    .cursor_node
                    .and_then(|c| self.node(c))
                    .and_then(|n| n.parent)
                {
                    self.move_cursor(Some(p), ctx);
                }
            }
            "cursor_parent_next_sibling" => {
                if let Some(p) = self
                    .cursor_node
                    .and_then(|c| self.node(c))
                    .and_then(|n| n.parent)
                {
                    let next = self.next_sibling(p);
                    self.move_cursor(next, ctx);
                }
            }
            "cursor_previous_sibling" => {
                if let Some(c) = self.cursor_node {
                    let target = self
                        .previous_sibling(c)
                        .or_else(|| self.node(c).and_then(|n| n.parent));
                    self.move_cursor(target, ctx);
                }
            }
            "cursor_next_sibling" => {
                if let Some(c) = self.cursor_node {
                    match self.next_sibling(c) {
                        Some(n) => self.move_cursor(Some(n), ctx),
                        None => {
                            if let Some(p) = self.node(c).and_then(|n| n.parent) {
                                let next = self.next_sibling(p);
                                self.move_cursor(next, ctx);
                            }
                        }
                    }
                }
            }
            "toggle_expand_all" => {
                let Some(c) = self.cursor_node else {
                    return true;
                };
                if self.node(c).and_then(|n| n.parent).is_none() {
                    return true;
                }
                let siblings = self.siblings(c);
                let all_collapsed = siblings
                    .iter()
                    .all(|s| self.node(*s).map(|n| !n.expanded).unwrap_or(true));
                for s in siblings {
                    if self.node(s).map(|n| n.allow_expand).unwrap_or(false) {
                        if all_collapsed {
                            self.expand(s, ctx);
                        } else {
                            self.collapse(s, ctx);
                        }
                    }
                }
                self.move_cursor(Some(c), ctx);
            }
            "collapse_or_parent" => {
                let Some(c) = self.cursor_node else {
                    return true;
                };
                let (expanded, allow, parent) = match self.node(c) {
                    Some(n) => (n.expanded, n.allow_expand, n.parent),
                    None => return true,
                };
                if expanded && allow {
                    self.collapse(c, ctx);
                } else if let Some(p) = parent {
                    if p != self.root || self.show_root {
                        self.move_cursor(Some(p), ctx);
                    }
                }
            }
            "expand_or_child" => {
                let Some(c) = self.cursor_node else {
                    return true;
                };
                let (expanded, allow, first) = match self.node(c) {
                    Some(n) => (n.expanded, n.allow_expand, n.children.first().copied()),
                    None => return true,
                };
                if allow && !expanded {
                    self.expand(c, ctx);
                } else if let Some(f) = first.filter(|_| expanded) {
                    self.move_cursor(Some(f), ctx);
                }
            }
            _ => return false,
        }
        true
    }

    /// Content-relative (virtual) coordinates of a mouse event.
    fn local(&self, screen_x: i32, screen_y: i32, ctx: &Ctx) -> (i32, i32) {
        let r = ctx.content_region();
        let s = ctx.scroll_offset();
        (screen_x - r.x + s.x, screen_y - r.y + s.y)
    }

    // ------------------------------------------------------------ rendering

    fn render_line(&self, ctx: &RenderCtx, y: i32, width: i32, virtual_width: i32) -> Strip {
        let Some(line) = usize::try_from(y).ok().and_then(|y| self.lines.get(y)) else {
            return Strip::blank(width, Style::default());
        };
        let focused = ctx.is_focused();
        let hovered = self.node_at_line(self.hover_line);
        let is_hover = hovered.map(|h| line.path.contains(&h)).unwrap_or(false);

        let base = ctx.base_style();
        let base_guide = ctx.component_style("tree--guides");
        let guide_hover = base_guide.combine(&ctx.component_style("tree--guides-hover"));
        let guide_selected = base_guide.combine(&ctx.component_style("tree--guides-selected"));

        let line_style = if is_hover {
            partial(ctx.component_style("tree--highlight-line"), &base)
        } else {
            Style::default()
        };

        let guide_depth = (self.guide_depth - 2).max(0) as usize;
        let show_guides = self.show_guides;
        let guides_for = |style: &Style| -> [String; 4] {
            let chars = if !show_guides {
                ["  "; 4]
            } else if style.bold == Some(true) {
                LINES_BOLD
            } else {
                LINES_DEFAULT
            };
            chars.map(|c| {
                let mut it = c.chars();
                let a = it.next().unwrap_or(' ');
                let b = it.next().unwrap_or(' ');
                format!("{a}{} ", b.to_string().repeat(guide_depth))
            })
        };

        let mut strip = Strip::default();
        let mut hover = Some(line.path[0]) == hovered;
        let mut selected = Some(line.path[0]) == self.cursor_node && focused;
        let mut guide_style = base_guide.clone();
        let last_node = line.node();
        for node in &line.path[1..] {
            if hover {
                guide_style = guide_hover.clone();
            }
            if selected {
                guide_style = guide_selected.clone();
            }
            let g = guides_for(&guide_style);
            let guide = if self.is_last(*node) { &g[0] } else { &g[1] };
            if *node != last_node {
                strip.push(Span::new(guide.clone(), line_style.combine(&guide_style)));
            }
            hover = hover || Some(*node) == hovered;
            selected = (selected || Some(*node) == self.cursor_node) && focused;
        }
        if line.path.len() > 1 {
            let g = guides_for(&guide_style);
            let t = if line.last { &g[2] } else { &g[3] };
            strip.push(Span::new(t.clone(), line_style.combine(&guide_style)));
        }

        // Label
        let mut label_style = partial(ctx.component_style("tree--label"), &base);
        if self.hover_line == y {
            label_style =
                label_style.combine(&partial(ctx.component_style("tree--highlight"), &base));
        }
        if self.cursor_line == y {
            label_style = label_style.combine(&ctx.component_style("tree--cursor"));
        }
        if let Some(n) = self.node(last_node) {
            if let Some(hook) = &self.label_hook {
                strip.push(Span::new((hook.icon)(n), line_style.clone()));
            } else if n.allow_expand {
                let icon = if n.expanded {
                    ICON_NODE_EXPANDED
                } else {
                    ICON_NODE
                };
                strip.push(Span::new(icon, line_style.clone()));
            }
            let text = match &self.label_hook {
                Some(hook) => (hook.label)(n, ctx),
                None => ctx.markup(&n.label),
            };
            for span in text.spans {
                let style = line_style.combine(&span.style).combine(&label_style);
                strip.push(Span::new(span.text.replace('\n', " "), style));
            }
        }
        let pad_width = virtual_width.max(width);
        let len = strip.cell_len();
        if len < pad_width {
            strip.push(Span::new(
                " ".repeat((pad_width - len) as usize),
                line_style,
            ));
        }
        strip
    }
}

impl<T: 'static> Widget for Tree<T> {
    fn type_names(&self) -> &'static [&'static str] {
        &["Tree"]
    }

    fn on_mount(&mut self, ctx: &mut Ctx) {
        // Like Textual (`watch_show_root` → `cursor_line = -1`, clamped to 0): the cursor starts
        // on the first line, posting `NodeHighlighted` for it.
        if self.cursor_line == -1 {
            self.build();
            self.set_cursor_line(0, ctx);
        }
    }

    fn default_css(&self) -> &'static str {
        DEFAULT_CSS
    }

    fn can_focus(&self) -> bool {
        true
    }

    fn help(&self) -> Option<&str> {
        Some(
            "\
## Tree

- **up** / **down** move the cursor, **left** / **right** collapse and expand.
- **enter** selects a node, **space** toggles it.
- **shift+left** jumps to the parent node.",
        )
    }

    fn bindings(&self) -> Vec<Binding> {
        vec![
            Binding::new("shift+left", "cursor_parent", "Cursor to parent").hidden(),
            Binding::new(
                "shift+right",
                "cursor_parent_next_sibling",
                "Cursor to next ancestor",
            )
            .hidden(),
            Binding::new(
                "shift+up",
                "cursor_previous_sibling",
                "Cursor to previous sibling",
            )
            .hidden(),
            Binding::new(
                "shift+down",
                "cursor_next_sibling",
                "Cursor to next sibling",
            )
            .hidden(),
            Binding::new("enter", "select_cursor", "Select").hidden(),
            Binding::new("space", "toggle_node", "Toggle").hidden(),
            Binding::new("shift+space", "toggle_expand_all", "Expand or collapse all").hidden(),
            Binding::new("up", "cursor_up", "Cursor Up").hidden(),
            Binding::new("down", "cursor_down", "Cursor Down").hidden(),
            Binding::new("left", "collapse_or_parent", "Collapse").hidden(),
            Binding::new("right", "expand_or_child", "Expand").hidden(),
            Binding::new("pageup", "page_up", "Page Up").hidden(),
            Binding::new("pagedown", "page_down", "Page Down").hidden(),
            Binding::new("home", "scroll_home", "Home").hidden(),
            Binding::new("end", "scroll_end", "End").hidden(),
        ]
    }

    fn run_action(&mut self, action: &str, _args: &[String], ctx: &mut Ctx) -> bool {
        self.action(action, ctx)
    }

    fn on_event(&mut self, event: &Event, ctx: &mut Ctx) -> Flow {
        match event {
            Event::MouseMove(m) => {
                let (_, y) = self.local(m.screen_x, m.screen_y, ctx);
                self.hover_line = if y >= 0 && y < self.lines.len() as i32 {
                    y
                } else {
                    -1
                };
                Flow::Continue
            }
            Event::Leave => {
                self.hover_line = -1;
                Flow::Continue
            }
            Event::Click(m) => {
                let (x, y) = self.local(m.screen_x, m.screen_y, ctx);
                let Some(node) = self.node_at_line(y) else {
                    return Flow::Stop;
                };
                let line = &self.lines[y as usize];
                let gx = line.guide_width(self.guide_depth);
                let allow = self.node(node).map(|n| n.allow_expand).unwrap_or(false);
                let icon_width = self.node(node).map(|n| self.icon_width(n)).unwrap_or(0);
                if allow && x >= gx && x < gx + icon_width {
                    self.toggle_node(node, ctx);
                } else {
                    self.set_cursor_line(y, ctx);
                    self.action("select_cursor", ctx);
                }
                Flow::Stop
            }
            _ => Flow::Continue,
        }
    }

    fn on_message(&mut self, msg: &Message, ctx: &mut Ctx) -> Flow {
        if msg.sender == ctx.id() {
            if let Some(s) = msg.get::<NodeSelected>() {
                if self.auto_expand {
                    self.toggle_node(s.node, ctx);
                }
            }
        }
        Flow::Continue
    }

    fn virtual_size(&self, ctx: &RenderCtx) -> Option<Size> {
        let width = if self.lines.is_empty() {
            ctx.size.width
        } else {
            self.content_width()
        };
        Some(Size::new(width, self.lines.len() as i32))
    }

    fn content_width(&self, _ctx: &RenderCtx) -> Option<i32> {
        Some(self.content_width())
    }

    fn content_height(&self, _ctx: &RenderCtx, _width: i32) -> Option<i32> {
        Some(self.lines.len() as i32)
    }

    fn render(&self, ctx: &RenderCtx) -> Content {
        let width = ctx.size.width;
        let virtual_width = self.content_width();
        let scroll = ctx.scroll_offset;
        let lines = (0..ctx.size.height)
            .map(|y| {
                self.render_line(ctx, y + scroll.y, width, virtual_width)
                    .crop(scroll.x, scroll.x + width)
            })
            .collect();
        Content::Lines(lines)
    }
}

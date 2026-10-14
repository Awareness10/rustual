//! `MarkdownViewer` and `MarkdownTableOfContents`.

use super::{
    sanitize_location, Highlighter, LinkClicked, Markdown, TableOfContentsSelected,
    TableOfContentsUpdated, TocEntry,
};
use crate::app::Severity;
use crate::context::Ctx;
use crate::dom::NodeId;
use crate::message::{Flow, Message};
use crate::widget::{Child, Widget, WidgetExt};
use crate::widgets::tree::{self, Tree};
use crate::widgets::VerticalScroll;
use std::path::{Path, PathBuf};
use textual_render::markup::escape;

/// Roman numerals used as TOC level markers (index = heading level).
const NUMERALS: [&str; 7] = [" ", "Ⅰ", "Ⅱ", "Ⅲ", "Ⅳ", "Ⅴ", "Ⅵ"];

// ---------------------------------------------------------------- navigator

/// A browser-like history of document paths.
#[derive(Debug, Clone, Default)]
pub struct Navigator {
    stack: Vec<PathBuf>,
    index: usize,
}

impl Navigator {
    pub fn new() -> Self {
        Self::default()
    }

    /// The current location (`.` when empty).
    pub fn location(&self) -> PathBuf {
        self.stack
            .get(self.index)
            .cloned()
            .unwrap_or_else(|| PathBuf::from("."))
    }

    /// Is the current location at the start of the history?
    pub fn start(&self) -> bool {
        self.index == 0
    }

    /// Is the current location at the end of the history?
    pub fn end(&self) -> bool {
        self.index + 1 >= self.stack.len()
    }

    /// Go to a new location (relative to the current document's directory). Returns it.
    pub fn go(&mut self, location: &str) -> PathBuf {
        let (path, anchor) = sanitize_location(location);
        let location = if path == Path::new(".") && !anchor.is_empty() {
            let (current, _) = sanitize_location(&self.location().to_string_lossy());
            format!("{}#{anchor}", current.to_string_lossy())
        } else {
            location.to_string()
        };
        let current = self.location();
        let base = current.parent().unwrap_or(Path::new(""));
        let mut new_path = base.join(location);
        if new_path.is_relative() {
            if let Ok(cwd) = std::env::current_dir() {
                new_path = cwd.join(new_path);
            }
        }
        if !self.stack.is_empty() {
            self.stack.truncate(self.index + 1);
        }
        self.stack.push(new_path.clone());
        self.index = self.stack.len() - 1;
        new_path
    }

    /// Go back. Returns true if the location changed.
    pub fn back(&mut self) -> bool {
        if self.index > 0 {
            self.index -= 1;
            true
        } else {
            false
        }
    }

    /// Go forward. Returns true if the location changed.
    pub fn forward(&mut self) -> bool {
        if self.index + 1 < self.stack.len() {
            self.index += 1;
            true
        } else {
            false
        }
    }
}

/// Posted by `MarkdownViewer` after navigating (Textual: `MarkdownViewer.NavigatorUpdated`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavigatorUpdated;

// ---------------------------------------------------------------- table of contents

/// Displays a table of contents in a [`Tree`]; selecting an entry posts
/// [`TableOfContentsSelected`].
#[derive(Debug, Default)]
pub struct MarkdownTableOfContents {
    toc: Vec<TocEntry>,
    node: Option<NodeId>,
}

impl MarkdownTableOfContents {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn table_of_contents(&self) -> &[TocEntry] {
        &self.toc
    }

    /// Replace the entries and rebuild the tree. `ctx` may belong to any widget.
    pub fn set_table_of_contents(&mut self, toc: Vec<TocEntry>, ctx: &mut Ctx) {
        self.toc = toc;
        let node = self.node.unwrap_or_else(|| ctx.id());
        let Some(tree_id) = ctx.query_in(node, "Tree").into_iter().next() else {
            return;
        };
        if let Some(tree) = ctx.get::<Tree<String>>(tree_id) {
            Self::rebuild(tree, &self.toc);
        }
    }

    fn rebuild(tree: &mut Tree<String>, toc: &[TocEntry]) {
        tree.clear();
        let root = tree.root();
        tree.set_expanded(root, true);
        for (level, name, block_id) in toc {
            let level = (*level).clamp(1, 6) as usize;
            let mut node = root;
            for _ in 0..level - 1 {
                let last = tree.node(node).and_then(|n| n.children().last().copied());
                match last {
                    Some(child) => {
                        node = child;
                        tree.set_allow_expand(node, true);
                        tree.set_expanded(node, true);
                    }
                    None => {
                        node = tree.insert(node, None, NUMERALS[level], None, true, true);
                    }
                }
            }
            let label = format!("[dim]{} [/dim]{}", NUMERALS[level], escape(name));
            tree.add_leaf(node, label, Some(block_id.clone()));
        }
    }
}

impl Widget for MarkdownTableOfContents {
    fn type_names(&self) -> &'static [&'static str] {
        &["MarkdownTableOfContents"]
    }

    fn default_css(&self) -> &'static str {
        "
MarkdownTableOfContents {
    width: auto;
    height: 1fr;
    background: $panel;
    &:focus-within {
        background-tint: $foreground 5%;
    }
}
MarkdownTableOfContents > Tree {
    padding: 1;
    width: auto;
    height: 1fr;
    background: $panel;
}
"
    }

    fn compose(&mut self) -> Vec<Child> {
        let mut tree: Tree<String> = Tree::new("TOC").show_root(false).auto_expand(false);
        Self::rebuild(&mut tree, &self.toc);
        vec![tree.child()]
    }

    fn on_mount(&mut self, ctx: &mut Ctx) {
        self.node = Some(ctx.id());
    }

    fn on_message(&mut self, msg: &Message, ctx: &mut Ctx) -> Flow {
        if let Some(sel) = msg.get::<tree::NodeSelected>() {
            let block_id = ctx
                .get::<Tree<String>>(msg.sender)
                .and_then(|t| t.data(sel.node).cloned());
            if let Some(block_id) = block_id {
                ctx.post_message(TableOfContentsSelected { block_id });
            }
            return Flow::Stop;
        }
        Flow::Continue
    }
}

// ---------------------------------------------------------------- viewer

/// A scrolling [`Markdown`] document with an optional table of contents sidebar and
/// browser-like navigation between local `.md` files.
///
/// Clicking a link: `#anchor` scrolls to the heading; a link with a URL scheme
/// (`https://`, `mailto:` ...) lets [`LinkClicked`] bubble to the app; anything else is loaded
/// as a file relative to the current document ([`MarkdownViewer::go`]).
///
/// Deviation from Textual: the viewer is a horizontal container (table of contents, then a
/// `VerticalScroll` holding the `Markdown`) rather than a `VerticalScroll` itself.
pub struct MarkdownViewer {
    markdown: Option<String>,
    show_toc: bool,
    navigator: Navigator,
    highlighter: Option<Highlighter>,
    node: Option<NodeId>,
}

impl MarkdownViewer {
    pub fn new(markdown: impl Into<String>) -> Self {
        MarkdownViewer {
            markdown: Some(markdown.into()),
            show_toc: true,
            navigator: Navigator::new(),
            highlighter: None,
            node: None,
        }
    }

    /// Show the table of contents sidebar (default true).
    pub fn show_table_of_contents(mut self, show: bool) -> Self {
        self.show_toc = show;
        self
    }

    /// Syntax highlighter for code fences.
    pub fn highlighter(mut self, h: Highlighter) -> Self {
        self.highlighter = Some(h);
        self
    }

    pub fn is_showing_table_of_contents(&self) -> bool {
        self.show_toc
    }

    pub fn set_show_table_of_contents(&mut self, show: bool, ctx: &mut Ctx) {
        self.show_toc = show;
        let node = self.node_id(ctx);
        ctx.set_class_on(node, show, "-show-table-of-contents");
    }

    pub fn navigator(&self) -> &Navigator {
        &self.navigator
    }

    fn node_id(&self, ctx: &Ctx) -> NodeId {
        self.node.unwrap_or_else(|| ctx.id())
    }

    /// The node of the `Markdown` document.
    pub fn document_id(&self, ctx: &Ctx) -> Option<NodeId> {
        let node = self.node_id(ctx);
        ctx.query_in(node, "Markdown").into_iter().next()
    }

    fn scroll_id(&self, ctx: &Ctx) -> Option<NodeId> {
        let node = self.node_id(ctx);
        ctx.query_in(node, "VerticalScroll").into_iter().next()
    }

    /// Run `f` on the `Markdown` document.
    pub fn with_document<R>(
        &self,
        ctx: &mut Ctx,
        f: impl FnOnce(&mut Markdown, &mut Ctx) -> R,
    ) -> Option<R> {
        let id = self.document_id(ctx)?;
        ctx.with_id::<Markdown, R>(id, f)
    }

    /// Replace the document text (no history entry).
    pub fn update(&mut self, markdown: impl Into<String>, ctx: &mut Ctx) {
        let text = markdown.into();
        self.with_document(ctx, |md, c| md.update(text, c));
    }

    fn load(&mut self, path: &Path, ctx: &mut Ctx) -> bool {
        let (file, anchor) = sanitize_location(&path.to_string_lossy());
        let result = self.with_document(ctx, |md, c| md.load(&file, c));
        match result {
            Some(Ok(())) => {
                if let Some(scroll) = self.scroll_id(ctx) {
                    ctx.scroll_node_to(scroll, 0, 0);
                }
                if !anchor.is_empty() {
                    self.with_document(ctx, |md, c| md.goto_anchor(&anchor, c));
                }
                true
            }
            Some(Err(e)) => {
                ctx.notify_with(
                    &format!("{}: {e}", file.display()),
                    "Unable to load document",
                    Severity::Error,
                    None,
                );
                false
            }
            None => false,
        }
    }

    /// Navigate to a document path (`file.md`, `file.md#anchor` or `#anchor`), relative to the
    /// current document. Returns false if the file could not be loaded.
    pub fn go(&mut self, location: &str, ctx: &mut Ctx) -> bool {
        let (path, anchor) = sanitize_location(location);
        if path == Path::new(".") && !anchor.is_empty() {
            return self
                .with_document(ctx, |md, c| md.goto_anchor(&anchor, c))
                .unwrap_or(false);
        }
        let target = self.navigator.go(location);
        let ok = self.load(&target, ctx);
        ctx.post_message(NavigatorUpdated);
        ok
    }

    /// Go back in the history.
    pub fn back(&mut self, ctx: &mut Ctx) -> bool {
        if self.navigator.back() {
            let loc = self.navigator.location();
            self.load(&loc, ctx);
            ctx.post_message(NavigatorUpdated);
            true
        } else {
            false
        }
    }

    /// Go forward in the history.
    pub fn forward(&mut self, ctx: &mut Ctx) -> bool {
        if self.navigator.forward() {
            let loc = self.navigator.location();
            self.load(&loc, ctx);
            ctx.post_message(NavigatorUpdated);
            true
        } else {
            false
        }
    }
}

/// Whether `href` has a URL scheme (`https:`, `mailto:`...) rather than being a file path.
fn is_url(href: &str) -> bool {
    match href.split_once(':') {
        Some((scheme, _)) => {
            scheme.len() > 1
                && scheme
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
        }
        None => false,
    }
}

impl Widget for MarkdownViewer {
    fn type_names(&self) -> &'static [&'static str] {
        &["MarkdownViewer"]
    }

    fn default_css(&self) -> &'static str {
        "
MarkdownViewer {
    height: 1fr;
    layout: horizontal;
    background: $surface;
    & > MarkdownTableOfContents {
        display: none;
    }
    & > VerticalScroll {
        scrollbar-gutter: stable;
        height: 1fr;
    }
}
MarkdownViewer.-show-table-of-contents > MarkdownTableOfContents {
    display: block;
}
"
    }

    fn can_focus_children(&self) -> bool {
        true
    }

    fn compose(&mut self) -> Vec<Child> {
        let mut md = Markdown::new(self.markdown.take().unwrap_or_default());
        md.set_highlighter(self.highlighter);
        vec![
            MarkdownTableOfContents::new().child(),
            VerticalScroll::new().with([md.child()]),
        ]
    }

    fn on_mount(&mut self, ctx: &mut Ctx) {
        self.node = Some(ctx.id());
        ctx.set_class(self.show_toc, "-show-table-of-contents");
    }

    fn on_message(&mut self, msg: &Message, ctx: &mut Ctx) -> Flow {
        if let Some(m) = msg.get::<TableOfContentsUpdated>() {
            let node = self.node_id(ctx);
            if let Some(toc) = ctx
                .query_in(node, "MarkdownTableOfContents")
                .into_iter()
                .next()
            {
                let entries = m.table_of_contents.clone();
                ctx.with_id::<MarkdownTableOfContents, _>(toc, |t, c| {
                    t.set_table_of_contents(entries, c)
                });
            }
            return Flow::Stop;
        }
        if let Some(m) = msg.get::<TableOfContentsSelected>() {
            let id = m.block_id.clone();
            self.with_document(ctx, |md, c| md.scroll_to_block(&id, c));
            return Flow::Stop;
        }
        if let Some(m) = msg.get::<LinkClicked>() {
            if is_url(&m.href) {
                return Flow::Continue;
            }
            let href = m.href.clone();
            self.go(&href, ctx);
            return Flow::Stop;
        }
        Flow::Continue
    }
}

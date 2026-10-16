//! `Markdown` and `MarkdownViewer` (port of `textual/widgets/_markdown.py`).
//!
//! ```
//! # use rustual::prelude::*;
//! # fn compose() -> Vec<Child> {
//! compose![Markdown::new("# Hello\n\nSome *markdown*.").id("md")]
//! # }
//! # fn handler(ctx: &mut Ctx) {
//! // later, from a handler:
//! ctx.with::<Markdown, _>("#md", |md, ctx| md.update("# New document", ctx));
//! # }
//! ```
//!
//! The document is parsed with `pulldown-cmark` (CommonMark + GFM tables, strikethrough and
//! task lists) into a tree of block widgets — [`MarkdownHeader`] (CSS types `MarkdownH1` ..
//! `MarkdownH6`), [`MarkdownParagraph`], [`MarkdownBlockQuote`], [`MarkdownBulletList`] /
//! [`MarkdownOrderedList`] with [`MarkdownListItem`]s, [`MarkdownFence`],
//! [`MarkdownHorizontalRule`] and [`MarkdownTable`] — mounted as children of the `Markdown`
//! widget, like Textual does.
//!
//! Messages: [`LinkClicked`] (a link was clicked; bubbles), [`TableOfContentsUpdated`]
//! (after mount / `update` / `append`), [`TableOfContentsSelected`] (from
//! [`MarkdownTableOfContents`]).
//!
//! Deviations from Textual:
//! - With [`Markdown::open_links`] (the default), only links with a URL scheme (`https:`,
//!   `mailto:` ...) are opened with [`crate::widgets::link::open_url`]; Textual passes every
//!   href to `App.open_url`. Relative links and `#anchors` are left to [`LinkClicked`]
//!   handlers (such as [`MarkdownViewer`]).
//! - Code fences are highlighted with the TextArea keyword highlighter (python, rust,
//!   javascript, json, toml; [`highlight_code`]) instead of Textual's Pygments-based
//!   `highlight`; replace it with [`Markdown::highlighter`] or disable it with
//!   `set_highlighter(None)`.
//! - `append` re-parses the whole document and remounts every block (no incremental update,
//!   no `MarkdownStream`).
//! - Headings are one widget type ([`MarkdownHeader`]) whose CSS type name depends on the level.
//! - Inline HTML and HTML blocks are dropped (as Textual does); footnotes are not enabled.

mod blocks;
mod inline;
mod parse;
mod table;
mod viewer;

pub use blocks::{
    fence_language, highlight_code, highlight_code_github_light, highlight_code_monokai,
    Highlighter, MarkdownBlockQuote, MarkdownBullet, MarkdownBulletList, MarkdownFence,
    MarkdownHeader, MarkdownHorizontalRule, MarkdownListItem, MarkdownOrderedList,
    MarkdownParagraph,
};
pub use parse::{parse, Block, ColumnAlign, Inline, InlineSpan, ListItem};
pub use table::MarkdownTable;
pub use viewer::{MarkdownTableOfContents, MarkdownViewer, Navigator, NavigatorUpdated};

use crate::context::Ctx;
use crate::dom::NodeId;
use crate::message::{Flow, Message};
use crate::widget::{Child, Widget, WidgetExt};
use crate::widgets::Vertical;
use rustual_css::Overflow;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

/// A table of contents entry: `(level, label, block id)`. The block id is the CSS id of the
/// heading widget (`heading-<slug>-<n>`).
pub type TocEntry = (u8, String, String);

/// Posted (bubbling) when a link in the document is clicked (Textual: `Markdown.LinkClicked`).
/// The sender is the block containing the link.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkClicked {
    /// The link target, percent-decoded.
    pub href: String,
}

/// Posted by `Markdown` when its table of contents changed (Textual:
/// `Markdown.TableOfContentsUpdated`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableOfContentsUpdated {
    pub table_of_contents: Vec<TocEntry>,
}

/// Posted by [`MarkdownTableOfContents`] when an entry was selected (Textual:
/// `Markdown.TableOfContentsSelected`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableOfContentsSelected {
    /// CSS id of the heading block.
    pub block_id: String,
}

/// Bullets for unordered lists, by nesting depth.
pub const BULLETS: [&str; 5] = ["• ", "▪ ", "‣ ", "⭑ ", "◦ "];

/// Task-list checkboxes: (unchecked, checked).
pub const CHECKBOXES: (&str, &str) = ("☐ ", "☒ ");

const DEFAULT_CSS: &str = "
Markdown {
    height: auto;
    padding: 0 2 0 2;
    layout: vertical;
    color: $foreground;
    overflow-y: hidden;

    MarkdownBlock {
        &:dark > .code_inline {
            background: $warning 10%;
            color: $text-warning 95%;
        }
        &:light > .code_inline {
            background: $error 5%;
            color: $text-error 95%;
        }
        & > .em {
            text-style: italic;
        }
        & > .strong {
            text-style: bold;
        }
        & > .s {
            text-style: strike;
        }
    }
}
MarkdownBlock {
    width: 1fr;
    height: auto;
}
MarkdownHeader {
    color: $text;
    margin: 2 0 1 0;
}

/* ANSI themes (Textual `:ansi`). */
Markdown {
    &:ansi {
        MarkdownBlock > .code_inline {
            background: ansi_default !important;
        }
    }
}
";

static HEADING_COUNTER: AtomicUsize = AtomicUsize::new(0);

/// Textual's `slug_for_tcss_id`: a slug usable as a CSS id.
pub fn slug_for_tcss_id(text: &str) -> String {
    let mut slug = String::new();
    for c in text.to_lowercase().replace(' ', "-").chars() {
        if c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' {
            slug.push(c);
        } else {
            slug.push_str(&format!("{:x}", c as u32));
        }
    }
    if slug.is_empty() {
        return "_".into();
    }
    if slug.starts_with(|c: char| c.is_ascii_digit()) {
        slug.insert(0, '_');
    }
    slug
}

/// GitHub-style heading slug (Textual's `_slug.slug`), used to resolve `#anchor` links.
pub fn slug(text: &str) -> String {
    let mut out = String::new();
    for c in text.trim().to_lowercase().chars() {
        let nonlingual = matches!(c as u32,
            0x24C2..=0x1F251 | 0x1F300..=0x1F5FF | 0x1F600..=0x1F64F | 0x1F680..=0x1F6FF
            | 0x1F900..=0x1F9FF | 0x200D);
        if c.is_whitespace() {
            out.push('-');
        } else if (c.is_ascii_punctuation() && c != '-' && c != '_') || nonlingual {
            continue;
        } else {
            out.push(c);
        }
    }
    out
}

/// Unique slugs within a document: repeats get `-1`, `-2`, ... (Textual's `TrackedSlugs`).
#[derive(Debug, Default, Clone)]
pub struct TrackedSlugs {
    used: HashMap<String, usize>,
}

impl TrackedSlugs {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn slug(&mut self, text: &str) -> String {
        let s = slug(text);
        let n = self.used.entry(s.clone()).or_insert(0);
        let out = if *n > 0 { format!("{s}-{n}") } else { s };
        *n += 1;
        out
    }
}

/// Split `"path#anchor"` into the path and the anchor (empty if none).
pub fn sanitize_location(location: &str) -> (PathBuf, String) {
    let (path, anchor) = location.split_once('#').unwrap_or((location, ""));
    let path = if path.is_empty() { "." } else { path };
    (PathBuf::from(path), anchor.to_string())
}

/// Build block widgets for `blocks`. `bullet_depth` counts enclosing unordered list items.
fn build_blocks(
    blocks: Vec<Block>,
    bullet_depth: usize,
    highlighter: blocks::FenceHighlight,
    toc: &mut Vec<TocEntry>,
    top_level: bool,
) -> Vec<Child> {
    let mut out = Vec::new();
    for block in blocks {
        match block {
            Block::Heading { level, inline } => {
                let n = HEADING_COUNTER.fetch_add(1, Ordering::Relaxed);
                let label = inline.plain();
                let id = format!("heading-{}-{n}", slug_for_tcss_id(&label));
                if top_level {
                    toc.push((level, label, id.clone()));
                }
                out.push(MarkdownHeader::new(level, inline).id(&id));
            }
            Block::Paragraph(inline) => out.push(MarkdownParagraph::new(inline).child()),
            Block::BlockQuote(children) => out.push(MarkdownBlockQuote::new().with(build_blocks(
                children,
                bullet_depth,
                highlighter,
                toc,
                false,
            ))),
            Block::List { start, items } => {
                let ordered = start.is_some();
                let first = start.unwrap_or(1);
                let suffix = ". ";
                let symbol_size = (0..items.len() as u64)
                    .map(|i| format!("{}{suffix}", first + i).chars().count())
                    .max()
                    .unwrap_or(0);
                let mut children = Vec::new();
                for (i, item) in items.into_iter().enumerate() {
                    let symbol = match (item.task, ordered) {
                        (Some(false), _) => CHECKBOXES.0.to_string(),
                        (Some(true), _) => CHECKBOXES.1.to_string(),
                        (None, true) => {
                            let s = format!("{}{suffix}", first + i as u64);
                            format!("{s:>width$}", width = symbol_size + 1)
                        }
                        (None, false) => BULLETS[bullet_depth % BULLETS.len()].to_string(),
                    };
                    let depth = bullet_depth + usize::from(!ordered);
                    let inner = build_blocks(item.blocks, depth, highlighter, toc, false);
                    children.push(MarkdownListItem::new().with([
                        MarkdownBullet::new(symbol).child(),
                        Vertical::new().with(inner),
                    ]));
                }
                out.push(if ordered {
                    MarkdownOrderedList::new().with(children)
                } else {
                    MarkdownBulletList::new().with(children)
                });
            }
            Block::Fence { lang, code } => out.push(
                MarkdownFence::new(code, lang)
                    .with_highlight(highlighter)
                    .child(),
            ),
            Block::Rule => out.push(MarkdownHorizontalRule::new().child()),
            Block::Table {
                aligns,
                header,
                rows,
            } => out.push(MarkdownTable::new(aligns, header, rows).child()),
        }
    }
    out
}

/// Scroll the scrollable ancestors of `target` so it is at the top of the viewport (as far as
/// possible). Works before the target has been laid out: ancestors are scrolled to the end
/// first, then the (post-layout) "scroll visible" pass scrolls back up just enough.
pub(crate) fn scroll_to_top(ctx: &mut Ctx, target: NodeId) {
    let ancestors = ctx.dom().ancestors(target);
    for anc in ancestors {
        let scrolls = ctx
            .dom()
            .get(anc)
            .map(|n| n.styles.get_overflow_y() != Overflow::Hidden)
            .unwrap_or(false);
        if scrolls {
            let x = ctx.dom().get(anc).map(|n| n.scroll_offset.x).unwrap_or(0);
            ctx.scroll_node_to(anc, x, i32::MAX / 4);
        }
    }
    macro_rules! request {
        ($($t:ty),*) => {
            None$(.or_else(|| ctx.with_id::<$t, _>(target, |_, c| c.scroll_visible())))*
        };
    }
    let _: Option<()> = request!(
        MarkdownHeader,
        MarkdownParagraph,
        MarkdownTable,
        MarkdownFence,
        MarkdownHorizontalRule,
        MarkdownBlockQuote,
        MarkdownBulletList,
        MarkdownOrderedList,
        MarkdownListItem
    );
}

/// A Markdown document rendered as a tree of block widgets.
pub struct Markdown {
    source: String,
    /// Blocks parsed at construction, mounted by `compose`.
    pending: Option<Vec<Block>>,
    toc: Vec<TocEntry>,
    highlighter: blocks::FenceHighlight,
    open_links: bool,
    node: Option<NodeId>,
}

impl Markdown {
    pub fn new(markdown: impl Into<String>) -> Self {
        let source = markdown.into();
        let blocks = parse(&source);
        Markdown {
            source,
            pending: Some(blocks),
            toc: Vec::new(),
            highlighter: blocks::FenceHighlight::Builtin,
            open_links: true,
            node: None,
        }
    }

    /// Open external links (with a URL scheme) when clicked (default `true`, Textual's
    /// `open_links`). [`LinkClicked`] is posted either way.
    pub fn open_links(mut self, open: bool) -> Self {
        self.open_links = open;
        self
    }

    pub fn set_open_links(&mut self, open: bool) {
        self.open_links = open;
    }

    pub fn opens_links(&self) -> bool {
        self.open_links
    }

    /// Syntax highlighter for code fences, replacing the built-in one (applies to blocks
    /// built after this call).
    pub fn highlighter(mut self, h: Highlighter) -> Self {
        self.highlighter = blocks::FenceHighlight::Custom(h);
        self
    }

    /// Replace the fence highlighter; `None` disables highlighting (applies to blocks built
    /// after this call).
    pub fn set_highlighter(&mut self, h: Option<Highlighter>) {
        self.highlighter = blocks::FenceHighlight::from_option(h);
    }

    /// The Markdown source.
    pub fn source(&self) -> &str {
        &self.source
    }

    /// The document's headings: `(level, label, block id)` (Textual: `table_of_contents`).
    /// Only top-level headings are included (not those inside lists or quotes).
    pub fn table_of_contents(&self) -> &[TocEntry] {
        &self.toc
    }

    fn build(&mut self, blocks: Vec<Block>) -> Vec<Child> {
        self.toc.clear();
        let mut toc = Vec::new();
        let children = build_blocks(blocks, 0, self.highlighter, &mut toc, true);
        self.toc = toc;
        children
    }

    fn node_id(&self, ctx: &Ctx) -> NodeId {
        self.node.unwrap_or_else(|| ctx.id())
    }

    /// Replace the document: removes the current blocks and mounts new ones, then posts
    /// [`TableOfContentsUpdated`]. `ctx` may belong to any widget.
    pub fn update(&mut self, markdown: impl Into<String>, ctx: &mut Ctx) {
        let node = self.node_id(ctx);
        self.source = markdown.into();
        let blocks = parse(&self.source);
        let children = self.build(blocks);
        ctx.remove_children(node);
        for child in children {
            ctx.mount_to(node, child, None);
        }
        let toc = self.toc.clone();
        ctx.post_message_to(
            node,
            TableOfContentsUpdated {
                table_of_contents: toc,
            },
        );
    }

    /// Append Markdown to the document (re-renders the whole document).
    pub fn append(&mut self, markdown: &str, ctx: &mut Ctx) {
        let source = format!("{}{markdown}", self.source);
        self.update(source, ctx);
    }

    /// Load a document from a file (`path#anchor` scrolls to the anchor).
    pub fn load(&mut self, location: impl AsRef<Path>, ctx: &mut Ctx) -> std::io::Result<()> {
        let (path, anchor) = sanitize_location(&location.as_ref().to_string_lossy());
        let text = std::fs::read_to_string(&path)?;
        self.update(text, ctx);
        if !anchor.is_empty() {
            self.goto_anchor(&anchor, ctx);
        }
        Ok(())
    }

    /// The block id of the heading whose GitHub-style slug is `anchor`.
    pub fn anchor_block_id(&self, anchor: &str) -> Option<String> {
        let anchor = anchor.trim_start_matches('#');
        let mut unique = TrackedSlugs::new();
        self.toc
            .iter()
            .find(|(_, title, _)| unique.slug(title) == anchor)
            .map(|(_, _, id)| id.clone())
    }

    /// Scroll the heading whose slug matches `anchor` to the top of the view. Returns false if
    /// there is no such heading.
    pub fn goto_anchor(&mut self, anchor: &str, ctx: &mut Ctx) -> bool {
        let Some(id) = self.anchor_block_id(anchor) else {
            return false;
        };
        self.scroll_to_block(&id, ctx)
    }

    /// Scroll the block with CSS id `block_id` to the top of the view.
    pub fn scroll_to_block(&self, block_id: &str, ctx: &mut Ctx) -> bool {
        let node = self.node_id(ctx);
        let Some(target) = ctx
            .query_in(node, &format!("#{block_id}"))
            .into_iter()
            .next()
        else {
            return false;
        };
        scroll_to_top(ctx, target);
        true
    }
}

impl Widget for Markdown {
    fn type_names(&self) -> &'static [&'static str] {
        &["Markdown"]
    }

    fn default_css(&self) -> &'static str {
        DEFAULT_CSS
    }

    fn compose(&mut self) -> Vec<Child> {
        match self.pending.take() {
            Some(blocks) => self.build(blocks),
            None => Vec::new(),
        }
    }

    fn on_mount(&mut self, ctx: &mut Ctx) {
        self.node = Some(ctx.id());
        let toc = self.toc.clone();
        ctx.post_message(TableOfContentsUpdated {
            table_of_contents: toc,
        });
    }

    fn on_message(&mut self, message: &Message, ctx: &mut Ctx) -> Flow {
        // Textual: `Markdown.on_markdown_link_clicked` (the message keeps bubbling).
        if let Some(link) = message.get::<LinkClicked>() {
            if self.open_links && is_url(&link.href) {
                let _ = ctx.open_url(&link.href);
            }
        }
        Flow::Continue
    }
}

/// Whether `href` has a URL scheme (`https:`, `mailto:` ...), i.e. is not a local path.
pub(crate) fn is_url(href: &str) -> bool {
    match href.split_once(':') {
        Some((scheme, _)) => {
            scheme.len() > 1
                && scheme.starts_with(|c: char| c.is_ascii_alphabetic())
                && scheme
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
        }
        None => false,
    }
}

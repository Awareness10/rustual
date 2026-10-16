//! Block widgets that make up a [`super::Markdown`] document (port of the `MarkdownBlock`
//! subclasses in `textual/widgets/_markdown.py`).

use super::inline::{InlineStyles, LinkMap, LinkTracker};
use super::parse::Inline;
use crate::context::Ctx;
use crate::events::Event;
use crate::message::Flow;
use crate::widget::{Content, RenderCtx, Widget};
use crate::widgets::text_area::{Highlighter as _, KeywordHighlighter, TextAreaTheme};
use rustual_render::{Strip, Style, Text};
use std::cell::RefCell;

/// Wrap inline content to the content width, recording link positions.
fn render_inline(ctx: &RenderCtx, inline: &Inline, links: &LinkTracker) -> Content {
    let styles = InlineStyles::new(ctx);
    let text = styles.text(inline, links.hover());
    let (overflow, no_wrap) = ctx.text_overflow();
    let lines = text.wrap(ctx.size.width, ctx.justify(), overflow, no_wrap);
    let mut map = LinkMap::default();
    map.add_lines(&lines, 0, 0);
    map.finish(&lines);
    links.set(map);
    Content::Lines(lines)
}

fn inline_height(inline: &Inline, width: i32) -> i32 {
    let mut t = Text::new();
    for s in &inline.spans {
        t.push(s.text.clone(), Style::default());
    }
    t.height_for_width(width)
}

// ---------------------------------------------------------------- headings

const H1_CSS: &str = "
MarkdownH1 {
    content-align: center middle;
    color: $markdown-h1-color;
    background: $markdown-h1-background;
    text-style: $markdown-h1-text-style;
}
";
const H2_CSS: &str = "
MarkdownH2 {
    color: $markdown-h2-color;
    background: $markdown-h2-background;
    text-style: $markdown-h2-text-style;
}
";
const H3_CSS: &str = "
MarkdownH3 {
    color: $markdown-h3-color;
    background: $markdown-h3-background;
    text-style: $markdown-h3-text-style;
    margin: 1 0;
    width: auto;
}
";
const H4_CSS: &str = "
MarkdownH4 {
    color: $markdown-h4-color;
    background: $markdown-h4-background;
    text-style: $markdown-h4-text-style;
    margin: 1 0;
}
";
const H5_CSS: &str = "
MarkdownH5 {
    color: $markdown-h5-color;
    background: $markdown-h5-background;
    text-style: $markdown-h5-text-style;
    margin: 1 0;
}
";
const H6_CSS: &str = "
MarkdownH6 {
    color: $markdown-h6-color;
    background: $markdown-h6-background;
    text-style: $markdown-h6-text-style;
    margin: 1 0;
}
";

/// A heading (`#` .. `######`). Its CSS type is `MarkdownH1` .. `MarkdownH6`
/// (plus `MarkdownHeader`, `MarkdownBlock`).
#[derive(Debug, Clone)]
pub struct MarkdownHeader {
    level: u8,
    inline: Inline,
    links: LinkTracker,
}

impl MarkdownHeader {
    pub fn new(level: u8, inline: Inline) -> Self {
        MarkdownHeader {
            level: level.clamp(1, 6),
            inline,
            links: LinkTracker::default(),
        }
    }

    /// Heading level, 1..=6.
    pub fn level(&self) -> u8 {
        self.level
    }

    /// The heading text without formatting.
    pub fn plain_text(&self) -> String {
        self.inline.plain()
    }
}

impl Widget for MarkdownHeader {
    fn type_names(&self) -> &'static [&'static str] {
        match self.level {
            1 => &["MarkdownH1", "MarkdownHeader", "MarkdownBlock", "Static"],
            2 => &["MarkdownH2", "MarkdownHeader", "MarkdownBlock", "Static"],
            3 => &["MarkdownH3", "MarkdownHeader", "MarkdownBlock", "Static"],
            4 => &["MarkdownH4", "MarkdownHeader", "MarkdownBlock", "Static"],
            5 => &["MarkdownH5", "MarkdownHeader", "MarkdownBlock", "Static"],
            _ => &["MarkdownH6", "MarkdownHeader", "MarkdownBlock", "Static"],
        }
    }

    fn default_css(&self) -> &'static str {
        match self.level {
            1 => H1_CSS,
            2 => H2_CSS,
            3 => H3_CSS,
            4 => H4_CSS,
            5 => H5_CSS,
            _ => H6_CSS,
        }
    }

    fn on_event(&mut self, event: &Event, ctx: &mut Ctx) -> Flow {
        self.links.handle_event(event, ctx).unwrap_or_default()
    }

    fn content_height(&self, _ctx: &RenderCtx, width: i32) -> Option<i32> {
        Some(inline_height(&self.inline, width))
    }

    fn render(&self, ctx: &RenderCtx) -> Content {
        render_inline(ctx, &self.inline, &self.links)
    }
}

// ---------------------------------------------------------------- paragraph

/// A paragraph of inline text.
#[derive(Debug, Clone)]
pub struct MarkdownParagraph {
    inline: Inline,
    links: LinkTracker,
}

impl MarkdownParagraph {
    pub fn new(inline: Inline) -> Self {
        MarkdownParagraph {
            inline,
            links: LinkTracker::default(),
        }
    }

    pub fn plain_text(&self) -> String {
        self.inline.plain()
    }
}

impl Widget for MarkdownParagraph {
    fn type_names(&self) -> &'static [&'static str] {
        &["MarkdownParagraph", "MarkdownBlock", "Static"]
    }

    fn default_css(&self) -> &'static str {
        "
Markdown > MarkdownParagraph {
     margin: 0 0 1 0;
}
"
    }

    fn on_event(&mut self, event: &Event, ctx: &mut Ctx) -> Flow {
        self.links.handle_event(event, ctx).unwrap_or_default()
    }

    fn content_height(&self, _ctx: &RenderCtx, width: i32) -> Option<i32> {
        Some(inline_height(&self.inline, width))
    }

    fn render(&self, ctx: &RenderCtx) -> Content {
        render_inline(ctx, &self.inline, &self.links)
    }
}

// ---------------------------------------------------------------- containers

macro_rules! block_container {
    ($(#[$doc:meta])* $name:ident, [$($type_name:literal),+], $css:literal) => {
        $(#[$doc])*
        #[derive(Debug, Default, Clone)]
        pub struct $name;

        impl $name {
            pub fn new() -> Self {
                $name
            }
        }

        impl Widget for $name {
            fn type_names(&self) -> &'static [&'static str] {
                &[$($type_name),+]
            }

            fn default_css(&self) -> &'static str {
                $css
            }
        }
    };
}

block_container!(
    /// A block quote (`>`); contains other blocks.
    MarkdownBlockQuote,
    ["MarkdownBlockQuote", "MarkdownBlock", "Static"],
    "
MarkdownBlockQuote {
    background: $boost;
    border-left: outer $text-primary 50%;
    margin: 1 0;
    padding: 0 1;
}
MarkdownBlockQuote:light {
    border-left: outer $text-secondary;
}
MarkdownBlockQuote > MarkdownBlockQuote {
    margin-left: 2;
    margin-top: 1;
}
"
);

block_container!(
    /// An unordered list; contains [`MarkdownListItem`]s.
    MarkdownBulletList,
    ["MarkdownBulletList", "MarkdownList", "MarkdownBlock", "Static"],
    "
MarkdownList {
    width: 1fr;
}
MarkdownList MarkdownList {
    margin: 0;
    padding-top: 0;
}
MarkdownBulletList {
    margin: 0 0 1 0;
    padding: 0 0;
}
"
);

block_container!(
    /// An ordered list; contains [`MarkdownListItem`]s.
    MarkdownOrderedList,
    ["MarkdownOrderedList", "MarkdownList", "MarkdownBlock", "Static"],
    "
MarkdownOrderedList {
    margin: 0 0 1 0;
    padding: 0 0;
}
"
);

block_container!(
    /// A list item: a [`MarkdownBullet`] next to a `Vertical` holding the item's blocks.
    MarkdownListItem,
    ["MarkdownListItem", "MarkdownBlock", "Static"],
    "
MarkdownListItem {
    layout: horizontal;
    margin-right: 1;
    height: auto;
}
MarkdownListItem > Vertical {
    width: 1fr;
    height: auto;
}
"
);

/// The bullet / number / checkbox of a list item.
#[derive(Debug, Clone)]
pub struct MarkdownBullet {
    symbol: String,
}

impl MarkdownBullet {
    pub fn new(symbol: impl Into<String>) -> Self {
        MarkdownBullet {
            symbol: symbol.into(),
        }
    }

    pub fn symbol(&self) -> &str {
        &self.symbol
    }
}

impl Widget for MarkdownBullet {
    fn type_names(&self) -> &'static [&'static str] {
        &["MarkdownBullet"]
    }

    fn default_css(&self) -> &'static str {
        "
MarkdownBullet {
    width: auto;
    height: auto;
    color: $text-primary;
    &:light {
        color: $text-secondary;
    }
}
"
    }

    fn render(&self, _ctx: &RenderCtx) -> Content {
        Content::Lines(vec![Strip::from_str(&self.symbol, Style::default())])
    }
}

// ---------------------------------------------------------------- rule

/// A horizontal rule (`---`).
#[derive(Debug, Default, Clone)]
pub struct MarkdownHorizontalRule;

impl MarkdownHorizontalRule {
    pub fn new() -> Self {
        MarkdownHorizontalRule
    }
}

impl Widget for MarkdownHorizontalRule {
    fn type_names(&self) -> &'static [&'static str] {
        &["MarkdownHorizontalRule", "MarkdownBlock", "Static"]
    }

    fn default_css(&self) -> &'static str {
        "
MarkdownHorizontalRule {
    border-bottom: solid $secondary;
    height: 1;
    padding-top: 1;
    margin-bottom: 1;
}
"
    }
}

// ---------------------------------------------------------------- fence

/// A syntax highlighter for code fences: `(code, language) -> Some(highlighted)`, or `None`
/// to fall back to plain text. The returned text is split into lines on `\n`.
pub type Highlighter = fn(&str, &str) -> Option<Text>;

/// How a fence is highlighted.
#[derive(Debug, Clone, Copy)]
pub(crate) enum FenceHighlight {
    Off,
    /// [`highlight_code`] with the `monokai` (dark) / `github_light` (light) theme.
    Builtin,
    Custom(Highlighter),
}

impl FenceHighlight {
    pub(crate) fn from_option(h: Option<Highlighter>) -> Self {
        h.map_or(FenceHighlight::Off, FenceHighlight::Custom)
    }
}

/// The built-in [`KeywordHighlighter`] language for a fence info string (`py`, `rs`, `js`
/// and the full names), if any.
pub fn fence_language(lang: &str) -> Option<&'static str> {
    let lang = lang
        .split_whitespace()
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    Some(match lang.as_str() {
        "python" | "py" | "python3" | "py3" => "python",
        "rust" | "rs" => "rust",
        "javascript" | "js" | "jsx" | "mjs" | "cjs" | "node" => "javascript",
        "json" | "jsonc" | "json5" => "json",
        "toml" => "toml",
        _ => return None,
    })
}

/// Highlight `code` with the TextArea's built-in keyword highlighter for `lang` (see
/// [`fence_language`]), styled by `theme`'s syntax styles. `None` if the language is unknown.
pub fn highlight_code(code: &str, lang: &str, theme: &TextAreaTheme) -> Option<Text> {
    let highlighter = KeywordHighlighter::for_language(fence_language(lang)?)?;
    let mut text = Text::new();
    for (i, line) in code.split('\n').enumerate() {
        if i > 0 {
            text.push("\n", Style::default());
        }
        let chars: Vec<char> = line.chars().collect();
        let mut pos = 0;
        for h in highlighter.highlight_line(i, line) {
            let (start, end) = (h.start.max(pos).min(chars.len()), h.end.min(chars.len()));
            if start >= end {
                continue;
            }
            if start > pos {
                text.push(
                    chars[pos..start].iter().collect::<String>(),
                    Style::default(),
                );
            }
            let style = theme.get_highlight(&h.name).cloned().unwrap_or_default();
            text.push(chars[start..end].iter().collect::<String>(), style);
            pos = end;
        }
        if pos < chars.len() {
            text.push(chars[pos..].iter().collect::<String>(), Style::default());
        }
    }
    Some(text)
}

/// A [`Highlighter`] using [`highlight_code`] with the `monokai` theme (the built-in default
/// under dark themes), for use with [`super::Markdown::highlighter`].
pub fn highlight_code_monokai(code: &str, lang: &str) -> Option<Text> {
    thread_local! {
        static MONOKAI: TextAreaTheme =
            TextAreaTheme::get_builtin_theme("monokai").unwrap_or_default();
    }
    MONOKAI.with(|t| highlight_code(code, lang, t))
}

/// A [`Highlighter`] using [`highlight_code`] with the `github_light` theme (the built-in
/// default under light themes).
pub fn highlight_code_github_light(code: &str, lang: &str) -> Option<Text> {
    thread_local! {
        static LIGHT: TextAreaTheme =
            TextAreaTheme::get_builtin_theme("github_light").unwrap_or_default();
    }
    LIGHT.with(|t| highlight_code(code, lang, t))
}

/// A fenced or indented code block. Lines are not wrapped (they are cropped).
///
/// Code in a known language (see [`fence_language`]) is highlighted by default with the
/// TextArea keyword highlighter: `monokai` colors under dark themes, `github_light` under light
/// ones. [`MarkdownFence::highlighter`] replaces (or, with `None`, disables) it.
#[derive(Debug, Clone)]
pub struct MarkdownFence {
    code: String,
    lang: String,
    highlight: FenceHighlight,
    /// `(dark, highlighted text)`: fences are re-rendered every frame.
    cache: RefCell<Option<(bool, Text)>>,
}

impl MarkdownFence {
    pub fn new(code: impl Into<String>, lang: impl Into<String>) -> Self {
        MarkdownFence {
            code: code.into(),
            lang: lang.into(),
            highlight: FenceHighlight::Builtin,
            cache: RefCell::new(None),
        }
    }

    /// Use a custom syntax highlighter instead of the built-in one; `None` disables
    /// highlighting.
    pub fn highlighter(mut self, h: Option<Highlighter>) -> Self {
        self.highlight = FenceHighlight::from_option(h);
        self
    }

    pub(crate) fn with_highlight(mut self, h: FenceHighlight) -> Self {
        self.highlight = h;
        self
    }

    pub fn code(&self) -> &str {
        &self.code
    }

    /// The fence's info string language (`"rust"` for ```` ```rust ````), may be empty.
    pub fn lang(&self) -> &str {
        &self.lang
    }

    fn text(&self, dark: bool) -> Text {
        if let Some((d, t)) = self.cache.borrow().as_ref() {
            if *d == dark {
                return t.clone();
            }
        }
        let highlighted = match self.highlight {
            FenceHighlight::Off => None,
            FenceHighlight::Custom(h) => h(&self.code, &self.lang),
            FenceHighlight::Builtin if dark => highlight_code_monokai(&self.code, &self.lang),
            FenceHighlight::Builtin => highlight_code_github_light(&self.code, &self.lang),
        };
        let text = highlighted.unwrap_or_else(|| Text::plain(self.code.clone()));
        *self.cache.borrow_mut() = Some((dark, text.clone()));
        text
    }
}

impl Widget for MarkdownFence {
    fn type_names(&self) -> &'static [&'static str] {
        &["MarkdownFence", "MarkdownBlock", "Static"]
    }

    fn default_css(&self) -> &'static str {
        // Textual puts a `Label` with `padding: 1 2` inside a horizontally scrolling fence.
        "
MarkdownFence {
    padding: 1 2;
    margin: 1 0;
    width: 1fr;
    height: auto;
    color: rgb(210,210,210);
    background: black 10%;
    &:light {
        background: white 30%;
    }
}

/* ANSI themes (Textual `:ansi`). */
MarkdownFence:ansi {
    background: transparent;

    margin: 0;
    & > Label {
        padding: 1 0;
    }
}
"
    }

    fn content_height(&self, _ctx: &RenderCtx, _width: i32) -> Option<i32> {
        Some(self.code.split('\n').count() as i32)
    }

    fn render(&self, ctx: &RenderCtx) -> Content {
        let text = self.text(ctx.env.dark);
        let lines = text.wrap(
            ctx.size.width,
            rustual_render::Justify::Left,
            rustual_render::TextOverflow::Clip,
            true,
        );
        Content::Lines(lines)
    }
}

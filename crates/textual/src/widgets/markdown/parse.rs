//! Markdown → block tree, using `pulldown-cmark` (GFM tables, strikethrough, task lists).
//!
//! This plays the role of Textual's `Markdown._parse_markdown` + `MarkdownBlock._token_to_content`:
//! the result is a tree of [`Block`]s whose inline content is a list of [`InlineSpan`]s carrying
//! the semantic flags (strong, em, ...) that are turned into styles at render time from the
//! component classes (`.strong`, `.em`, `.s`, `.code_inline`).

use pulldown_cmark::{Alignment, CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use std::iter::Peekable;

/// One run of inline text with uniform formatting.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct InlineSpan {
    pub text: String,
    pub strong: bool,
    pub em: bool,
    pub strike: bool,
    pub code: bool,
    /// Link target (links and images).
    pub link: Option<String>,
}

/// Inline content of a block (paragraph, heading, table cell).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Inline {
    pub spans: Vec<InlineSpan>,
}

impl Inline {
    /// The text without formatting.
    pub fn plain(&self) -> String {
        self.spans.iter().map(|s| s.text.as_str()).collect()
    }

    pub fn is_empty(&self) -> bool {
        self.spans.iter().all(|s| s.text.is_empty())
    }

    /// Hrefs of all links, in order (duplicates removed).
    pub fn links(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for s in &self.spans {
            if let Some(l) = &s.link {
                if !out.contains(l) {
                    out.push(l.clone());
                }
            }
        }
        out
    }

    fn push(&mut self, span: InlineSpan) {
        if span.text.is_empty() {
            return;
        }
        if let Some(last) = self.spans.last_mut() {
            let same = last.strong == span.strong
                && last.em == span.em
                && last.strike == span.strike
                && last.code == span.code
                && last.link == span.link;
            if same {
                last.text.push_str(&span.text);
                return;
            }
        }
        self.spans.push(span);
    }

    fn trim_end(&mut self) {
        while let Some(last) = self.spans.last_mut() {
            let t = last.text.trim_end_matches([' ', '\n']).len();
            last.text.truncate(t);
            if last.text.is_empty() {
                self.spans.pop();
            } else {
                break;
            }
        }
    }
}

/// Column alignment of a table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ColumnAlign {
    #[default]
    None,
    Left,
    Center,
    Right,
}

/// A list item.
#[derive(Debug, Clone, PartialEq)]
pub struct ListItem {
    /// `Some(checked)` for task-list items (`- [ ]` / `- [x]`).
    pub task: Option<bool>,
    pub blocks: Vec<Block>,
}

/// A block-level element.
#[derive(Debug, Clone, PartialEq)]
pub enum Block {
    Heading {
        level: u8,
        inline: Inline,
    },
    Paragraph(Inline),
    BlockQuote(Vec<Block>),
    List {
        /// `Some(start)` for ordered lists.
        start: Option<u64>,
        items: Vec<ListItem>,
    },
    Fence {
        lang: String,
        code: String,
    },
    Rule,
    Table {
        aligns: Vec<ColumnAlign>,
        header: Vec<Inline>,
        rows: Vec<Vec<Inline>>,
    },
}

/// Parse a Markdown document.
pub fn parse(source: &str) -> Vec<Block> {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);
    let mut p = BlockParser {
        events: Parser::new_ext(source, options).peekable(),
        task: None,
    };
    p.blocks()
}

struct BlockParser<'a> {
    events: Peekable<Parser<'a>>,
    /// Task marker seen in the current list item.
    task: Option<bool>,
}

fn heading_level(l: HeadingLevel) -> u8 {
    match l {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

/// Whether an event belongs to inline content (as opposed to starting / ending a block).
fn is_inline_event(e: &Event) -> bool {
    match e {
        Event::Text(_)
        | Event::Code(_)
        | Event::InlineHtml(_)
        | Event::InlineMath(_)
        | Event::SoftBreak
        | Event::HardBreak
        | Event::FootnoteReference(_)
        | Event::TaskListMarker(_) => true,
        Event::Start(t) => matches!(
            t,
            Tag::Emphasis
                | Tag::Strong
                | Tag::Strikethrough
                | Tag::Superscript
                | Tag::Subscript
                | Tag::Link { .. }
                | Tag::Image { .. }
        ),
        Event::End(t) => matches!(
            t,
            TagEnd::Emphasis
                | TagEnd::Strong
                | TagEnd::Strikethrough
                | TagEnd::Superscript
                | TagEnd::Subscript
                | TagEnd::Link
                | TagEnd::Image
        ),
        _ => false,
    }
}

/// Collapse runs of whitespace into single spaces (Textual: `re.sub(r"\s+", " ", ...)`).
fn collapse_ws(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut ws = false;
    for c in s.chars() {
        if c.is_whitespace() {
            if !ws {
                out.push(' ');
            }
            ws = true;
        } else {
            out.push(c);
            ws = false;
        }
    }
    out
}

#[derive(Default)]
struct InlineState {
    strong: u32,
    em: u32,
    strike: u32,
    links: Vec<String>,
}

impl InlineState {
    fn span(&self, text: String, code: bool) -> InlineSpan {
        InlineSpan {
            text,
            strong: self.strong > 0,
            em: self.em > 0,
            strike: self.strike > 0,
            code,
            link: self.links.last().cloned(),
        }
    }
}

impl<'a> BlockParser<'a> {
    /// Parse blocks until the end of the enclosing container (whose `End` is consumed) or EOF.
    fn blocks(&mut self) -> Vec<Block> {
        let mut out = Vec::new();
        while let Some(ev) = self.events.peek() {
            if is_inline_event(ev) {
                // Inline content directly in a container (tight list items): implicit paragraph.
                let inline = self.inline(true);
                if !inline.is_empty() {
                    out.push(Block::Paragraph(inline));
                }
                continue;
            }
            let Some(ev) = self.events.next() else { break };
            match ev {
                Event::End(_) => break,
                Event::Rule => out.push(Block::Rule),
                Event::Start(tag) => {
                    if let Some(b) = self.block(tag) {
                        out.push(b);
                    }
                }
                // HTML blocks, math, ...: ignored (Textual ignores unhandled tokens).
                _ => {}
            }
        }
        out
    }

    /// Parse the block started by `tag` (its `Start` already consumed).
    fn block(&mut self, tag: Tag<'a>) -> Option<Block> {
        match tag {
            Tag::Paragraph => {
                let inline = self.inline(false);
                (!inline.is_empty()).then_some(Block::Paragraph(inline))
            }
            Tag::Heading { level, .. } => Some(Block::Heading {
                level: heading_level(level),
                inline: self.inline(false),
            }),
            Tag::BlockQuote(_) => Some(Block::BlockQuote(self.blocks())),
            Tag::CodeBlock(kind) => {
                let lang = match kind {
                    CodeBlockKind::Fenced(info) => {
                        info.split_whitespace().next().unwrap_or("").to_string()
                    }
                    CodeBlockKind::Indented => String::new(),
                };
                let mut code = String::new();
                for ev in self.events.by_ref() {
                    match ev {
                        Event::Text(t) => code.push_str(&t),
                        Event::End(_) => break,
                        _ => {}
                    }
                }
                Some(Block::Fence {
                    lang,
                    code: code.trim_end().to_string(),
                })
            }
            Tag::List(start) => {
                let mut items = Vec::new();
                while let Some(ev) = self.events.next() {
                    match ev {
                        Event::Start(Tag::Item) => {
                            let saved = self.task.take();
                            let blocks = self.blocks();
                            let task = self.task.take();
                            self.task = saved;
                            items.push(ListItem { task, blocks });
                        }
                        Event::End(_) => break,
                        _ => {}
                    }
                }
                Some(Block::List { start, items })
            }
            Tag::Table(aligns) => {
                let aligns = aligns
                    .iter()
                    .map(|a| match a {
                        Alignment::None => ColumnAlign::None,
                        Alignment::Left => ColumnAlign::Left,
                        Alignment::Center => ColumnAlign::Center,
                        Alignment::Right => ColumnAlign::Right,
                    })
                    .collect();
                let mut header = Vec::new();
                let mut rows: Vec<Vec<Inline>> = Vec::new();
                let mut in_head = false;
                while let Some(ev) = self.events.next() {
                    match ev {
                        Event::Start(Tag::TableHead) => in_head = true,
                        Event::End(TagEnd::TableHead) => in_head = false,
                        Event::Start(Tag::TableRow) => rows.push(Vec::new()),
                        Event::Start(Tag::TableCell) => {
                            let cell = self.inline(false);
                            if in_head {
                                header.push(cell);
                            } else if let Some(r) = rows.last_mut() {
                                r.push(cell);
                            }
                        }
                        Event::End(TagEnd::Table) => break,
                        _ => {}
                    }
                }
                Some(Block::Table {
                    aligns,
                    header,
                    rows,
                })
            }
            // Unsupported containers (HTML blocks, footnotes, ...): skip their content.
            _ => {
                let mut depth = 1;
                for ev in self.events.by_ref() {
                    match ev {
                        Event::Start(_) => depth += 1,
                        Event::End(_) => {
                            depth -= 1;
                            if depth == 0 {
                                break;
                            }
                        }
                        _ => {}
                    }
                }
                None
            }
        }
    }

    /// Parse inline content. With `loose`, stops (without consuming) at the first non-inline
    /// event; otherwise consumes up to and including the enclosing block's `End`.
    fn inline(&mut self, loose: bool) -> Inline {
        let mut out = Inline::default();
        let mut st = InlineState::default();
        while let Some(ev) = self.events.peek() {
            if !is_inline_event(ev) {
                if !loose {
                    self.events.next();
                }
                break;
            }
            let Some(ev) = self.events.next() else { break };
            match ev {
                Event::Text(t) => out.push(st.span(collapse_ws(&t), false)),
                Event::Code(t) => out.push(st.span(t.to_string(), true)),
                Event::InlineMath(t) => out.push(st.span(t.to_string(), true)),
                Event::SoftBreak => out.push(st.span(" ".into(), false)),
                Event::HardBreak => out.push(st.span("\n".into(), false)),
                Event::TaskListMarker(checked) => self.task = Some(checked),
                Event::Start(Tag::Strong) => st.strong += 1,
                Event::End(TagEnd::Strong) => st.strong = st.strong.saturating_sub(1),
                Event::Start(Tag::Emphasis) => st.em += 1,
                Event::End(TagEnd::Emphasis) => st.em = st.em.saturating_sub(1),
                Event::Start(Tag::Strikethrough) => st.strike += 1,
                Event::End(TagEnd::Strikethrough) => st.strike = st.strike.saturating_sub(1),
                Event::Start(Tag::Link { dest_url, .. }) => st.links.push(dest_url.to_string()),
                Event::End(TagEnd::Link) => {
                    st.links.pop();
                }
                Event::Start(Tag::Image { dest_url, .. }) => {
                    // Textual renders images as "🖼  " + alt text, clickable as a link.
                    let mut alt = String::new();
                    for ev in self.events.by_ref() {
                        match ev {
                            Event::Text(t) | Event::Code(t) => alt.push_str(&t),
                            Event::End(TagEnd::Image) => break,
                            _ => {}
                        }
                    }
                    st.links.push(dest_url.to_string());
                    out.push(st.span(format!("🖼  {}", collapse_ws(&alt)), false));
                    st.links.pop();
                }
                // Inline HTML, footnote refs, sub/superscript markers: dropped.
                _ => {}
            }
        }
        out.trim_end();
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn para(blocks: &[Block]) -> &Inline {
        match &blocks[0] {
            Block::Paragraph(i) => i,
            b => panic!("not a paragraph: {b:?}"),
        }
    }

    #[test]
    fn inline_flags() {
        let b = parse("a **b** *c* ~~d~~ `e` [f](http://x)");
        let i = para(&b);
        let get = |t: &str| i.spans.iter().find(|s| s.text == t).unwrap().clone();
        assert!(get("b").strong);
        assert!(get("c").em);
        assert!(get("d").strike);
        assert!(get("e").code);
        assert_eq!(get("f").link.as_deref(), Some("http://x"));
        assert_eq!(i.plain(), "a b c d e f");
    }

    #[test]
    fn tight_and_task_lists() {
        let b = parse("- [ ] one\n- [x] two\n  - nested\n");
        let Block::List { start, items } = &b[0] else {
            panic!()
        };
        assert_eq!(*start, None);
        assert_eq!(items[0].task, Some(false));
        assert_eq!(items[1].task, Some(true));
        assert!(matches!(items[1].blocks[1], Block::List { .. }));
        assert_eq!(para(&items[0].blocks).plain(), "one");
    }

    #[test]
    fn table_and_fence() {
        let b = parse("| a | b |\n|:-|-:|\n| 1 | 2 |\n\n```rust\nfn x() {}\n```\n");
        let Block::Table {
            aligns,
            header,
            rows,
        } = &b[0]
        else {
            panic!()
        };
        assert_eq!(aligns, &[ColumnAlign::Left, ColumnAlign::Right]);
        assert_eq!(header[1].plain(), "b");
        assert_eq!(rows[0][0].plain(), "1");
        assert_eq!(
            b[1],
            Block::Fence {
                lang: "rust".into(),
                code: "fn x() {}".into()
            }
        );
    }

    #[test]
    fn breaks_and_images() {
        let b = parse("a\nb  \nc ![alt](img.png)");
        assert_eq!(para(&b).plain(), "a b\nc 🖼  alt");
    }
}

//! Tests for `Markdown` / `MarkdownViewer`.

use std::time::{SystemTime, UNIX_EPOCH};
use textual::prelude::*;
use textual::render::Attrs;
use textual::widgets::markdown::{self, MarkdownTableOfContents, TocEntry};

const DOC: &str = "\
# Title

Hello **bold** and *italic* and ~~gone~~ and `code` and [link](https://example.com).

## Lists

- one
- two
  - nested
    - deeper
- [ ] todo
- [x] done

1. first
2. second

> quoted text

```rust
fn main() {}
```

---

| Name | Qty |
|:-----|----:|
| apple | 3 |
| kiwi | 12 |

### Third
";

struct Root {
    doc: Option<String>,
    links: Vec<String>,
    tocs: Vec<Vec<TocEntry>>,
}

impl Root {
    fn new(doc: &str) -> Self {
        Root {
            doc: Some(doc.to_string()),
            links: vec![],
            tocs: vec![],
        }
    }
}

impl Widget for Root {
    fn compose(&mut self) -> Vec<Child> {
        compose![Markdown::new(self.doc.take().unwrap()).id("md")]
    }
    fn on_message(&mut self, msg: &Message, _ctx: &mut Ctx) -> Flow {
        if let Some(l) = msg.get::<markdown::LinkClicked>() {
            self.links.push(l.href.clone());
        }
        if let Some(t) = msg.get::<markdown::TableOfContentsUpdated>() {
            self.tocs.push(t.table_of_contents.clone());
        }
        Flow::Continue
    }
}

async fn pilot(doc: &str, size: Size) -> Pilot {
    App::new(Root::new(doc)).test(size).await
}

/// Position (x, y) of the first occurrence of `needle` in the screen text.
fn find(p: &mut Pilot, needle: &str) -> Option<(i32, i32)> {
    for (y, line) in p.screen_text().lines().enumerate() {
        if let Some(i) = line.find(needle) {
            let x = line[..i].chars().count() as i32;
            return Some((x, y as i32));
        }
    }
    None
}

fn root(p: &Pilot) -> &Root {
    p.app::<Root>().unwrap()
}

#[tokio::test]
async fn renders_every_block_type() {
    let mut p = pilot(DOC, Size::new(50, 60)).await;
    assert!(p.css_errors().is_empty(), "{:?}", p.css_errors());
    let text = p.screen_text();
    for needle in [
        "Title",
        "Hello bold and italic and gone and code and",
        "Lists",
        "• one",
        "• two",
        "▪ nested",
        "‣ deeper",
        "☐ todo",
        "☒ done",
        " 1. first",
        " 2. second",
        "▌ quoted text",
        "fn main() {}",
        "─────",
        "┌──",
        "│ Name",
        "Qty │",
        "│ apple",
        "12 │",
        "└──",
        "Third",
    ] {
        assert!(text.contains(needle), "missing {needle:?} in\n{text}");
    }
    let pos = |n: &str| text.find(n).unwrap();
    assert!(pos("Title") < pos("Hello"));
    assert!(pos("• one") < pos(" 1. first"));
    assert!(pos("quoted") < pos("fn main"));
    assert!(pos("fn main") < pos("┌──"));
    assert!(pos("└──") < pos("Third"));
    assert_eq!(p.query("MarkdownH1").len(), 1);
    assert_eq!(p.query("MarkdownHeader").len(), 3);
    assert_eq!(p.query("MarkdownBulletList").len(), 3);
    assert_eq!(p.query("MarkdownOrderedList").len(), 1);
    assert_eq!(p.query("MarkdownListItem").len(), 8);
    assert_eq!(p.query("MarkdownBlockQuote").len(), 1);
    assert_eq!(p.query("MarkdownFence").len(), 1);
    assert_eq!(p.query("MarkdownTable").len(), 1);
    assert_eq!(p.query("MarkdownHorizontalRule").len(), 1);
}

#[tokio::test]
async fn heading_styles_and_layout() {
    let mut p = pilot(DOC, Size::new(50, 60)).await;
    let (x, y) = find(&mut p, "Title").unwrap();
    assert!(x > 15, "H1 centered, x = {x}");
    let canvas = p.canvas();
    assert!(canvas.get(x, y).unwrap().attrs.contains(Attrs::BOLD));
    let (x2, y2) = find(&mut p, "Lists").unwrap();
    assert_eq!(x2, 2, "left aligned after padding");
    let canvas = p.canvas();
    assert!(canvas.get(x2, y2).unwrap().attrs.contains(Attrs::UNDERLINE));
    let text = p.screen_text();
    let lines: Vec<&str> = text.lines().collect();
    assert!(lines[y2 as usize - 1].trim().is_empty() && lines[y2 as usize - 2].trim().is_empty());
}

#[tokio::test]
async fn inline_styles() {
    let mut p = pilot(DOC, Size::new(50, 60)).await;
    let (_, y) = find(&mut p, "Hello").unwrap();
    let line = p.screen_text().lines().nth(y as usize).unwrap().to_string();
    let col = |w: &str| line[..line.find(w).unwrap()].chars().count() as i32;
    let canvas = p.canvas();
    let cell = |x: i32| canvas.get(x, y).unwrap().clone();
    let plain = cell(col("Hello"));
    assert!(plain.attrs.is_empty());
    assert!(cell(col("bold")).attrs.contains(Attrs::BOLD));
    assert!(cell(col("italic")).attrs.contains(Attrs::ITALIC));
    assert!(cell(col("gone")).attrs.contains(Attrs::STRIKE));
    let code = cell(col("code "));
    assert_ne!(code.bg, plain.bg, "inline code has a background");
    assert_ne!(code.fg, plain.fg, "inline code has its own color");
    let (lx, ly) = find(&mut p, "link.").unwrap();
    let canvas = p.canvas();
    let link = canvas.get(lx, ly).unwrap();
    assert!(link.attrs.contains(Attrs::UNDERLINE));
    assert_eq!(link.link.as_deref(), Some("https://example.com"));
    assert!(canvas.get(lx + 4, ly).unwrap().link.is_none());
}

#[tokio::test]
async fn fence_has_background() {
    let mut p = pilot(DOC, Size::new(50, 60)).await;
    let (x, y) = find(&mut p, "fn main").unwrap();
    let (hx, hy) = find(&mut p, "Hello").unwrap();
    let canvas = p.canvas();
    assert_ne!(canvas.get(x, y).unwrap().bg, canvas.get(hx, hy).unwrap().bg);
    assert_eq!(
        canvas.get(x, y - 1).unwrap().bg,
        canvas.get(x, y).unwrap().bg
    );
    assert_eq!(x, 4);
}

#[tokio::test]
async fn link_click_posts_message() {
    let mut p = pilot(DOC, Size::new(50, 60)).await;
    let (x, y) = find(&mut p, "Hello").unwrap();
    p.click_at(x, y).await;
    assert!(root(&p).links.is_empty(), "plain text is not a link");
    let (x, y) = find(&mut p, "link.").unwrap();
    p.click_at(x + 2, y).await;
    assert_eq!(root(&p).links, vec!["https://example.com".to_string()]);
    p.click_at(x + 4, y).await;
    assert_eq!(root(&p).links.len(), 1);
}

#[tokio::test]
async fn links_in_headings_tables_and_images() {
    let doc = "# [Home](home.md)\n\n| a | b |\n|---|---|\n| x | [cell%20link](c%20d.md) |\n\nImage: ![logo](logo.png)\n";
    let mut p = pilot(doc, Size::new(40, 20)).await;
    let (x, y) = find(&mut p, "Home").unwrap();
    p.click_at(x + 1, y).await;
    let (x, y) = find(&mut p, "cell").unwrap();
    p.click_at(x, y).await;
    let (x, y) = find(&mut p, "logo").unwrap();
    p.click_at(x, y).await;
    assert_eq!(
        root(&p).links,
        vec!["home.md".to_string(), "c d.md".into(), "logo.png".into()]
    );
    assert!(p.screen_text().contains("🖼  logo"));
}

#[tokio::test]
async fn link_hover_style() {
    let mut p = pilot("A [link](x) here", Size::new(30, 5)).await;
    let (x, y) = find(&mut p, "link").unwrap();
    let before = p.canvas().get(x, y).unwrap().clone();
    assert!(before.attrs.contains(Attrs::UNDERLINE));
    p.click_at(x, y).await;
    let after = p.canvas().get(x, y).unwrap().clone();
    assert!(after.attrs.contains(Attrs::BOLD), "hover style is bold");
    assert!(!after.attrs.contains(Attrs::UNDERLINE));
}

#[tokio::test]
async fn table_alignment_and_header() {
    let doc = "| Left | Center | Right |\n|:--|:-:|--:|\n| a | b | c |\n";
    let mut p = pilot(doc, Size::new(40, 10)).await;
    let text = p.screen_text();
    let row = text
        .lines()
        .find(|l| l.contains("│ a"))
        .unwrap()
        .to_string();
    let header = text
        .lines()
        .find(|l| l.contains("Left"))
        .unwrap()
        .to_string();
    let cells: Vec<&str> = row.trim().trim_matches('│').split('│').collect();
    assert_eq!(cells.len(), 3);
    assert!(cells[0].starts_with(" a"));
    assert!(cells[2].ends_with("c "));
    let b = cells[1].find('b').unwrap();
    assert!(b > 2 && b < cells[1].len() - 2, "centered: {:?}", cells[1]);
    let (x, y) = find(&mut p, "Left").unwrap();
    assert!(p.canvas().get(x, y).unwrap().attrs.contains(Attrs::BOLD));
    assert!(header.contains("Right │"));
}

#[tokio::test]
async fn narrow_table_wraps_cells() {
    let doc = "| Name | Description |\n|---|---|\n| x | a very long description that must wrap |\n";
    let mut p = pilot(doc, Size::new(30, 12)).await;
    let text = p.screen_text();
    assert!(text.contains("a very long"), "{text}");
    assert!(text.contains("wrap"), "{text}");
    assert!(text.contains("└"), "{text}");
}

#[tokio::test]
async fn ordered_list_start_and_wrapping_items() {
    let doc = "9. nine\n10. ten has a long line of text that wraps around\n";
    let mut p = pilot(doc, Size::new(30, 10)).await;
    let text = p.screen_text();
    assert!(text.contains("  9. nine"), "{text}");
    assert!(text.contains(" 10. ten"), "{text}");
    let lines: Vec<&str> = text.lines().collect();
    let i = lines.iter().position(|l| l.contains("10. ten")).unwrap();
    assert!(lines[i + 1].starts_with("      "), "{:?}", lines[i + 1]);
}

#[tokio::test]
async fn hard_and_soft_breaks() {
    let mut p = pilot("one\ntwo  \nthree", Size::new(30, 5)).await;
    let text = p.screen_text();
    assert!(text.contains("one two"));
    assert!(text.lines().any(|l| l.trim() == "three"));
}

#[tokio::test]
async fn table_of_contents_and_message() {
    let mut p = pilot(DOC, Size::new(50, 60)).await;
    let toc: Vec<TocEntry> = p
        .query_one::<Markdown>("#md")
        .unwrap()
        .table_of_contents()
        .to_vec();
    let summary: Vec<(u8, &str)> = toc.iter().map(|(l, s, _)| (*l, s.as_str())).collect();
    assert_eq!(summary, vec![(1, "Title"), (2, "Lists"), (3, "Third")]);
    for (_, _, id) in &toc {
        assert!(id.starts_with("heading-"));
        assert_eq!(p.query(&format!("#{id}")).len(), 1);
    }
    assert_eq!(root(&p).tocs, vec![toc]);
    let _ = p.screen_text();
}

#[tokio::test]
async fn update_replaces_document() {
    let mut p = pilot(DOC, Size::new(50, 60)).await;
    p.with::<Markdown, _>("#md", |md, ctx| md.update("# New\n\nfresh *text*", ctx))
        .await;
    let text = p.screen_text();
    assert!(
        text.contains("New") && text.contains("fresh text"),
        "{text}"
    );
    assert!(!text.contains("Title") && !text.contains("• one"));
    assert_eq!(p.query("MarkdownHeader").len(), 1);
    assert_eq!(p.query("MarkdownParagraph").len(), 1);
    let tocs = &root(&p).tocs;
    assert_eq!(tocs.len(), 2);
    assert_eq!(tocs[1][0].1, "New");
    assert_eq!(
        p.query_one::<Markdown>("#md").unwrap().source(),
        "# New\n\nfresh *text*"
    );
}

#[tokio::test]
async fn append_extends_document() {
    let mut p = pilot("# A\n\nfirst", Size::new(40, 20)).await;
    p.with::<Markdown, _>("#md", |md, ctx| md.append("\n\n## B\n\nsecond", ctx))
        .await;
    let text = p.screen_text();
    assert!(text.contains("first") && text.contains("second") && text.contains("B"));
    let toc = p
        .query_one::<Markdown>("#md")
        .unwrap()
        .table_of_contents()
        .to_vec();
    assert_eq!(toc.len(), 2);
}

fn highlight(code: &str, lang: &str) -> Option<Text> {
    (lang == "rust").then(|| {
        let mut t = Text::new();
        t.push(code.to_string(), Style::default().bold());
        t
    })
}

#[tokio::test]
async fn fence_highlighter_hook() {
    struct H;
    impl Widget for H {
        fn compose(&mut self) -> Vec<Child> {
            compose![
                Markdown::new("```rust\nlet x = 1;\n```\n\n```\nplain\n```").highlighter(highlight)
            ]
        }
    }
    let mut p = App::new(H).test(Size::new(30, 12)).await;
    let (x, y) = find(&mut p, "let x").unwrap();
    let (px, py) = find(&mut p, "plain").unwrap();
    let c = p.canvas();
    assert!(c.get(x, y).unwrap().attrs.contains(Attrs::BOLD));
    assert!(!c.get(px, py).unwrap().attrs.contains(Attrs::BOLD));
}

/// A long document so that scrolling is needed.
fn long_doc() -> String {
    let mut s = String::from("# Top\n\n");
    for i in 0..6 {
        s.push_str(&format!("## Section {i}\n\n"));
        for j in 0..6 {
            s.push_str(&format!("Paragraph {i}.{j}.\n\n"));
        }
    }
    s.push_str("## Same\n\ntext\n\n## Same\n\nlast one\n");
    s
}

struct Scrolling;

impl Widget for Scrolling {
    fn compose(&mut self) -> Vec<Child> {
        compose![VerticalScroll::new()
            .id("scroll")
            .with([Markdown::new(long_doc()).id("md")])]
    }
}

#[tokio::test]
async fn goto_anchor_scrolls_heading_to_top() {
    let mut p = App::new(Scrolling).test(Size::new(40, 12)).await;
    assert!(p.screen_text().contains("Top"));
    let found = p
        .with::<Markdown, _>("#md", |md, ctx| md.goto_anchor("section-3", ctx))
        .await
        .unwrap();
    assert!(found);
    p.pause().await;
    let first = p.screen_text().lines().next().unwrap().to_string();
    assert!(first.contains("Section 3"), "{}", p.screen_text());
    let found = p
        .with::<Markdown, _>("#md", |md, ctx| md.goto_anchor("same-1", ctx))
        .await
        .unwrap();
    assert!(found);
    p.pause().await;
    assert!(p.screen_text().contains("last one"));
    let missing = p
        .with::<Markdown, _>("#md", |md, ctx| md.goto_anchor("nope", ctx))
        .await
        .unwrap();
    assert!(!missing);
}

#[tokio::test]
async fn goto_anchor_right_after_update() {
    let mut p = App::new(Scrolling).test(Size::new(40, 12)).await;
    p.with::<Markdown, _>("#md", |md, ctx| {
        md.update(long_doc().replace("Section 4", "Target"), ctx);
        md.goto_anchor("target", ctx)
    })
    .await;
    p.pause().await;
    let first = p.screen_text().lines().next().unwrap().to_string();
    assert!(first.contains("Target"), "{}", p.screen_text());
}

#[test]
fn slugs() {
    assert_eq!(markdown::slug("Hello, World!"), "hello-world");
    assert_eq!(markdown::slug("  A_b-c  "), "a_b-c");
    let mut t = markdown::TrackedSlugs::new();
    assert_eq!(t.slug("x"), "x");
    assert_eq!(t.slug("x"), "x-1");
    assert_eq!(markdown::slug_for_tcss_id("1 Hi!"), "_1-hi21");
}

// ---------------------------------------------------------------- viewer

struct ViewerApp {
    viewer: Option<MarkdownViewer>,
    links: Vec<String>,
    nav: usize,
}

impl Widget for ViewerApp {
    fn compose(&mut self) -> Vec<Child> {
        compose![self.viewer.take().unwrap().id("viewer")]
    }
    fn on_message(&mut self, msg: &Message, _ctx: &mut Ctx) -> Flow {
        if let Some(l) = msg.get::<markdown::LinkClicked>() {
            self.links.push(l.href.clone());
        }
        if msg.is::<markdown::NavigatorUpdated>() {
            self.nav += 1;
        }
        Flow::Continue
    }
}

async fn viewer(v: MarkdownViewer, size: Size) -> Pilot {
    App::new(ViewerApp {
        viewer: Some(v),
        links: vec![],
        nav: 0,
    })
    .test(size)
    .await
}

#[tokio::test]
async fn viewer_shows_table_of_contents() {
    let mut p = viewer(MarkdownViewer::new(long_doc()), Size::new(60, 14)).await;
    assert!(p.css_errors().is_empty(), "{:?}", p.css_errors());
    let text = p.screen_text();
    assert!(text.contains("Ⅰ Top"), "{text}");
    assert!(text.contains("Ⅱ Section 0"), "{text}");
    let toc = p
        .query_one::<MarkdownTableOfContents>("MarkdownTableOfContents")
        .unwrap();
    assert_eq!(toc.table_of_contents().len(), 9);
    let (tx, _) = find(&mut p, "Ⅰ Top").unwrap();
    let (sx, _) = find(&mut p, "Ⅱ Section 0").unwrap();
    assert!(sx > tx, "sections nested under the H1");
}

#[tokio::test]
async fn viewer_toc_click_scrolls_to_heading() {
    let mut p = viewer(MarkdownViewer::new(long_doc()), Size::new(60, 14)).await;
    let (x, y) = find(&mut p, "Ⅱ Section 2").unwrap();
    p.click_at(x + 3, y).await;
    p.pause().await;
    let text = p.screen_text();
    assert!(text.lines().next().unwrap().contains("Section 2"), "{text}");
    p.press(&["down", "enter"]).await;
    p.pause().await;
    let text = p.screen_text();
    assert!(text.lines().next().unwrap().contains("Section 3"), "{text}");
}

#[tokio::test]
async fn viewer_without_table_of_contents() {
    let mut p = viewer(
        MarkdownViewer::new(long_doc()).show_table_of_contents(false),
        Size::new(60, 14),
    )
    .await;
    assert!(!p.screen_text().contains("Ⅰ Top"));
    let (x, _) = find(&mut p, "Top").unwrap();
    assert!(x > 20, "H1 centered over the full width");
    p.with::<MarkdownViewer, _>("#viewer", |v, ctx| v.set_show_table_of_contents(true, ctx))
        .await;
    assert!(p.screen_text().contains("Ⅰ Top"));
}

#[tokio::test]
async fn viewer_anchor_links_and_external_links() {
    let mut doc = String::from("[jump](#section-4) [web](https://example.com)\n\n");
    doc.push_str(&long_doc());
    let mut p = viewer(
        MarkdownViewer::new(doc).show_table_of_contents(false),
        Size::new(60, 14),
    )
    .await;
    let (x, y) = find(&mut p, "jump").unwrap();
    p.click_at(x, y).await;
    p.pause().await;
    assert!(
        p.screen_text()
            .lines()
            .next()
            .unwrap()
            .contains("Section 4"),
        "{}",
        p.screen_text()
    );
    assert!(p.app::<ViewerApp>().unwrap().links.is_empty());
    p.with::<VerticalScroll, _>("MarkdownViewer VerticalScroll", |_, ctx| {
        ctx.scroll_to(0, 0)
    })
    .await;
    let (x, y) = find(&mut p, "web").unwrap();
    p.click_at(x, y).await;
    assert_eq!(
        p.app::<ViewerApp>().unwrap().links,
        vec!["https://example.com"]
    );
}

fn temp_dir() -> std::path::PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("textual-md-{}-{nanos}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[tokio::test]
async fn viewer_navigates_local_files() {
    let dir = temp_dir();
    std::fs::write(dir.join("a.md"), "# Page A\n\nGo to [page b](b.md).\n").unwrap();
    std::fs::write(
        dir.join("b.md"),
        "# Page B\n\nBack to [a](a.md). [missing](nope.md)\n",
    )
    .unwrap();
    let mut p = viewer(MarkdownViewer::new(""), Size::new(60, 14)).await;
    let a = dir.join("a.md").to_string_lossy().to_string();
    let ok = p
        .with::<MarkdownViewer, _>("#viewer", |v, ctx| v.go(&a, ctx))
        .await
        .unwrap();
    assert!(ok);
    assert!(p.screen_text().contains("Page A"));
    assert!(p.screen_text().contains("Ⅰ Page A"), "TOC updated");
    let (x, y) = find(&mut p, "page b").unwrap();
    p.click_at(x, y).await;
    assert!(p.screen_text().contains("Page B"), "{}", p.screen_text());
    p.with::<MarkdownViewer, _>("#viewer", |v, ctx| v.back(ctx))
        .await;
    assert!(p.screen_text().contains("Page A"));
    p.with::<MarkdownViewer, _>("#viewer", |v, ctx| v.forward(ctx))
        .await;
    assert!(p.screen_text().contains("Page B"));
    let nav = p
        .query_one::<MarkdownViewer>("#viewer")
        .unwrap()
        .navigator()
        .clone();
    assert!(nav.end() && !nav.start());
    assert_eq!(p.app::<ViewerApp>().unwrap().nav, 4);
    let (x, y) = find(&mut p, "missing").unwrap();
    p.click_at(x, y).await;
    assert!(p.screen_text().contains("Page B"));
    assert!(!p.notifications().is_empty());
    std::fs::remove_dir_all(dir).ok();
}

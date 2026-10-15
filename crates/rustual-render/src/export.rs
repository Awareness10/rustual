//! Export a [`Canvas`] as an SVG screenshot (like Textual's `export_screenshot`) or as an
//! ANSI-escaped string (for printing a frame to a terminal or log).

use crate::canvas::{Canvas, Cell};
use crate::color::Color;
use crate::style::Attrs;
use std::fmt::Write;

const CHAR_WIDTH: f32 = 9.8;
const LINE_HEIGHT: f32 = 20.0;
const FONT_SIZE: f32 = 16.0;
const PADDING: f32 = 12.0;
const TITLE_BAR: f32 = 36.0;

fn xml_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

/// Effective (fg, bg) after applying reverse.
fn colors(cell: &Cell) -> (Color, Color) {
    if cell.attrs.contains(Attrs::REVERSE) {
        (cell.bg, cell.fg)
    } else {
        (cell.fg, cell.bg)
    }
}

impl Canvas {
    /// Render as a standalone SVG document with a window frame and `title`.
    pub fn to_svg(&self, title: &str) -> String {
        let size = self.size();
        let term_w = size.width as f32 * CHAR_WIDTH;
        let term_h = size.height as f32 * LINE_HEIGHT;
        let width = term_w + PADDING * 2.0;
        let height = term_h + PADDING * 2.0 + TITLE_BAR;
        let mut s = String::new();
        let _ = write!(
            s,
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}">
<style>
  .t {{ font-family: "Fira Code", "DejaVu Sans Mono", Menlo, Consolas, monospace; font-size: {FONT_SIZE}px; white-space: pre; dominant-baseline: central; }}
  .b {{ font-weight: bold; }} .i {{ font-style: italic; }} .u {{ text-decoration: underline; }} .s {{ text-decoration: line-through; }}
</style>
<rect width="{width}" height="{height}" rx="8" fill="#292929"/>
<circle cx="20" cy="18" r="6" fill="#ff5f57"/><circle cx="40" cy="18" r="6" fill="#febc2e"/><circle cx="60" cy="18" r="6" fill="#28c840"/>
<text class="t" x="{cx}" y="18" fill="#c5c8c6" text-anchor="middle">{title}</text>
<g transform="translate({PADDING}, {top})">
"##,
            cx = width / 2.0,
            top = TITLE_BAR + PADDING,
            title = xml_escape(title),
        );
        for y in 0..size.height {
            let row = self.row(y);
            let py = y as f32 * LINE_HEIGHT;
            // Backgrounds: merge runs of the same color.
            let mut x = 0usize;
            while x < row.len() {
                let (_, bg) = colors(&row[x]);
                let start = x;
                while x < row.len() && colors(&row[x]).1 == bg {
                    x += 1;
                }
                let _ = writeln!(
                    s,
                    r#"<rect x="{:.1}" y="{py:.1}" width="{:.1}" height="{LINE_HEIGHT}" fill="{}" shape-rendering="crispEdges"/>"#,
                    start as f32 * CHAR_WIDTH,
                    (x - start) as f32 * CHAR_WIDTH + 0.5,
                    bg.hex()
                );
            }
            // Text: one element per run of identical style, positioned per cell.
            let mut x = 0usize;
            while x < row.len() {
                let cell = &row[x];
                if cell.symbol.trim().is_empty() {
                    x += 1;
                    continue;
                }
                let (fg, _) = colors(cell);
                let attrs = cell.attrs;
                let start = x;
                let mut text = String::new();
                while x < row.len() {
                    let c = &row[x];
                    if c.is_continuation() {
                        x += 1;
                        continue;
                    }
                    if colors(c).0 != fg || c.attrs != attrs {
                        break;
                    }
                    text.push_str(&c.symbol);
                    x += 1;
                }
                let mut class = String::from("t");
                for (flag, name) in [
                    (Attrs::BOLD, " b"),
                    (Attrs::ITALIC, " i"),
                    (Attrs::UNDERLINE, " u"),
                    (Attrs::STRIKE, " s"),
                ] {
                    if attrs.contains(flag) {
                        class.push_str(name);
                    }
                }
                let opacity = if attrs.contains(Attrs::DIM) {
                    r#" opacity="0.6""#
                } else {
                    ""
                };
                let text = text.trim_end();
                let _ = writeln!(
                    s,
                    r#"<text xml:space="preserve" class="{class}" x="{:.1}" y="{:.1}" fill="{}"{opacity} textLength="{:.1}" lengthAdjust="spacingAndGlyphs">{}</text>"#,
                    start as f32 * CHAR_WIDTH,
                    py + LINE_HEIGHT / 2.0,
                    fg.hex(),
                    crate::cells::cell_len(text) as f32 * CHAR_WIDTH,
                    xml_escape(text)
                );
            }
        }
        s.push_str("</g>\n</svg>\n");
        s
    }

    /// Render as text with 24-bit ANSI escape codes, one line per row.
    pub fn to_ansi(&self) -> String {
        let mut s = String::new();
        let size = self.size();
        for y in 0..size.height {
            let mut last: Option<(Color, Color, Attrs)> = None;
            for cell in self.row(y) {
                if cell.is_continuation() {
                    continue;
                }
                let key = (cell.fg, cell.bg, cell.attrs);
                if last != Some(key) {
                    s.push_str("\x1b[0");
                    for (flag, code) in [
                        (Attrs::BOLD, ";1"),
                        (Attrs::DIM, ";2"),
                        (Attrs::ITALIC, ";3"),
                        (Attrs::UNDERLINE, ";4"),
                        (Attrs::BLINK, ";5"),
                        (Attrs::REVERSE, ";7"),
                        (Attrs::STRIKE, ";9"),
                    ] {
                        if cell.attrs.contains(flag) {
                            s.push_str(code);
                        }
                    }
                    let _ = write!(
                        s,
                        ";38;2;{};{};{};48;2;{};{};{}m",
                        cell.fg.r, cell.fg.g, cell.fg.b, cell.bg.r, cell.bg.g, cell.bg.b
                    );
                    last = Some(key);
                }
                s.push_str(&cell.symbol);
            }
            s.push_str("\x1b[0m\n");
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use crate::geometry::{Region, Size};
    use crate::style::Style;
    use crate::{Canvas, Color};

    fn sample() -> Canvas {
        let mut c = Canvas::new(Size::new(6, 2), Color::rgb(10, 20, 30));
        c.put_str(
            0,
            0,
            "a<b",
            &Style::new().fg(Color::rgb(255, 0, 0)).bold(),
            Region::new(0, 0, 6, 2),
        );
        c.put_str(0, 1, "日本", &Style::new(), Region::new(0, 0, 6, 2));
        c
    }

    #[test]
    fn svg_contains_escaped_text_and_colors() {
        let svg = sample().to_svg("My <App>");
        assert!(svg.starts_with("<svg"));
        assert!(svg.contains("a&lt;b"));
        assert!(svg.contains("My &lt;App&gt;"));
        assert!(svg.contains("#FF0000"));
        assert!(svg.contains("#0A141E"));
        assert!(svg.contains("class=\"t b\""));
        assert!(svg.contains("日本"));
        assert!(svg.trim_end().ends_with("</svg>"));
    }

    #[test]
    fn ansi_has_truecolor_codes() {
        let ansi = sample().to_ansi();
        assert!(ansi.contains("\x1b[0;1;38;2;255;0;0;48;2;10;20;30ma<b"));
        assert_eq!(ansi.lines().count(), 2);
        assert!(ansi.contains("日本"));
    }
}

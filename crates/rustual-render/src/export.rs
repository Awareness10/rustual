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

/// Stroke widths of box-drawing lines (px; a cell is `CHAR_WIDTH` × `LINE_HEIGHT`).
const LIGHT: f32 = 1.25;
const HEAVY: f32 = 2.5;
/// Double lines: two `DOUBLE_STROKE` lines, `DOUBLE_GAP` apart (center to center).
const DOUBLE_STROKE: f32 = 1.0;
const DOUBLE_GAP: f32 = 3.0;

/// Where a shape is drawn: the cells `x .. x + w` of a row, `cell_w` wide each.
struct Frame {
    x: f32,
    y: f32,
    w: f32,
    cell_w: f32,
    h: f32,
}

/// Weight of one arm of a box-drawing character.
#[derive(Clone, Copy, PartialEq, PartialOrd)]
enum Line {
    None,
    Light,
    Heavy,
    Double,
}

impl Line {
    fn width(self) -> f32 {
        match self {
            Line::None => 0.0,
            Line::Light => LIGHT,
            Line::Heavy => HEAVY,
            Line::Double => DOUBLE_GAP + DOUBLE_STROKE,
        }
    }
}

/// A character drawn as vector shapes instead of a font glyph: glyphs of block elements and box
/// drawing rarely fill the cell, which leaves gaps between rows and seams between columns.
enum Shape {
    /// Rectangles in cell fractions `(x, y, w, h)`, with a fill opacity (shades `░▒▓`).
    Blocks(&'static [(f32, f32, f32, f32)], f32),
    /// Box drawing: the weight of the up, right, down and left arms.
    Box([Line; 4]),
    /// A rounded corner (`╭╮╯╰`): the arms it joins, as `(right, down)` signs: `╭` is (1, 1).
    Arc(f32, f32),
}

const UL: (f32, f32, f32, f32) = (0.0, 0.0, 0.5, 0.5);
const UR: (f32, f32, f32, f32) = (0.5, 0.0, 0.5, 0.5);
const LL: (f32, f32, f32, f32) = (0.0, 0.5, 0.5, 0.5);
const LR: (f32, f32, f32, f32) = (0.5, 0.5, 0.5, 0.5);
const FULL: &[(f32, f32, f32, f32)] = &[(0.0, 0.0, 1.0, 1.0)];

/// Lower `n` eighths (`▁` .. `█`).
const LOWER: [&[(f32, f32, f32, f32)]; 8] = [
    &[(0.0, 0.875, 1.0, 0.125)],
    &[(0.0, 0.75, 1.0, 0.25)],
    &[(0.0, 0.625, 1.0, 0.375)],
    &[(0.0, 0.5, 1.0, 0.5)],
    &[(0.0, 0.375, 1.0, 0.625)],
    &[(0.0, 0.25, 1.0, 0.75)],
    &[(0.0, 0.125, 1.0, 0.875)],
    FULL,
];
/// Left `n` eighths, from `▉` (7) down to `▏` (1).
const LEFT: [&[(f32, f32, f32, f32)]; 7] = [
    &[(0.0, 0.0, 0.875, 1.0)],
    &[(0.0, 0.0, 0.75, 1.0)],
    &[(0.0, 0.0, 0.625, 1.0)],
    &[(0.0, 0.0, 0.5, 1.0)],
    &[(0.0, 0.0, 0.375, 1.0)],
    &[(0.0, 0.0, 0.25, 1.0)],
    &[(0.0, 0.0, 0.125, 1.0)],
];

/// The shape for a cell's symbol, if it is drawn as one.
fn shape_of(symbol: &str) -> Option<Shape> {
    let mut chars = symbol.chars();
    let c = chars.next()?;
    if chars.next().is_some() {
        return None;
    }
    use Line::{Double as D, Heavy as H, Light as L, None as N};
    let boxed = |arms: [Line; 4]| Some(Shape::Box(arms));
    match c as u32 {
        0x2580 => Some(Shape::Blocks(&[(0.0, 0.0, 1.0, 0.5)], 1.0)),
        n @ 0x2581..=0x2588 => Some(Shape::Blocks(LOWER[(n - 0x2581) as usize], 1.0)),
        n @ 0x2589..=0x258F => Some(Shape::Blocks(LEFT[(n - 0x2589) as usize], 1.0)),
        0x2590 => Some(Shape::Blocks(&[(0.5, 0.0, 0.5, 1.0)], 1.0)),
        0x2591 => Some(Shape::Blocks(FULL, 0.25)),
        0x2592 => Some(Shape::Blocks(FULL, 0.5)),
        0x2593 => Some(Shape::Blocks(FULL, 0.75)),
        0x2594 => Some(Shape::Blocks(&[(0.0, 0.0, 1.0, 0.125)], 1.0)),
        0x2595 => Some(Shape::Blocks(&[(0.875, 0.0, 0.125, 1.0)], 1.0)),
        0x2596 => Some(Shape::Blocks(&[LL], 1.0)),
        0x2597 => Some(Shape::Blocks(&[LR], 1.0)),
        0x2598 => Some(Shape::Blocks(&[UL], 1.0)),
        0x2599 => Some(Shape::Blocks(&[UL, LL, LR], 1.0)),
        0x259A => Some(Shape::Blocks(&[UL, LR], 1.0)),
        0x259B => Some(Shape::Blocks(&[UL, UR, LL], 1.0)),
        0x259C => Some(Shape::Blocks(&[UL, UR, LR], 1.0)),
        0x259D => Some(Shape::Blocks(&[UR], 1.0)),
        0x259E => Some(Shape::Blocks(&[UR, LL], 1.0)),
        0x259F => Some(Shape::Blocks(&[UR, LL, LR], 1.0)),
        // Light and heavy lines, corners, tees and crosses: [up, right, down, left].
        0x2500 => boxed([N, L, N, L]),
        0x2501 => boxed([N, H, N, H]),
        0x2502 => boxed([L, N, L, N]),
        0x2503 => boxed([H, N, H, N]),
        0x250C => boxed([N, L, L, N]),
        0x250F => boxed([N, H, H, N]),
        0x2510 => boxed([N, N, L, L]),
        0x2513 => boxed([N, N, H, H]),
        0x2514 => boxed([L, L, N, N]),
        0x2517 => boxed([H, H, N, N]),
        0x2518 => boxed([L, N, N, L]),
        0x251B => boxed([H, N, N, H]),
        0x251C => boxed([L, L, L, N]),
        0x2523 => boxed([H, H, H, N]),
        0x2524 => boxed([L, N, L, L]),
        0x252B => boxed([H, N, H, H]),
        0x252C => boxed([N, L, L, L]),
        0x2533 => boxed([N, H, H, H]),
        0x2534 => boxed([L, L, N, L]),
        0x253B => boxed([H, H, N, H]),
        0x253C => boxed([L, L, L, L]),
        0x254B => boxed([H, H, H, H]),
        // Double lines and corners (tees and crosses keep the font glyph).
        0x2550 => boxed([N, D, N, D]),
        0x2551 => boxed([D, N, D, N]),
        0x2554 => boxed([N, D, D, N]),
        0x2557 => boxed([N, N, D, D]),
        0x255A => boxed([D, D, N, N]),
        0x255D => boxed([D, N, N, D]),
        // Rounded corners.
        0x256D => Some(Shape::Arc(1.0, 1.0)),
        0x256E => Some(Shape::Arc(-1.0, 1.0)),
        0x256F => Some(Shape::Arc(-1.0, -1.0)),
        0x2570 => Some(Shape::Arc(1.0, -1.0)),
        // Half lines.
        0x2574 => boxed([N, N, N, L]),
        0x2575 => boxed([L, N, N, N]),
        0x2576 => boxed([N, L, N, N]),
        0x2577 => boxed([N, N, L, N]),
        0x2578 => boxed([N, N, N, H]),
        0x2579 => boxed([H, N, N, N]),
        0x257A => boxed([N, H, N, N]),
        0x257B => boxed([N, N, H, N]),
        0x257C => boxed([N, H, N, L]),
        0x257D => boxed([L, N, H, N]),
        0x257E => boxed([N, L, N, H]),
        0x257F => boxed([H, N, L, N]),
        _ => None,
    }
}

/// An axis-aligned rectangle with crisp edges: they snap to whole pixels the same way as the
/// neighbouring cell's, so no hairline seams show where cells meet at fractional pixels.
fn rect(s: &mut String, x: f32, y: f32, w: f32, h: f32, attrs: &str) {
    if w > 0.0 && h > 0.0 {
        let _ = writeln!(
            s,
            r#"<rect x="{x:.2}" y="{y:.2}" width="{w:.2}" height="{h:.2}"{attrs} shape-rendering="crispEdges"/>"#
        );
    }
}

impl Shape {
    /// Whether the shape spans the whole cell width, so runs of it merge into one.
    fn spans_width(&self) -> bool {
        match self {
            Shape::Blocks(rects, _) => rects.iter().all(|r| r.0 == 0.0 && r.2 == 1.0),
            Shape::Box([up, right, down, left]) => {
                *up == Line::None && *down == Line::None && right == left && *right != Line::Double
            }
            Shape::Arc(..) => false,
        }
    }

    fn write(&self, s: &mut String, f: &Frame, color: &str, attrs: &str) {
        match self {
            Shape::Blocks(rects, opacity) => {
                let attrs = if *opacity < 1.0 {
                    format!(r#"{attrs} fill-opacity="{opacity}""#)
                } else {
                    attrs.to_string()
                };
                for &(rx, ry, rw, rh) in *rects {
                    rect(
                        s,
                        f.x + rx * f.w,
                        f.y + ry * f.h,
                        rw * f.w,
                        rh * f.h,
                        &attrs,
                    );
                }
            }
            Shape::Box(arms) => box_lines(s, f, *arms, attrs),
            Shape::Arc(dx, dy) => {
                // A quarter circle from the middle of the horizontal edge it joins to the
                // middle of the vertical one, then straight on to the cell edges.
                let (cx, cy) = (f.x + f.cell_w / 2.0, f.y + f.h / 2.0);
                let r = f.cell_w / 2.0;
                let hx = if *dx > 0.0 { f.x + f.cell_w } else { f.x };
                let vy = if *dy > 0.0 { f.y + f.h } else { f.y };
                let sweep = if dx * dy > 0.0 { 0 } else { 1 };
                let _ = writeln!(
                    s,
                    r#"<path d="M{hx:.2} {cy:.2} H{:.2} A{r:.2} {r:.2} 0 0 {sweep} {cx:.2} {:.2} V{vy:.2}" fill="none" stroke="{color}" stroke-width="{LIGHT}"{}/>"#,
                    cx + dx * r,
                    cy + dy * r,
                    if attrs.contains("opacity=\"0.6\"") {
                        r#" opacity="0.6""#
                    } else {
                        ""
                    },
                );
            }
        }
    }
}

/// Light / heavy arms as rectangles from the cell center to its edges; double arms as two
/// parallel lines (straight lines and corners only).
fn box_lines(s: &mut String, f: &Frame, [up, right, down, left]: [Line; 4], attrs: &str) {
    let (x0, y0, x1, y1) = (f.x, f.y, f.x + f.w, f.y + f.h);
    let (cx, cy) = (f.x + f.cell_w / 2.0, f.y + f.h / 2.0);
    let vertical = up.width().max(down.width());
    let horizontal = left.width().max(right.width());
    if [up, right, down, left].contains(&Line::Double) {
        let g = DOUBLE_GAP / 2.0;
        let t = DOUBLE_STROKE;
        let (hor, ver) = (
            left != Line::None || right != Line::None,
            up != Line::None || down != Line::None,
        );
        if hor && !ver {
            for oy in [-g, g] {
                rect(s, x0, cy + oy - t / 2.0, x1 - x0, t, attrs);
            }
        } else if ver && !hor {
            for ox in [-g, g] {
                rect(s, cx + ox - t / 2.0, y0, t, y1 - y0, attrs);
            }
        } else {
            // A corner: the outer line turns at (-g, -g) away from the arms, the inner at (+g, +g).
            let sx = if right != Line::None { 1.0 } else { -1.0 };
            let sy = if down != Line::None { 1.0 } else { -1.0 };
            for k in [-g, g] {
                let (px, py) = (cx - sx * k, cy - sy * k);
                let (hx0, hx1) = if sx > 0.0 {
                    (px - t / 2.0, x1)
                } else {
                    (x0, px + t / 2.0)
                };
                let (vy0, vy1) = if sy > 0.0 {
                    (py - t / 2.0, y1)
                } else {
                    (y0, py + t / 2.0)
                };
                rect(s, hx0, py - t / 2.0, hx1 - hx0, t, attrs);
                rect(s, px - t / 2.0, vy0, t, vy1 - vy0, attrs);
            }
        }
        return;
    }
    // Horizontal arms reach the center minus half the vertical stroke so joints are filled.
    let left_end = cx - vertical / 2.0;
    let right_start = cx + vertical / 2.0;
    if left != Line::None && right == left {
        rect(s, x0, cy - left.width() / 2.0, x1 - x0, left.width(), attrs);
    } else {
        if left != Line::None {
            let w = left.width();
            rect(
                s,
                x0,
                cy - w / 2.0,
                right_start.max(cx + w / 2.0) - x0,
                w,
                attrs,
            );
        }
        if right != Line::None {
            let w = right.width();
            let from = left_end.min(cx - w / 2.0);
            rect(s, from, cy - w / 2.0, x1 - from, w, attrs);
        }
    }
    let top_end = cy + horizontal / 2.0;
    let bottom_start = cy - horizontal / 2.0;
    if up != Line::None && down == up {
        rect(s, cx - up.width() / 2.0, y0, up.width(), y1 - y0, attrs);
    } else {
        if up != Line::None {
            let w = up.width();
            rect(
                s,
                cx - w / 2.0,
                y0,
                w,
                top_end.max(cy + w / 2.0) - y0,
                attrs,
            );
        }
        if down != Line::None {
            let w = down.width();
            let from = bottom_start.min(cy - w / 2.0);
            rect(s, cx - w / 2.0, from, w, y1 - from, attrs);
        }
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
        // Backgrounds first (merged runs of the same color), so that shapes may overlap into the
        // next row without being painted over.
        for y in 0..size.height {
            let row = self.row(y);
            let py = y as f32 * LINE_HEIGHT;
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
        }
        for y in 0..size.height {
            let row = self.row(y);
            let py = y as f32 * LINE_HEIGHT;
            // Block elements and box drawing: vector shapes that fill the cell exactly.
            self.svg_shapes(&mut s, y);
            // Text: one element per run of identical style, positioned per cell.
            let mut x = 0usize;
            while x < row.len() {
                let cell = &row[x];
                if cell.symbol.trim().is_empty() || shape_of(&cell.symbol).is_some() {
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
                    if colors(c).0 != fg || c.attrs != attrs || shape_of(&c.symbol).is_some() {
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

    /// Write the shapes of row `y` (see [`shape_of`]); runs of identical cells become one shape.
    fn svg_shapes(&self, s: &mut String, y: i32) {
        let row = self.row(y);
        let mut x = 0usize;
        while x < row.len() {
            let cell = &row[x];
            let Some(shape) = shape_of(&cell.symbol) else {
                x += 1;
                continue;
            };
            let (fg, _) = colors(cell);
            // Horizontal runs of the same full-width shape (`─`, `▄`, `█`, ...) merge.
            let start = x;
            x += 1;
            if shape.spans_width() {
                while x < row.len()
                    && row[x].symbol == cell.symbol
                    && colors(&row[x]).0 == fg
                    && row[x].attrs.contains(Attrs::DIM) == cell.attrs.contains(Attrs::DIM)
                {
                    x += 1;
                }
            }
            let frame = Frame {
                x: start as f32 * CHAR_WIDTH,
                y: y as f32 * LINE_HEIGHT,
                w: (x - start) as f32 * CHAR_WIDTH,
                cell_w: CHAR_WIDTH,
                h: LINE_HEIGHT,
            };
            let mut attrs = format!(r#" fill="{}""#, fg.hex());
            if cell.attrs.contains(Attrs::DIM) {
                attrs.push_str(r#" opacity="0.6""#);
            }
            shape.write(s, &frame, &fg.hex(), &attrs);
        }
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

    fn line(text: &str) -> Canvas {
        let mut c = Canvas::new(Size::new(8, 1), Color::rgb(0, 0, 0));
        c.put_str(
            0,
            0,
            text,
            &Style::new().fg(Color::rgb(255, 0, 0)),
            Region::new(0, 0, 8, 1),
        );
        c
    }

    fn body(svg: &str) -> &str {
        &svg[svg.find("<g transform").unwrap()..]
    }

    #[test]
    fn block_elements_are_rectangles_not_glyphs() {
        // A full block fills the cell exactly; `▄` is its lower half.
        let svg = line("█▄").to_svg("t");
        let b = body(&svg);
        assert!(!b.contains('█') && !b.contains('▄'), "{b}");
        assert!(b.contains(r##"<rect x="0.00" y="0.00" width="9.80" height="20.00" fill="#FF0000" shape-rendering="crispEdges"/>"##), "{b}");
        assert!(
            b.contains(r##"<rect x="9.80" y="10.00" width="9.80" height="10.00" fill="#FF0000""##),
            "{b}"
        );
        // Shades are the full cell with a fill opacity.
        assert!(body(&line("▒").to_svg("t")).contains(r#"fill-opacity="0.5""#));
    }

    #[test]
    fn box_drawing_is_drawn_as_lines() {
        // A run of `─` is one rectangle through the middle of the cells.
        let svg = line("──────").to_svg("t");
        let b = body(&svg);
        assert!(!b.contains('─'), "{b}");
        let rects = b.matches(r##"fill="#FF0000""##).count();
        assert_eq!(rects, 1, "{b}");
        assert!(b.contains(r#"width="58.80" height="1.25""#), "{b}");
        // Rounded corners are arcs; corners and tees join their arms.
        assert!(body(&line("╭╮").to_svg("t"))
            .contains("<path d=\"M9.80 10.00 H9.80 A4.90 4.90 0 0 0 4.90 14.90 V20.00\""));
        assert_eq!(body(&line("┼").to_svg("t")).matches("<rect").count(), 3); // background + 2
        assert_eq!(
            body(&line("╔").to_svg("t"))
                .matches(r##"fill="#FF0000""##)
                .count(),
            4
        );
    }

    #[test]
    fn other_characters_stay_text() {
        // Dashed lines (and anything else) keep the font glyph, and break text runs around shapes.
        let svg = line("a┄b█c").to_svg("t");
        let b = body(&svg);
        assert!(b.contains(">a┄b</text>"), "{b}");
        assert!(b.contains(">c</text>"), "{b}");
        assert!(!b.contains('█'));
    }

    #[test]
    fn ansi_has_truecolor_codes() {
        let ansi = sample().to_ansi();
        assert!(ansi.contains("\x1b[0;1;38;2;255;0;0;48;2;10;20;30ma<b"));
        assert_eq!(ansi.lines().count(), 2);
        assert!(ansi.contains("日本"));
    }
}

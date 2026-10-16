//! Compositor: paints laid-out widgets onto a [`Canvas`].
//!
//! For each node in [`Arrangement::order`] (clipped to `layout.clip`):
//! 1. Fill the border box with the background (`background` composited over what is beneath;
//!    `background-tint` applied).
//! 2. Draw borders (`BorderKind::chars/locations`), border title/subtitle (markup, aligned per
//!    `border-title-align`, styled with `border-title-*`).
//! 3. Render content (`Widget::render`) into `content_region`: `Content::Text` is wrapped to the
//!    content width with `text-align`, `text-wrap`, `text-overflow`, then positioned by
//!    `content-align` (horizontal/vertical); `Content::Lines` drawn as-is. Base style is the
//!    widget's color (incl. `auto`) and `text-style` (see `widget::style_from_styles`).
//!    `text-opacity` blends text toward the background.
//! 4. Draw scrollbars inside the padding, at the edges of `content_region`
//!    (vertical: right, horizontal: bottom, plus corner) using `scrollbar-*` colors, with
//!    ⅛-cell thumb ends and hover / active colors (see `scrollbar.rs`).
//! 5. Draw outline over the border box edge.
//! 6. `tint` and `opacity` (< 1 blends the node's painted cells toward what was beneath).
//!
//! Screens: `layers` lists screens bottom → top; each is painted fully in turn (a modal screen's
//! semi-transparent background lets the screen beneath show through).

use crate::dom::{Dom, Node, NodeId};
use crate::layout::Arrangement;
use crate::scrollbar;
use crate::widget::{style_from_styles, Content, RenderCtx, RenderEnv};
use rustual_css::{AlignH, AlignV, BorderEdge, ColorValue, Hatch, Styles, TextAlign, Visibility};
use rustual_render::border::BorderCellStyle;
use rustual_render::{
    Attrs, BorderKind, Canvas, Cell, Color, Justify, Region, Size, Strip, Style, Text, TextOverflow,
};

pub fn render(dom: &Dom, env: &RenderEnv, layers: &[(NodeId, &Arrangement)], size: Size) -> Canvas {
    let mut canvas = Canvas::new(size, Color::BLACK);
    for (_screen, arrangement) in layers {
        for id in &arrangement.order {
            if let Some(node) = dom.get(*id) {
                paint_node(&mut canvas, dom, env, *id, node);
            }
        }
    }
    canvas
}

fn sample_bg(canvas: &Canvas, region: Region) -> Color {
    canvas
        .get(region.x, region.y)
        .map(|c| c.bg)
        .unwrap_or(Color::BLACK)
}

fn snapshot(canvas: &Canvas, region: Region) -> Vec<Cell> {
    let mut out = Vec::with_capacity(region.area().max(0) as usize);
    for y in region.y..region.bottom() {
        for x in region.x..region.right() {
            if let Some(c) = canvas.get(x, y) {
                out.push(c.clone());
            }
        }
    }
    out
}

fn paint_node(canvas: &mut Canvas, dom: &Dom, env: &RenderEnv, id: NodeId, node: &Node) {
    let layout = &node.layout;
    if !layout.visible {
        return;
    }
    let clip = layout.clip.intersection(&canvas.size().region());
    if clip.is_empty() {
        return;
    }
    let styles = &node.styles;
    let region = layout.region;
    let opacity = styles.get_opacity().clamp(0.0, 1.0);
    if opacity <= 0.0 {
        return;
    }
    let before = if opacity < 1.0 {
        Some(snapshot(canvas, clip))
    } else {
        None
    };

    // Outer background: what is beneath the widget.
    let outer_bg = sample_bg(canvas, clip);

    // 1. background
    // Hidden ancestors aren't painted, but (as in Textual, which derives backgrounds from the
    // ancestor chain) their backgrounds still show behind their visible descendants.
    let hidden_bg = hidden_ancestor_background(dom, id);
    if !hidden_bg.is_transparent() {
        canvas.fill(region, clip, hidden_bg);
    }
    let bg = styles.get_background();
    if node.is_screen && bg.a < 1.0 && !bg.is_transparent() {
        // A translucent screen (e.g. ModalScreen's `$background 60%`) dims the screen beneath
        // rather than erasing it: composite the color over every cell, keeping the glyphs.
        let area = region.intersection(&clip);
        for y in area.y..area.bottom() {
            for x in area.x..area.right() {
                if let Some(c) = canvas.get_mut(x, y) {
                    c.bg = c.bg.composite(bg);
                    c.fg = c.fg.composite(bg);
                }
            }
        }
    } else if !bg.is_transparent() {
        canvas.fill(region, clip, bg);
    } else if !node.is_screen {
        // Like Textual, a widget is an opaque rectangle: with a transparent background it shows
        // its ancestors' background *color*, not what they drew (e.g. a parent's `render()`).
        let parent_bg = dom
            .parent(id)
            .map(|p| crate::widget::effective_background(dom, p))
            .unwrap_or(Color::BLACK);
        canvas.fill(region, clip, parent_bg);
    }
    if let Some(tint) = styles.background_tint {
        if !tint.is_transparent() {
            for y in clip.y..clip.bottom() {
                for x in clip.x..clip.right() {
                    if let Some(c) = canvas.get_mut(x, y) {
                        c.bg = c.bg.tint(tint);
                    }
                }
            }
        }
    }
    let inner_bg = sample_bg(canvas, clip);
    let fg = resolve_fg(styles, inner_bg);

    // 2. border + titles
    paint_border(
        canvas,
        styles.get_border(),
        region,
        clip,
        inner_bg,
        outer_bg,
    );
    paint_labels(canvas, dom, env, id, node, inner_bg, outer_bg, fg, clip);

    // 3. content (+ hatch over its blank cells)
    paint_content(canvas, dom, env, id, node, clip);
    if let Some(hatch) = styles.get_hatch() {
        paint_hatch(
            canvas,
            hatch,
            region.shrink(styles.border_spacing()),
            clip,
            inner_bg,
        );
    }

    // 4. scrollbars
    paint_scrollbars(canvas, env, id, node, clip, inner_bg);

    // 5. outline (drawn over the edge of the border box)
    let outline = styles.get_outline();
    if outline.iter().any(|e| e.kind.has_width()) {
        paint_border(canvas, outline, region, clip, inner_bg, outer_bg);
    }

    // 6. tint & opacity
    if let Some(tint) = styles.tint {
        if !tint.is_transparent() {
            canvas.tint_region(clip, tint);
        }
    }
    if let Some(before) = before {
        let mut i = 0;
        for y in clip.y..clip.bottom() {
            for x in clip.x..clip.right() {
                if let (Some(c), Some(b)) = (canvas.get_mut(x, y), before.get(i)) {
                    let base = b.bg;
                    c.fg = base.blend(c.fg, opacity);
                    c.bg = base.blend(c.bg, opacity);
                }
                i += 1;
            }
        }
    }
}

/// `hatch`: draw blank cells of the padding box as the hatch character (Textual `apply_hatch`:
/// every run of spaces, including those between words, in `background + color` over the
/// widget's background). Children paint afterwards, so they cover it.
fn paint_hatch(canvas: &mut Canvas, hatch: Hatch, area: Region, clip: Region, bg: Color) {
    let area = area.intersection(&clip);
    let fg = bg.composite(hatch.color);
    let mut symbol = [0u8; 4];
    let symbol: &str = hatch.character.encode_utf8(&mut symbol);
    for y in area.y..area.bottom() {
        for x in area.x..area.right() {
            if let Some(c) = canvas.get_mut(x, y) {
                if c.symbol == " " {
                    c.symbol.replace_range(.., symbol);
                    c.fg = fg;
                    c.bg = bg;
                    c.attrs = Attrs::NONE;
                    c.link = None;
                }
            }
        }
    }
}

/// Composite background of the unbroken run of `visibility: hidden` ancestors directly above `id`
/// (transparent if the parent is visible).
fn hidden_ancestor_background(dom: &Dom, id: NodeId) -> Color {
    let mut chain = Vec::new();
    for anc in dom.ancestors(id) {
        let Some(n) = dom.get(anc) else { break };
        if !effectively_hidden(dom, anc) {
            break;
        }
        chain.push(n.styles.get_background());
    }
    chain.iter().rev().fold(Color::TRANSPARENT, |acc, bg| {
        if acc.is_transparent() {
            *bg
        } else {
            acc.composite(*bg)
        }
    })
}

/// Split unstyled leading spaces (added by justification) off a strip: (cells, rest).
fn split_leading_pad(line: &Strip) -> (i32, Strip) {
    let mut pad = 0;
    let mut spans = line.spans.iter().peekable();
    while let Some(span) = spans.peek() {
        if span.style.is_empty() && !span.text.is_empty() && span.text.chars().all(|c| c == ' ') {
            pad += span.text.len() as i32;
            spans.next();
        } else {
            break;
        }
    }
    (pad, Strip::new(spans.cloned().collect()))
}

/// `visibility` is inherited unless set explicitly (Textual semantics).
fn effectively_hidden(dom: &Dom, id: NodeId) -> bool {
    for n in dom.ancestors_with_self(id) {
        match dom.get(n).and_then(|n| n.styles.visibility) {
            Some(v) => return v == Visibility::Hidden,
            None => continue,
        }
    }
    false
}

/// The widget's text color over `bg` (resolving `auto`); WHITE if unset.
fn resolve_fg(styles: &Styles, bg: Color) -> Color {
    match styles.color {
        Some(ColorValue::Color(c)) => bg.composite(c),
        Some(ColorValue::Auto(a)) => bg.composite(bg.get_contrast_text(a)),
        None => Color::WHITE,
    }
}

/// Style for one border cell given Textual's location code (see `_border.py::get_box`).
fn border_cell_style(loc: BorderCellStyle, color: Color, inner: Color, outer: Color) -> Style {
    match loc {
        // Textual location 0: inner + style
        BorderCellStyle::Normal => Style::new().fg(inner.composite(color)).bg(inner),
        // 1: outer + style
        BorderCellStyle::Outer => Style::new().fg(outer.composite(color)).bg(outer),
        // 2: Style(outer.bg, inner.fg, reverse) → visual fg = outer bg, visual bg = border color
        BorderCellStyle::OuterReverse => Style::new().fg(outer).bg(inner.composite(color)),
        // 3: Style(inner.bg, outer.fg, reverse) → visual fg = inner bg, visual bg = border color
        BorderCellStyle::Reverse => Style::new().fg(inner).bg(outer.composite(color)),
    }
}

fn paint_border(
    canvas: &mut Canvas,
    edges: [BorderEdge; 4],
    region: Region,
    clip: Region,
    inner: Color,
    outer: Color,
) {
    let [top, right, bottom, left] = edges;
    if region.is_empty() {
        return;
    }
    let (has_t, has_r, has_b, has_l) = (
        top.kind.has_width(),
        right.kind.has_width(),
        bottom.kind.has_width(),
        left.kind.has_width(),
    );
    if !(has_t || has_r || has_b || has_l) {
        return;
    }
    let x0 = region.x;
    let x1 = region.right() - 1;
    let y0 = region.y;
    let y1 = region.bottom() - 1;
    let put = |canvas: &mut Canvas,
               x: i32,
               y: i32,
               kind: BorderKind,
               row: usize,
               col: usize,
               color: Color| {
        let ch = kind.chars()[row][col];
        let loc = kind.locations()[row][col];
        canvas.put_str(x, y, ch, &border_cell_style(loc, color, inner, outer), clip);
    };
    // top row
    if has_t {
        for x in x0..=x1 {
            let col = if x == x0 && has_l {
                0
            } else if x == x1 && has_r {
                2
            } else {
                1
            };
            put(canvas, x, y0, top.kind, 0, col, top.color);
        }
    }
    if has_b && y1 != y0 {
        for x in x0..=x1 {
            let col = if x == x0 && has_l {
                0
            } else if x == x1 && has_r {
                2
            } else {
                1
            };
            put(canvas, x, y1, bottom.kind, 2, col, bottom.color);
        }
    }
    let ys = if has_t { y0 + 1 } else { y0 };
    let ye = if has_b { y1 - 1 } else { y1 };
    for y in ys..=ye {
        if has_l {
            put(canvas, x0, y, left.kind, 1, 0, left.color);
        }
        if has_r && x1 != x0 {
            put(canvas, x1, y, right.kind, 1, 2, right.color);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn paint_labels(
    canvas: &mut Canvas,
    dom: &Dom,
    env: &RenderEnv,
    id: NodeId,
    node: &Node,
    inner: Color,
    outer: Color,
    fg: Color,
    clip: Region,
) {
    let styles = &node.styles;
    let [top, right, bottom, left] = styles.get_border();
    let region = node.layout.region;
    let ctx = RenderCtx::new(dom, env, id, region.size());
    let labels = [
        (
            node.border_title.as_deref(),
            top,
            region.y,
            0usize,
            styles.border_title_align,
            &styles.border_title_color,
            &styles.border_title_background,
            &styles.border_title_style,
        ),
        (
            node.border_subtitle.as_deref(),
            bottom,
            region.bottom() - 1,
            2usize,
            styles.border_subtitle_align,
            &styles.border_subtitle_color,
            &styles.border_subtitle_background,
            &styles.border_subtitle_style,
        ),
    ];
    for (label, edge, y, row, align, color, background, text_style) in labels {
        let Some(label) = label else { continue };
        if !edge.kind.has_width() || label.is_empty() {
            continue;
        }
        let has_l = left.kind.has_width();
        let has_r = right.kind.has_width();
        let corners = has_l as i32 + has_r as i32;
        let width = region.width;
        if width <= 2 * corners {
            continue;
        }
        let text: Text = ctx.markup(label);
        let max = width - 2 * corners;
        let mut strip = text
            .wrap(max, Justify::Left, TextOverflow::Ellipsis, true)
            .into_iter()
            .next()
            .unwrap_or_default();
        if strip.cell_len() > max {
            strip = strip.crop(0, max);
        }
        let mut spans = Vec::new();
        if has_l {
            spans.push(rustual_render::Span::raw(" "));
        }
        spans.extend(strip.spans);
        if has_r {
            spans.push(rustual_render::Span::raw(" "));
        }
        let label_strip = Strip::new(spans);
        // Base style from the edge's label location (BORDER_LABEL_LOCATIONS uses the middle column).
        let loc = edge.kind.locations()[row][1];
        let mut base = border_cell_style(loc, edge.color, inner, outer);
        // Labels default to the widget's text color on the border's background.
        if matches!(loc, BorderCellStyle::Normal | BorderCellStyle::Outer) {
            base.fg = Some(fg);
        }
        let (flip_top, flip_bottom) = edge.kind.title_flip();
        if (row == 0 && flip_top) || (row == 2 && flip_bottom) {
            std::mem::swap(&mut base.fg, &mut base.bg);
        }
        if let Some(c) = color {
            base.fg = Some(base.bg.unwrap_or(inner).composite(*c));
        }
        if let Some(b) = background {
            base.bg = Some(base.bg.unwrap_or(inner).composite(*b));
        }
        if let Some(ts) = text_style {
            base = base.combine(&Style {
                fg: None,
                bg: None,
                ..ts.clone()
            });
        }
        let len = label_strip.cell_len();
        let space = (width - corners - len).max(0);
        let start = match align.unwrap_or(AlignH::Left) {
            AlignH::Left => {
                if space > 0 {
                    1
                } else {
                    0
                }
            }
            AlignH::Center => space / 2,
            AlignH::Right => (space - 1).max(0),
        };
        let x = region.x + if has_l { 1 } else { 0 } + start;
        canvas.put_strip(x, y, &label_strip, clip, &base);
    }
}

fn paint_content(
    canvas: &mut Canvas,
    dom: &Dom,
    env: &RenderEnv,
    id: NodeId,
    node: &Node,
    clip: Region,
) {
    let cr = node.layout.content_region;
    if cr.is_empty() {
        return;
    }
    let Some(widget) = node.widget() else { return };
    let ctx = RenderCtx::new(dom, env, id, cr.size());
    let content = widget.render(&ctx);
    let inner_bg = sample_bg(canvas, cr.intersection(&clip));
    let mut base = style_from_styles(&node.styles, inner_bg);
    // The background is already painted; let text show it (and any translucency) through.
    base.bg = None;
    if base.fg.is_none() {
        base.fg = Some(Color::WHITE);
    }
    let content_clip = cr.intersection(&clip);
    if content_clip.is_empty() {
        return;
    }
    let is_text = matches!(content, Content::Text(_));
    let lines: Vec<Strip> = match content {
        Content::Empty => return,
        Content::Text(t) => {
            let (overflow, no_wrap) = ctx.text_overflow();
            t.wrap(cr.width, ctx.justify(), overflow, no_wrap)
        }
        Content::Lines(lines) => lines,
    };
    let (align_h, align_v) = node.styles.get_content_align();
    // Justified text (text-align center/right/justify) spans the full content width, as in
    // Textual, so horizontal content-align only moves left-aligned blocks.
    let justified = is_text
        && node.styles.get_text_align() != TextAlign::Start
        && node.styles.get_text_align() != TextAlign::Left;
    let block_w = if justified {
        cr.width
    } else {
        lines.iter().map(|l| l.cell_len()).max().unwrap_or(0)
    };
    let block_h = lines.len() as i32;
    let off_x = match align_h {
        AlignH::Left => 0,
        AlignH::Center => ((cr.width - block_w) / 2).max(0),
        AlignH::Right => (cr.width - block_w).max(0),
    };
    let off_y = match align_v {
        AlignV::Top => 0,
        AlignV::Middle => ((cr.height - block_h) / 2).max(0),
        AlignV::Bottom => (cr.height - block_h).max(0),
    };
    for (i, line) in lines.iter().enumerate() {
        let y = cr.y + off_y + i as i32;
        if y >= cr.bottom() {
            break;
        }
        // Justification padding is layout, not text: skip it so text styles such as `reverse`
        // only apply to the actual text (as in Textual).
        let (pad, line) = if justified {
            split_leading_pad(line)
        } else {
            (0, line.clone())
        };
        canvas.put_strip(cr.x + off_x + pad, y, &line, content_clip, &base);
    }
    let text_opacity = node.styles.get_text_opacity();
    if text_opacity < 1.0 {
        for y in content_clip.y..content_clip.bottom() {
            for x in content_clip.x..content_clip.right() {
                if let Some(c) = canvas.get_mut(x, y) {
                    c.fg = c.bg.blend(c.fg, text_opacity.max(0.0));
                }
            }
        }
    }
}

/// Paint scrollbars with Textual's `ScrollBarRender` (⅛-cell thumb ends), using the hover /
/// active colors for the scrollbar in `env.scrollbar_highlight`.
fn paint_scrollbars(
    canvas: &mut Canvas,
    env: &RenderEnv,
    id: NodeId,
    node: &Node,
    clip: Region,
    inner_bg: Color,
) {
    let layout = &node.layout;
    if !(layout.scrollbar_v || layout.scrollbar_h) {
        return;
    }
    let state = |vertical: bool| {
        env.scrollbar_highlight
            .filter(|h| h.node == id && h.vertical == vertical)
            .map(|h| h.active)
    };
    for vertical in [true, false] {
        let Some(region) = scrollbar::bar_region(node, vertical) else {
            continue;
        };
        let (back, bar) = scrollbar::bar_colors(node, inner_bg, state(vertical));
        let (size, virt, window, pos) = scrollbar::bar_metrics(node, vertical);
        let cells = scrollbar::render_bar(size, virt, window, pos, vertical, back, bar);
        for (i, cell) in cells.iter().enumerate() {
            let style = Style::new().fg(cell.fg).bg(cell.bg);
            let i = i as i32;
            let (x0, y0, w, h) = if vertical {
                (region.x, region.y + i, region.width, 1)
            } else {
                (region.x + i, region.y, 1, region.height)
            };
            for y in y0..y0 + h {
                for x in x0..x0 + w {
                    canvas.put_str(x, y, cell.glyph, &style, clip);
                }
            }
        }
    }
    if layout.scrollbar_v && layout.scrollbar_h {
        let s = &node.styles;
        let corner_color =
            inner_bg.composite(s.scrollbar_corner_color.unwrap_or(Color::rgb(30, 30, 30)));
        let cr = layout.content_region;
        let w = s.get_scrollbar_size_vertical();
        let h = s.get_scrollbar_size_horizontal();
        canvas.fill(
            Region::new(cr.right(), cr.bottom(), w, h),
            clip,
            corner_color,
        );
    }
}

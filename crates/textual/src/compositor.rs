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
//!    (vertical: right, horizontal: bottom, plus corner) using `scrollbar-*` colors.
//! 5. Draw outline over the border box edge.
//! 6. `tint` and `opacity` (< 1 blends the node's painted cells toward what was beneath).
//!
//! Screens: `layers` lists screens bottom → top; each is painted fully in turn (a modal screen's
//! semi-transparent background lets the screen beneath show through).

use crate::dom::{Dom, Node, NodeId};
use crate::layout::Arrangement;
use crate::widget::{style_from_styles, Content, RenderCtx, RenderEnv};
use textual_css::{AlignH, AlignV, BorderEdge, ColorValue, Styles};
use textual_render::border::BorderCellStyle;
use textual_render::{
    BorderKind, Canvas, Cell, Color, Justify, Region, Size, Strip, Style, Text, TextOverflow,
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

    // 3. content
    paint_content(canvas, dom, env, id, node, clip);

    // 4. scrollbars
    paint_scrollbars(canvas, node, clip, inner_bg);

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
            spans.push(textual_render::Span::raw(" "));
        }
        spans.extend(strip.spans);
        if has_r {
            spans.push(textual_render::Span::raw(" "));
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
    let lines: Vec<Strip> = match content {
        Content::Empty => return,
        Content::Text(t) => {
            let (overflow, no_wrap) = ctx.text_overflow();
            t.wrap(cr.width, ctx.justify(), overflow, no_wrap)
        }
        Content::Lines(lines) => lines,
    };
    let (align_h, align_v) = node.styles.get_content_align();
    let block_w = lines.iter().map(|l| l.cell_len()).max().unwrap_or(0);
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
        canvas.put_strip(cr.x + off_x, y, line, content_clip, &base);
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

/// (start, length) of a scrollbar thumb along a track.
pub(crate) fn thumb(track: i32, viewport: i32, virtual_len: i32, offset: i32) -> (i32, i32) {
    if track <= 0 || virtual_len <= 0 {
        return (0, 0);
    }
    if virtual_len <= viewport {
        return (0, track);
    }
    let len = ((viewport as f32 / virtual_len as f32) * track as f32)
        .ceil()
        .max(1.0) as i32;
    let len = len.min(track);
    let max_offset = (virtual_len - viewport).max(1);
    let start = ((offset.clamp(0, max_offset) as f32 / max_offset as f32) * (track - len) as f32)
        .round() as i32;
    (start, len)
}

fn paint_scrollbars(canvas: &mut Canvas, node: &Node, clip: Region, inner_bg: Color) {
    let layout = &node.layout;
    if !(layout.scrollbar_v || layout.scrollbar_h) {
        return;
    }
    let s = &node.styles;
    let track_color = inner_bg.composite(s.scrollbar_background.unwrap_or(Color::rgb(30, 30, 30)));
    let thumb_color = track_color.composite(s.scrollbar_color.unwrap_or(Color::rgb(90, 90, 90)));
    let corner_color =
        inner_bg.composite(s.scrollbar_corner_color.unwrap_or(Color::rgb(30, 30, 30)));
    let cr = layout.content_region;
    let vs = layout.virtual_size;
    let off = node.scroll_offset;
    if layout.scrollbar_v {
        let w = s.get_scrollbar_size_vertical();
        let (start, len) = thumb(cr.height, cr.height, vs.height, off.y);
        for dy in 0..cr.height {
            let on = dy >= start && dy < start + len;
            let color = if on { thumb_color } else { track_color };
            canvas.fill(Region::new(cr.right(), cr.y + dy, w, 1), clip, color);
        }
    }
    if layout.scrollbar_h {
        let h = s.get_scrollbar_size_horizontal();
        let (start, len) = thumb(cr.width, cr.width, vs.width, off.x);
        for dx in 0..cr.width {
            let on = dx >= start && dx < start + len;
            let color = if on { thumb_color } else { track_color };
            canvas.fill(Region::new(cr.x + dx, cr.bottom(), 1, h), clip, color);
        }
    }
    if layout.scrollbar_v && layout.scrollbar_h {
        let w = s.get_scrollbar_size_vertical();
        let h = s.get_scrollbar_size_horizontal();
        canvas.fill(
            Region::new(cr.right(), cr.bottom(), w, h),
            clip,
            corner_color,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::thumb;

    #[test]
    fn thumb_geometry() {
        assert_eq!(thumb(10, 10, 5, 0), (0, 10));
        assert_eq!(thumb(10, 10, 20, 0), (0, 5));
        assert_eq!(thumb(10, 10, 20, 10), (5, 5));
        assert_eq!(thumb(10, 10, 1000, 990), (9, 1));
    }
}

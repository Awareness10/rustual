//! Rich-style renderables: [`Table`], [`Panel`], [`Columns`], [`Padding`] and [`Group`], ports
//! of the `rich` classes of the same names (rich 15). They lay themselves out for a given width
//! and render to [`Strip`]s, so any widget can show them (e.g. `Static::renderable`).
//!
//! ```
//! use rustual_render::renderables::{render_lines, Panel, RenderOptions, Table};
//!
//! let mut table = Table::new().title("Planets");
//! table.add_column("Name");
//! table.add_column_with("Moons", |c| c.justify(rustual_render::Justify::Right));
//! table.add_row(["Earth", "1"]);
//! table.add_row(["Mars", "2"]);
//!
//! let lines = render_lines(&Panel::fit(table), &RenderOptions::new(40));
//! let text: Vec<String> = lines.iter().map(|l| l.plain_text().trim_end().to_string()).collect();
//! assert_eq!(text[0], "╭───────────────────╮");
//! assert_eq!(text[2], "│ ┏━━━━━━━┳━━━━━━━┓ │");
//! ```
//!
//! Strings are Textual markup (`"[b]bold[/]"`), like `rich`'s console markup.

mod boxes;
mod columns;
mod panel;
mod ratio;
mod table;

pub use boxes::BoxChars;
pub use columns::Columns;
pub use panel::Panel;
pub use table::{Column, Table, VerticalAlign};

/// The box styles of `rich.box` (`ROUNDED`, `HEAVY_HEAD`, `SQUARE`, `ASCII`, ...).
pub mod box_styles {
    pub use super::boxes::*;
}

use crate::strip::Strip;
use crate::style::Style;
use crate::text::{Justify, Text, TextOverflow};
use crate::Spacing;
use std::fmt;
use std::sync::Arc;

/// Options a renderable is rendered with (`rich.console.ConsoleOptions`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RenderOptions {
    /// Width available, in cells.
    pub max_width: i32,
    /// Exact height to render (`None`: as many lines as needed).
    pub height: Option<i32>,
    /// Text justification for text-like content (`None`: left).
    pub justify: Option<Justify>,
    /// Text overflow for text-like content (`None`: fold).
    pub overflow: Option<TextOverflow>,
    /// Disable wrapping of text-like content.
    pub no_wrap: bool,
}

impl RenderOptions {
    pub fn new(max_width: i32) -> Self {
        RenderOptions {
            max_width,
            height: None,
            justify: None,
            overflow: None,
            no_wrap: false,
        }
    }

    /// The same options with a different width.
    pub fn with_width(&self, max_width: i32) -> Self {
        RenderOptions { max_width, ..*self }
    }

    /// The same options with a different height.
    pub fn with_height(&self, height: Option<i32>) -> Self {
        RenderOptions { height, ..*self }
    }
}

/// The minimum and maximum widths a renderable needs (`rich.measure.Measurement`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Measurement {
    pub minimum: i32,
    pub maximum: i32,
}

impl Measurement {
    pub const fn new(minimum: i32, maximum: i32) -> Self {
        Measurement { minimum, maximum }
    }

    /// Ensure `0 <= minimum <= maximum`.
    pub fn normalize(self) -> Self {
        let minimum = self.minimum.max(0).min(self.maximum);
        Measurement::new(minimum.max(0), minimum.max(self.maximum).max(0))
    }

    /// Both widths capped at `width`.
    pub fn with_maximum(self, width: i32) -> Self {
        Measurement::new(self.minimum.min(width), self.maximum.min(width))
    }

    /// Both widths at least `width`.
    pub fn with_minimum(self, width: i32) -> Self {
        let width = width.max(0);
        Measurement::new(self.minimum.max(width), self.maximum.max(width))
    }

    /// Clamp within an optional range.
    pub fn clamp(self, min_width: Option<i32>, max_width: Option<i32>) -> Self {
        let mut m = self;
        if let Some(w) = min_width {
            m = m.with_minimum(w);
        }
        if let Some(w) = max_width {
            m = m.with_maximum(w);
        }
        m
    }
}

/// Something that lays itself out for a width and renders to lines (`rich`'s
/// `__rich_console__` + `__rich_measure__`).
pub trait Renderable: fmt::Debug + Send + Sync {
    /// Render for `options` (lines may be shorter than `options.max_width`; see [`render_lines`]).
    fn render(&self, options: &RenderOptions) -> Vec<Strip>;

    /// Width range this renderable needs (default: any width up to `max_width`).
    fn measure(&self, options: &RenderOptions) -> Measurement {
        Measurement::new(0, options.max_width)
    }
}

/// A shared renderable (cells of a table, the contents of a panel, ...).
pub type AnyRenderable = Arc<dyn Renderable>;

/// Conversion into an [`AnyRenderable`]: every [`Renderable`], plus strings (as markup).
pub trait IntoRenderable {
    fn into_renderable(self) -> AnyRenderable;
}

impl<T: Renderable + 'static> IntoRenderable for T {
    fn into_renderable(self) -> AnyRenderable {
        Arc::new(self)
    }
}

impl IntoRenderable for AnyRenderable {
    fn into_renderable(self) -> AnyRenderable {
        self
    }
}

impl IntoRenderable for &str {
    fn into_renderable(self) -> AnyRenderable {
        Arc::new(Text::from_markup(self))
    }
}

impl IntoRenderable for String {
    fn into_renderable(self) -> AnyRenderable {
        self.as_str().into_renderable()
    }
}

impl IntoRenderable for &String {
    fn into_renderable(self) -> AnyRenderable {
        self.as_str().into_renderable()
    }
}

/// Measure `renderable` within `options` (`Measurement.get`): normalized and capped at the width.
pub fn measure(renderable: &dyn Renderable, options: &RenderOptions) -> Measurement {
    let max_width = options.max_width;
    if max_width < 1 {
        return Measurement::new(0, 0);
    }
    let m = renderable
        .measure(options)
        .normalize()
        .with_maximum(max_width);
    if m.maximum < 1 {
        return Measurement::new(0, 0);
    }
    m.normalize()
}

/// Measurement fitting all `renderables` (`measure_renderables`).
pub fn measure_all(renderables: &[AnyRenderable], options: &RenderOptions) -> Measurement {
    if renderables.is_empty() {
        return Measurement::new(0, 0);
    }
    let ms: Vec<Measurement> = renderables
        .iter()
        .map(|r| measure(r.as_ref(), options))
        .collect();
    Measurement::new(
        ms.iter().map(|m| m.minimum).max().unwrap_or(0),
        ms.iter().map(|m| m.maximum).max().unwrap_or(0),
    )
    .with_maximum(options.max_width)
}

/// Render `renderable` to lines of exactly `options.max_width` cells (`Console.render_lines`
/// with `pad=True`), and exactly `options.height` lines when set.
pub fn render_lines(renderable: &dyn Renderable, options: &RenderOptions) -> Vec<Strip> {
    render_lines_styled(renderable, options, &Style::default(), true)
}

/// [`render_lines`] with `style` applied beneath every span (and used for padding); `pad`
/// controls whether lines are padded to the width (they are always cropped to it).
pub fn render_lines_styled(
    renderable: &dyn Renderable,
    options: &RenderOptions,
    style: &Style,
    pad: bool,
) -> Vec<Strip> {
    let width = options.max_width.max(0);
    let styled = *style != Style::default();
    let mut lines: Vec<Strip> = renderable
        .render(options)
        .into_iter()
        .map(|line| {
            let line = if styled {
                line.apply_base_style(style)
            } else {
                line
            };
            if pad || line.cell_len() > width {
                line.set_cell_len(width, style)
            } else {
                line
            }
        })
        .collect();
    if let Some(height) = options.height {
        let height = height.max(0) as usize;
        lines.truncate(height);
        while lines.len() < height {
            lines.push(Strip::blank(width, style.clone()));
        }
    }
    lines
}

/// Pad or crop every line to `width` and the line count to `height` (`Segment.set_shape`).
pub(crate) fn set_shape(lines: Vec<Strip>, width: i32, height: usize, style: &Style) -> Vec<Strip> {
    let mut out: Vec<Strip> = lines
        .into_iter()
        .take(height)
        .map(|l| l.set_cell_len(width, style))
        .collect();
    while out.len() < height {
        out.push(Strip::blank(width, style.clone()));
    }
    out
}

/// Vertical text alignment of `lines` within `height` lines (`Segment.align_top` & co.).
pub(crate) fn align_vertical(
    lines: Vec<Strip>,
    width: i32,
    height: usize,
    align: VerticalAlign,
    style: &Style,
) -> Vec<Strip> {
    let extra = height.saturating_sub(lines.len());
    if extra == 0 {
        return lines;
    }
    let blank = || Strip::blank(width, style.clone());
    let top = match align {
        VerticalAlign::Top => 0,
        VerticalAlign::Middle => extra / 2,
        VerticalAlign::Bottom => extra,
    };
    let mut out: Vec<Strip> = (0..top).map(|_| blank()).collect();
    out.extend(lines);
    out.extend((0..extra - top).map(|_| blank()));
    out
}

// ------------------------------------------------------------------------------------- Text

impl Renderable for Text {
    fn render(&self, options: &RenderOptions) -> Vec<Strip> {
        self.wrap(
            options.max_width,
            options.justify.unwrap_or_default(),
            options.overflow.unwrap_or_default(),
            options.no_wrap,
        )
    }

    /// Widest line, and the widest word as the minimum (`Text.__rich_measure__`).
    fn measure(&self, _options: &RenderOptions) -> Measurement {
        let plain = self.plain_text();
        let cell_len = |s: &str| crate::cells::cell_len(s);
        let max = plain.lines().map(cell_len).max().unwrap_or(0);
        let min = plain.split_whitespace().map(cell_len).max().unwrap_or(max);
        Measurement::new(min, max)
    }
}

// ---------------------------------------------------------------------------------- Padding

/// Blank space around a renderable (`rich.padding.Padding`).
#[derive(Debug, Clone)]
pub struct Padding {
    renderable: AnyRenderable,
    pad: Spacing,
    style: Style,
    expand: bool,
}

impl Padding {
    /// `pad` is `(top, right, bottom, left)`; see [`Spacing::all`], [`Spacing::symmetric`].
    pub fn new(renderable: impl IntoRenderable, pad: Spacing) -> Self {
        Padding {
            renderable: renderable.into_renderable(),
            pad,
            style: Style::default(),
            expand: true,
        }
    }

    /// Style of the padding (and beneath the content).
    pub fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }

    /// Fill the available width (default `true`); otherwise fit the content.
    pub fn expand(mut self, expand: bool) -> Self {
        self.expand = expand;
        self
    }
}

impl Renderable for Padding {
    fn render(&self, options: &RenderOptions) -> Vec<Strip> {
        let Spacing {
            top,
            right,
            bottom,
            left,
        } = self.pad;
        let width = if self.expand {
            options.max_width
        } else {
            (measure(self.renderable.as_ref(), options).maximum + left + right)
                .min(options.max_width)
        };
        let mut inner = options.with_width(width - left - right);
        if let Some(h) = options.height {
            inner = inner.with_height(Some(h - top - bottom));
        }
        let blank = || Strip::blank(width, self.style.clone());
        let mut out: Vec<Strip> = (0..top.max(0)).map(|_| blank()).collect();
        for line in render_lines_styled(self.renderable.as_ref(), &inner, &self.style, true) {
            let mut strip = Strip::default();
            if left > 0 {
                strip.push(crate::text::Span::new(
                    " ".repeat(left as usize),
                    self.style.clone(),
                ));
            }
            strip.spans.extend(line.spans);
            if right > 0 {
                strip.push(crate::text::Span::new(
                    " ".repeat(right as usize),
                    self.style.clone(),
                ));
            }
            out.push(strip);
        }
        out.extend((0..bottom.max(0)).map(|_| blank()));
        out
    }

    fn measure(&self, options: &RenderOptions) -> Measurement {
        let max_width = options.max_width;
        let extra = self.pad.left + self.pad.right;
        if max_width - extra < 1 {
            return Measurement::new(max_width, max_width);
        }
        let m = measure(self.renderable.as_ref(), options);
        Measurement::new(m.minimum + extra, m.maximum + extra).with_maximum(max_width)
    }
}

// ------------------------------------------------------------------------------------ Group

/// Renderables stacked vertically (`rich.console.Group`).
#[derive(Debug, Clone, Default)]
pub struct Group {
    renderables: Vec<AnyRenderable>,
}

impl Group {
    pub fn new<I, R>(renderables: I) -> Self
    where
        I: IntoIterator<Item = R>,
        R: IntoRenderable,
    {
        Group {
            renderables: renderables
                .into_iter()
                .map(IntoRenderable::into_renderable)
                .collect(),
        }
    }

    pub fn push(&mut self, renderable: impl IntoRenderable) {
        self.renderables.push(renderable.into_renderable());
    }
}

impl Renderable for Group {
    fn render(&self, options: &RenderOptions) -> Vec<Strip> {
        self.renderables
            .iter()
            .flat_map(|r| r.render(&options.with_height(None)))
            .collect()
    }

    fn measure(&self, options: &RenderOptions) -> Measurement {
        measure_all(&self.renderables, options)
    }
}

// --------------------------------------------------------------------- Align / Constrain

/// Horizontal alignment of a renderable that is narrower than the width (`rich.align.Align`).
#[derive(Debug, Clone)]
pub(crate) struct Align {
    pub renderable: AnyRenderable,
    pub justify: Justify,
}

impl Renderable for Align {
    fn render(&self, options: &RenderOptions) -> Vec<Strip> {
        let width = measure(self.renderable.as_ref(), options)
            .maximum
            .min(options.max_width);
        let inner = options.with_width(width).with_height(None);
        let lines = self.renderable.render(&inner);
        let shape_width = lines.iter().map(Strip::cell_len).max().unwrap_or(0);
        let height = lines.len();
        set_shape(lines, shape_width, height, &Style::default())
            .into_iter()
            .map(|l| l.align(options.max_width, self.justify, &Style::default()))
            .collect()
    }

    fn measure(&self, options: &RenderOptions) -> Measurement {
        measure(self.renderable.as_ref(), options)
    }
}

/// Limit a renderable's width (`rich.constrain.Constrain`).
#[derive(Debug, Clone)]
pub(crate) struct Constrain {
    pub renderable: AnyRenderable,
    pub width: i32,
}

impl Renderable for Constrain {
    fn render(&self, options: &RenderOptions) -> Vec<Strip> {
        self.renderable
            .render(&options.with_width(self.width.min(options.max_width)))
    }

    fn measure(&self, options: &RenderOptions) -> Measurement {
        measure(
            self.renderable.as_ref(),
            &options.with_width(self.width.min(options.max_width)),
        )
    }
}

#[cfg(test)]
mod tests;

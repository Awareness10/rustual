//! `Panel` (port of `rich/panel.py`).

use super::boxes::{BoxChars, ROUNDED};
use super::{
    measure, measure_all, render_lines_styled, AnyRenderable, IntoRenderable, Measurement, Padding,
    RenderOptions, Renderable,
};
use crate::strip::Strip;
use crate::style::Style;
use crate::text::{Justify, Span, Text, TextOverflow};
use crate::Spacing;

/// A renderable in a box, with an optional title and subtitle in the border
/// (`rich.panel.Panel`). Panels fill the width by default; [`Panel::fit`] fits the content.
#[derive(Debug, Clone)]
pub struct Panel {
    renderable: AnyRenderable,
    box_chars: BoxChars,
    title: Option<Text>,
    title_align: Justify,
    subtitle: Option<Text>,
    subtitle_align: Justify,
    expand: bool,
    style: Style,
    border_style: Style,
    width: Option<i32>,
    height: Option<i32>,
    padding: Spacing,
}

impl Panel {
    pub fn new(renderable: impl IntoRenderable) -> Self {
        Panel {
            renderable: renderable.into_renderable(),
            box_chars: ROUNDED,
            title: None,
            title_align: Justify::Center,
            subtitle: None,
            subtitle_align: Justify::Center,
            expand: true,
            style: Style::default(),
            border_style: Style::default(),
            width: None,
            height: None,
            padding: Spacing::symmetric(0, 1),
        }
    }

    /// A panel that fits its content instead of filling the width (`Panel.fit`).
    pub fn fit(renderable: impl IntoRenderable) -> Self {
        Panel::new(renderable).expand(false)
    }

    /// Box characters (default `ROUNDED`; see [`super::box_styles`]).
    pub fn box_style(mut self, box_chars: BoxChars) -> Self {
        self.box_chars = box_chars;
        self
    }
    /// Title in the top border (markup).
    pub fn title(mut self, title: &str) -> Self {
        self.title = Some(Text::from_markup(title));
        self
    }
    pub fn title_align(mut self, align: Justify) -> Self {
        self.title_align = align;
        self
    }
    /// Subtitle in the bottom border (markup).
    pub fn subtitle(mut self, subtitle: &str) -> Self {
        self.subtitle = Some(Text::from_markup(subtitle));
        self
    }
    pub fn subtitle_align(mut self, align: Justify) -> Self {
        self.subtitle_align = align;
        self
    }
    /// Fill the available width (default `true`).
    pub fn expand(mut self, expand: bool) -> Self {
        self.expand = expand;
        self
    }
    /// Style of the contents (and beneath the border).
    pub fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }
    pub fn border_style(mut self, style: Style) -> Self {
        self.border_style = style;
        self
    }
    /// Fixed width (including the border).
    pub fn width(mut self, width: i32) -> Self {
        self.width = Some(width);
        self
    }
    /// Fixed height (including the border).
    pub fn height(mut self, height: i32) -> Self {
        self.height = Some(height);
        self
    }
    /// Padding inside the border (default: one cell left and right).
    pub fn padding(mut self, padding: Spacing) -> Self {
        self.padding = padding;
        self
    }

    /// `" title "` on one line, as `Panel._title` prepares it.
    fn label(text: &Option<Text>) -> Option<Text> {
        let text = text.as_ref()?;
        if text.is_empty() {
            return None;
        }
        let mut out = Text::new();
        out.push(" ", Style::default());
        for span in &text.spans {
            out.push(span.text.replace('\n', " "), span.style.clone());
        }
        out.push(" ", Style::default());
        Some(out)
    }

    fn content(&self) -> AnyRenderable {
        if self.padding == Spacing::ZERO {
            self.renderable.clone()
        } else {
            Padding::new(self.renderable.clone(), self.padding).into_renderable()
        }
    }
}

/// The title or subtitle aligned within `width`, filled with the border character.
fn align_label(text: &Text, width: i32, align: Justify, fill: char, style: &Style) -> Strip {
    let line = text
        .wrap(width, Justify::Left, TextOverflow::Clip, true)
        .into_iter()
        .next()
        .unwrap_or_default();
    let excess = width - line.cell_len();
    if excess <= 0 {
        return line;
    }
    let fill = |n: i32| Span::new(fill.to_string().repeat(n.max(0) as usize), style.clone());
    let mut strip = Strip::default();
    let (left, right) = match align {
        Justify::Left | Justify::Full => (0, excess),
        Justify::Center => (excess / 2, excess - excess / 2),
        Justify::Right => (excess, 0),
    };
    if left > 0 {
        strip.push(fill(left));
    }
    strip.spans.extend(line.spans);
    if right > 0 {
        strip.push(fill(right));
    }
    strip
}

impl Renderable for Panel {
    fn render(&self, options: &RenderOptions) -> Vec<Strip> {
        let renderable = self.content();
        let style = self.style.clone();
        let border_style = style.combine(&self.border_style);
        let b = self.box_chars;
        let mut width = match self.width {
            None => options.max_width,
            Some(w) => options.max_width.min(w),
        };
        let title = Panel::label(&self.title).map(|mut t| {
            t.stylize_base(&border_style);
            t
        });
        let mut child_width = if self.expand {
            width - 2
        } else {
            measure(renderable.as_ref(), &options.with_width(width - 2)).maximum
        };
        let mut child_height = self.height.or(options.height);
        if let Some(h) = child_height.as_mut() {
            *h -= 2;
        }
        if let Some(t) = &title {
            child_width = (options.max_width - 2).min(child_width.max(t.cell_len() + 2));
        }
        width = child_width + 2;
        let child_options = options.with_width(child_width).with_height(child_height);
        let lines = render_lines_styled(renderable.as_ref(), &child_options, &style, true);

        let border = |s: String| Span::new(s, border_style.clone());
        let mut out = Vec::with_capacity(lines.len() + 2);
        match &title {
            Some(t) if width > 4 => {
                let mut strip = Strip::new(vec![border(format!("{}{}", b.top_left(), b.top()))]);
                strip.spans.extend(
                    align_label(t, width - 4, self.title_align, b.top(), &border_style).spans,
                );
                strip.push(border(format!("{}{}", b.top(), b.top_right())));
                out.push(strip);
            }
            _ => out.push(Strip::new(vec![border(b.get_top(&[width - 2]))])),
        }
        let (left, right, _) = b.mid_vertical();
        for line in lines {
            let mut strip = Strip::new(vec![border(left.to_string())]);
            strip.spans.extend(line.spans);
            strip.push(border(right.to_string()));
            out.push(strip);
        }
        let subtitle = Panel::label(&self.subtitle).map(|mut t| {
            t.stylize_base(&border_style);
            t
        });
        match &subtitle {
            Some(t) if width > 4 => {
                let mut strip =
                    Strip::new(vec![border(format!("{}{}", b.bottom_left(), b.bottom()))]);
                strip.spans.extend(
                    align_label(t, width - 4, self.subtitle_align, b.bottom(), &border_style).spans,
                );
                strip.push(border(format!("{}{}", b.bottom(), b.bottom_right())));
                out.push(strip);
            }
            _ => out.push(Strip::new(vec![border(b.get_bottom(&[width - 2]))])),
        }
        out
    }

    fn measure(&self, options: &RenderOptions) -> Measurement {
        let padding = self.padding.left + self.padding.right;
        let width = match self.width {
            Some(w) => w,
            None => {
                let mut renderables = vec![self.renderable.clone()];
                if let Some(t) = Panel::label(&self.title) {
                    renderables.push(t.into_renderable());
                }
                measure_all(
                    &renderables,
                    &options.with_width(options.max_width - padding - 2),
                )
                .maximum
                    + padding
                    + 2
            }
        };
        Measurement::new(width, width)
    }
}

//! `Sparkline` (port of `textual/widgets/_sparkline.py` and `textual/renderables/sparkline.py`):
//! a compact bar chart of numeric data, colored from `sparkline--min-color` to
//! `sparkline--max-color`.

use crate::widget::{Content, RenderCtx, Widget};
use rustual_render::{Color, Span, Strip, Style};

const DEFAULT_CSS: &str = "
Sparkline {
    height: 1;
}
Sparkline > .sparkline--max-color {
    color: $primary;
}
Sparkline > .sparkline--min-color {
    color: $primary 30%;
}
";

/// Summarizes the values of one bucket (one bar) into a single value.
pub type SummaryFunction = Box<dyn Fn(&[f64]) -> f64>;

/// The default summary function: the bucket's maximum.
pub fn summary_max(values: &[f64]) -> f64 {
    values.iter().copied().fold(f64::NEG_INFINITY, f64::max)
}

/// Renders data as sparkline rows (port of `textual.renderables.sparkline.Sparkline`).
pub struct SparklineRenderable<'a> {
    pub data: &'a [f64],
    pub width: i32,
    pub height: i32,
    pub min_color: Color,
    pub max_color: Color,
    pub summary: &'a dyn Fn(&[f64]) -> f64,
}

impl SparklineRenderable<'_> {
    pub const BARS: [&'static str; 8] = ["▁", "▂", "▃", "▄", "▅", "▆", "▇", "█"];

    /// Partition `data` into `num_buckets` buckets (empty buckets are skipped).
    pub fn buckets(data: &[f64], num_buckets: usize) -> Vec<&[f64]> {
        let len = data.len();
        (0..num_buckets)
            .filter_map(|b| {
                let start = len * b / num_buckets;
                let end = len * (b + 1) / num_buckets;
                (end > start).then(|| &data[start..end])
            })
            .collect()
    }

    pub fn render(&self) -> Vec<Strip> {
        let width = self.width.max(0);
        let height = self.height.max(1);
        let min_style = Style::new().fg(self.min_color);
        let max_style = Style::new().fg(self.max_color);
        let n = self.data.len();
        if n == 0 {
            let mut lines: Vec<Strip> = (0..height - 1).map(|_| Strip::default()).collect();
            lines.push(Strip::from_str(&"▁".repeat(width as usize), min_style));
            return lines;
        }
        if n == 1 {
            return (0..height)
                .map(|_| Strip::from_str(&"█".repeat(width as usize), max_style.clone()))
                .collect();
        }
        if width == 0 {
            return vec![Strip::default(); height as usize];
        }

        let bar_line_segments = Self::BARS.len() as i64;
        let bar_segments = bar_line_segments * height as i64 - 1;
        let minimum = self.data.iter().copied().fold(f64::INFINITY, f64::min);
        let maximum = self.data.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let extent = if maximum - minimum != 0.0 {
            maximum - minimum
        } else {
            1.0
        };
        let buckets = Self::buckets(self.data, width as usize);
        let step = buckets.len() as f64 / width as f64;

        let mut lines = Vec::with_capacity(height as usize);
        for i in (0..height as i64).rev() {
            let low = i * bar_line_segments;
            let high = (i + 1) * bar_line_segments;
            let mut spans: Vec<Span> = Vec::new();
            let mut bucket_index = 0.0;
            for _ in 0..width {
                let partition = buckets[(bucket_index as usize).min(buckets.len() - 1)];
                let summary = (self.summary)(partition);
                let ratio = (summary - minimum) / extent;
                let bar_index = (ratio * bar_segments as f64) as i64;
                let (bar, colored) = if bar_index < low {
                    (" ", false)
                } else if bar_index >= high {
                    ("█", true)
                } else {
                    (Self::BARS[(bar_index % bar_line_segments) as usize], true)
                };
                let style = if colored {
                    Style::new().fg(blend_colors(self.min_color, self.max_color, ratio))
                } else {
                    Style::new()
                };
                spans.push(Span::new(bar, style));
                bucket_index += step;
            }
            let mut strip = Strip::new(spans);
            strip = strip.simplify();
            lines.push(strip);
        }
        lines
    }
}

/// `_blend_colors.blend_colors`: linear RGB blend (unclamped ratio, like Rich's `Color.from_rgb`).
fn blend_colors(c1: Color, c2: Color, ratio: f64) -> Color {
    let lerp = |a: u8, b: u8| (a as f64 + (b as f64 - a as f64) * ratio).clamp(0.0, 255.0) as u8;
    Color::rgb(lerp(c1.r, c2.r), lerp(c1.g, c2.g), lerp(c1.b, c2.b))
}

/// A sparkline widget. Each column summarizes a bucket of the data (max by default).
pub struct Sparkline {
    data: Vec<f64>,
    min_color: Option<Color>,
    max_color: Option<Color>,
    summary: SummaryFunction,
}

impl Default for Sparkline {
    fn default() -> Self {
        Sparkline::new(Vec::new())
    }
}

impl std::fmt::Debug for Sparkline {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Sparkline")
            .field("data", &self.data)
            .field("min_color", &self.min_color)
            .field("max_color", &self.max_color)
            .finish_non_exhaustive()
    }
}

impl Sparkline {
    pub fn new(data: impl Into<Vec<f64>>) -> Self {
        Sparkline {
            data: data.into(),
            min_color: None,
            max_color: None,
            summary: Box::new(summary_max),
        }
    }

    /// Color for the smallest values (instead of `sparkline--min-color` from CSS).
    pub fn min_color(mut self, color: Color) -> Self {
        self.min_color = Some(color);
        self
    }

    /// Color for the largest values (instead of `sparkline--max-color` from CSS).
    pub fn max_color(mut self, color: Color) -> Self {
        self.max_color = Some(color);
        self
    }

    /// How a bucket of values becomes one bar (default: max).
    pub fn summary_function(mut self, f: impl Fn(&[f64]) -> f64 + 'static) -> Self {
        self.summary = Box::new(f);
        self
    }

    pub fn data(&self) -> &[f64] {
        &self.data
    }

    pub fn set_data(&mut self, data: impl Into<Vec<f64>>) {
        self.data = data.into();
    }

    /// Append a value, keeping at most `max_len` values (oldest dropped).
    pub fn push(&mut self, value: f64, max_len: usize) {
        self.data.push(value);
        if self.data.len() > max_len {
            let excess = self.data.len() - max_len;
            self.data.drain(..excess);
        }
    }
}

impl Widget for Sparkline {
    fn type_names(&self) -> &'static [&'static str] {
        &["Sparkline"]
    }

    fn default_css(&self) -> &'static str {
        DEFAULT_CSS
    }

    fn render(&self, ctx: &RenderCtx) -> Content {
        let base = ctx.background();
        let resolve = |explicit: Option<Color>, class: &str| match explicit {
            Some(c) => base.composite(c),
            None => ctx.component_style(class).fg.unwrap_or(Color::WHITE),
        };
        let renderable = SparklineRenderable {
            data: &self.data,
            width: ctx.size.width,
            height: ctx.size.height,
            min_color: resolve(self.min_color, "sparkline--min-color"),
            max_color: resolve(self.max_color, "sparkline--max-color"),
            summary: &*self.summary,
        };
        Content::Lines(renderable.render())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plain(data: &[f64], width: i32, height: i32) -> Vec<String> {
        SparklineRenderable {
            data,
            width,
            height,
            min_color: Color::rgb(0, 255, 0),
            max_color: Color::rgb(255, 0, 0),
            summary: &summary_max,
        }
        .render()
        .iter()
        .map(|s| s.plain_text())
        .collect()
    }

    #[test]
    fn buckets_partition() {
        let d = [1.0, 2.0, 3.0, 4.0];
        let b = SparklineRenderable::buckets(&d, 2);
        assert_eq!(b, vec![&[1.0, 2.0][..], &[3.0, 4.0][..]]);
        assert_eq!(SparklineRenderable::buckets(&d, 8).len(), 4);
    }

    #[test]
    fn renders_bars() {
        assert_eq!(plain(&[], 4, 1), vec!["▁▁▁▁"]);
        assert_eq!(plain(&[5.0], 3, 1), vec!["███"]);
        let d: Vec<f64> = (0..8).map(|v| v as f64).collect();
        assert_eq!(plain(&d, 8, 1), vec!["▁▂▃▄▅▆▇█"]);
        // Two rows: the lower row is filled where the upper row has bars.
        let rows = plain(&[0.0, 1.0], 2, 2);
        assert_eq!(rows, vec![" █", "▁█"]);
    }
}

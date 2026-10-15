//! `LoadingIndicator` (port of `textual/widgets/_loading_indicator.py`): five pulsing dots,
//! refreshed at 16 fps. Input events that reach it are stopped (so it can cover and "disable"
//! widgets beneath it).

use crate::context::Ctx;
use crate::events::Event;
use crate::message::Flow;
use crate::widget::{Content, RenderCtx, Widget};
use rustual_render::{Color, Style, Text};
use std::time::Duration;
use tokio::time::Instant;

const DEFAULT_CSS: &str = "
LoadingIndicator {
    width: 100%;
    height: 100%;
    min-height: 1;
    content-align: center middle;
    color: $primary;
    text-style: not reverse;
}
LoadingIndicator.-textual-loading-indicator {
    layer: _loading;
    background: $boost;
    dock: top;
}
";

/// An animated loading indicator.
#[derive(Debug, Clone)]
pub struct LoadingIndicator {
    start: Instant,
}

impl Default for LoadingIndicator {
    fn default() -> Self {
        Self::new()
    }
}

impl LoadingIndicator {
    pub fn new() -> Self {
        LoadingIndicator {
            start: Instant::now(),
        }
    }

    /// Seconds since the indicator was mounted.
    pub fn elapsed(&self) -> f64 {
        self.start.elapsed().as_secs_f64()
    }
}

/// Linear gradient through `(position, color)` stops (positions ascending in `0..=1`).
pub(crate) fn gradient_color(stops: &[(f64, Color)], position: f64) -> Color {
    let position = position.clamp(0.0, 1.0);
    let Some(first) = stops.first() else {
        return Color::default();
    };
    if position <= first.0 {
        return first.1;
    }
    for pair in stops.windows(2) {
        let ((p1, c1), (p2, c2)) = (pair[0], pair[1]);
        if position <= p2 {
            let f = if p2 > p1 {
                (position - p1) / (p2 - p1)
            } else {
                1.0
            };
            return c1.blend(c2, f as f32);
        }
    }
    stops[stops.len() - 1].1
}

impl Widget for LoadingIndicator {
    fn type_names(&self) -> &'static [&'static str] {
        &["LoadingIndicator"]
    }

    fn default_css(&self) -> &'static str {
        DEFAULT_CSS
    }

    fn on_mount(&mut self, ctx: &mut Ctx) {
        self.start = Instant::now();
        // Textual: `auto_refresh = 1 / 16` (the runtime repaints after each tick).
        ctx.set_interval(Duration::from_secs_f64(1.0 / 16.0), |_: &mut Self, _| {});
    }

    fn on_event(&mut self, event: &Event, _ctx: &mut Ctx) -> Flow {
        // Prevent input events from bubbling (widgets in a loading state are inert).
        if event.mouse().is_some() || event.key().is_some() {
            return Flow::Stop;
        }
        Flow::Continue
    }

    fn render(&self, ctx: &RenderCtx) -> Content {
        let elapsed = self.elapsed();
        let speed = 0.8;
        let background = ctx.background();
        let color = ctx.base_style().fg.unwrap_or(Color::WHITE);
        let stops = [
            (0.0, background.blend(color, 0.1)),
            (0.7, color),
            (1.0, color.lighten(0.1)),
        ];
        let mut text = Text::new();
        for dot in 0..5 {
            let blend = (elapsed * speed - dot as f64 / 8.0).rem_euclid(1.0);
            let c = gradient_color(&stops, (1.0 - blend).powi(2));
            let s = if dot < 4 { "● " } else { "●" };
            text.push(s, Style::new().fg(c));
        }
        Content::Text(text)
    }
}

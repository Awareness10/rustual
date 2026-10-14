//! `ProgressBar` (port of `textual/widgets/_progress_bar.py`, `textual/renderables/bar.py`
//! and `textual/eta.py`).
//!
//! A `ProgressBar` composes a [`Bar`] (`#bar`), a [`PercentageStatus`] (`#percentage`) and an
//! [`EtaStatus`] (`#eta`). Textual data-binds the parent's reactives into the children; here the
//! children read the parent `ProgressBar`'s state while rendering, so updating the progress bar
//! (e.g. `ctx.query_one::<ProgressBar>("#pb").unwrap().advance(1.0)`) is all that's needed.
//!
//! With `total == None` the bar is indeterminate and animates (a bouncing highlight, refreshed at
//! 15 fps by an interval that only runs while indeterminate).

use crate::app::TimerId;
use crate::context::Ctx;
use crate::widget::{Child, Content, RenderCtx, Widget};
use std::time::Duration;
use textual_render::{Gradient, Span, Strip, Style, Text};
use tokio::time::Instant;

const PROGRESS_BAR_CSS: &str = "
ProgressBar {
    width: auto;
    height: 1;
    layout: horizontal;
}
";

const BAR_CSS: &str = "
Bar {
    width: 32;
    height: 1;

    &> .bar--bar {
        color: $primary;
        background: $surface;
    }
    &> .bar--indeterminate {
        color: $error;
        background: $surface;
    }
    &> .bar--complete {
        color: $success;
        background: $surface;
    }
}
";

const PERCENTAGE_CSS: &str = "
PercentageStatus {
    width: 5;
    height: auto;
    content-align-horizontal: right;
}
";

const ETA_CSS: &str = "
EtaStatus {
    width: 9;
    height: auto;
    content-align-horizontal: right;
}
";

/// Python's `round()` (ties to even).
fn py_round(x: f64) -> f64 {
    x.round_ties_even()
}

// ---------------------------------------------------------------------------------- Bar renderable

/// Thin horizontal bar with a portion highlighted, using half-cell glyphs (`━`, `╸`, `╺`).
/// Port of `textual.renderables.bar.Bar`.
#[derive(Debug, Clone, PartialEq)]
pub struct BarRenderable {
    pub highlight_range: (f64, f64),
    pub highlight_style: Style,
    pub background_style: Style,
    pub width: i32,
}

impl BarRenderable {
    pub const HALF_BAR_LEFT: &'static str = "╺";
    pub const BAR: &'static str = "━";
    pub const HALF_BAR_RIGHT: &'static str = "╸";

    pub fn new(
        highlight_range: (f64, f64),
        highlight_style: Style,
        background_style: Style,
        width: i32,
    ) -> Self {
        BarRenderable {
            highlight_range,
            highlight_style,
            background_style,
            width,
        }
    }

    pub fn render(&self) -> Strip {
        self.render_with_gradient(None)
    }

    /// Render, coloring the highlighted portion with `gradient` (replaces the highlight color),
    /// like Textual's `Bar(gradient=...)`.
    pub fn render_with_gradient(&self, gradient: Option<&Gradient>) -> Strip {
        let width = self.width.max(0) as f64;
        let bg = &self.background_style;
        let hl = &self.highlight_style;
        let rep = |s: &str, n: f64| s.repeat(n.max(0.0) as usize);
        let (start, end) = self.highlight_range;
        let start = start.max(0.0);
        let end = end.min(width);
        let mut spans: Vec<Span> = Vec::new();
        let mut push = |text: String, style: &Style| {
            if !text.is_empty() {
                spans.push(Span::new(text, style.clone()));
            }
        };

        if (start == 0.0 && end == 0.0) || end < 0.0 || start > end {
            push(rep(Self::BAR, width), bg);
            return Strip::new(spans);
        }

        // Round start and end to the nearest half.
        let start = py_round(start * 2.0) / 2.0;
        let end = py_round(end * 2.0) / 2.0;
        let half_start = start - start.trunc() > 0.0;
        let half_end = end - end.trunc() > 0.0;

        // Initial non-highlighted portion.
        push(rep(Self::BAR, (start - 0.5).trunc()), bg);
        if !half_start && start > 0.0 {
            push(Self::HALF_BAR_RIGHT.to_string(), bg);
        }
        // The highlighted portion.
        let bar_width = end.trunc() - start.trunc();
        let mut highlight = if half_start {
            format!("{}{}", Self::HALF_BAR_LEFT, rep(Self::BAR, bar_width - 1.0))
        } else {
            rep(Self::BAR, bar_width)
        };
        if half_end {
            highlight.push_str(Self::HALF_BAR_RIGHT);
        }
        match gradient {
            None => push(highlight, hl),
            Some(g) => {
                // Textual's `_apply_gradient`.
                let max_width = width - 1.0;
                let n = highlight.chars().count();
                for (offset, ch) in highlight.chars().enumerate() {
                    let color = if max_width <= 0.0 {
                        g.get_color(0.0)
                    } else {
                        g.get_color((n - offset) as f64 / max_width)
                    };
                    let style = Style {
                        fg: Some(color),
                        ..hl.clone()
                    };
                    push(ch.to_string(), &style);
                }
            }
        }
        // The non-highlighted tail.
        if !half_end && end - width != 0.0 {
            push(Self::HALF_BAR_LEFT.to_string(), bg);
        }
        push(rep(Self::BAR, width.trunc() - end.trunc() - 1.0), bg);
        Strip::new(spans)
    }
}

// ---------------------------------------------------------------------------------- ETA

/// Speed / time-to-completion estimation (port of `textual.eta.ETA`).
#[derive(Debug, Clone, PartialEq)]
pub struct Eta {
    estimation_period: f64,
    max_extrapolate: f64,
    samples: Vec<(f64, f64)>,
    add_count: usize,
}

impl Default for Eta {
    fn default() -> Self {
        Eta::new(60.0, 30.0)
    }
}

impl Eta {
    pub fn new(estimation_period: f64, extrapolate_period: f64) -> Self {
        Eta {
            estimation_period,
            max_extrapolate: extrapolate_period,
            samples: vec![(0.0, 0.0)],
            add_count: 0,
        }
    }

    pub fn reset(&mut self) {
        self.samples.clear();
    }

    /// Add a sample: `progress` is a ratio (0 = start, 1 = complete) at `time` seconds.
    pub fn add_sample(&mut self, time: f64, progress: f64) {
        if self.samples.last().map(|s| s.1 > progress).unwrap_or(false) {
            // Progress went backwards: start over.
            self.reset();
        }
        self.samples.push((time, progress));
        self.add_count += 1;
        if self.add_count.is_multiple_of(100) {
            self.prune();
        }
    }

    fn prune(&mut self) {
        if self.samples.len() <= 10 {
            return;
        }
        let prune_time = self.samples[self.samples.len() - 1].0 - self.estimation_period;
        let index = self.bisect_left(prune_time);
        self.samples.drain(..index);
    }

    /// `bisect.bisect_left(samples, (time, 0))`.
    fn bisect_left(&self, time: f64) -> usize {
        self.samples
            .partition_point(|s| s.0 < time || (s.0 == time && s.1 < 0.0))
    }

    fn progress_at(&self, time: f64) -> (f64, f64) {
        let index = self.bisect_left(time);
        if index >= self.samples.len() {
            return self.samples[self.samples.len() - 1];
        }
        if index == 0 {
            return self.samples[0];
        }
        let (t1, p1) = self.samples[index - 1];
        let (t2, p2) = self.samples[index];
        let factor = (time - t1) / (t2 - t1);
        (time, p1 + (p2 - p1) * factor)
    }

    /// Progress per second, if it can be calculated.
    pub fn speed(&self) -> Option<f64> {
        if self.samples.len() < 2 {
            return None;
        }
        let (recent_time, p2) = self.samples[self.samples.len() - 1];
        let (start_time, p1) = self.progress_at(recent_time - self.estimation_period);
        if recent_time - start_time < 1.0 {
            return None;
        }
        let delta = recent_time - start_time;
        Some(if delta != 0.0 { (p2 - p1) / delta } else { 0.0 })
    }

    /// Estimated seconds until completion at `time`.
    pub fn get_eta(&self, time: f64) -> Option<i64> {
        let speed = self.speed().filter(|s| *s != 0.0)?;
        let (recent_time, recent_progress) = *self.samples.last()?;
        let remaining = 1.0 - recent_progress;
        if remaining <= 0.0 {
            return Some(0);
        }
        let since = self.max_extrapolate.min(time - recent_time);
        let eta = 1.0f64.max((remaining - speed * since) / speed);
        Some(eta.ceil() as i64)
    }
}

// ---------------------------------------------------------------------------------- ProgressBar

/// A progress bar with percentage and ETA displays. Progress is measured in "steps".
#[derive(Debug, Clone)]
pub struct ProgressBar {
    total: Option<f64>,
    progress: f64,
    show_bar: bool,
    show_percentage: bool,
    show_eta: bool,
    eta: Eta,
    display_eta: Option<i64>,
    clock_start: Instant,
    anim_timer: Option<TimerId>,
    gradient: Option<Gradient>,
}

impl Default for ProgressBar {
    fn default() -> Self {
        Self::new(None)
    }
}

impl ProgressBar {
    /// A progress bar with `total` steps (`None` = indeterminate).
    pub fn new(total: Option<f64>) -> Self {
        ProgressBar {
            total: total.map(|t| t.max(0.0)),
            progress: 0.0,
            show_bar: true,
            show_percentage: true,
            show_eta: true,
            eta: Eta::default(),
            display_eta: None,
            clock_start: Instant::now(),
            anim_timer: None,
            gradient: None,
        }
    }

    /// Color the bar with a gradient instead of the CSS highlight color (Textual: `gradient=`).
    pub fn gradient(mut self, gradient: Gradient) -> Self {
        self.gradient = Some(gradient);
        self
    }

    pub fn set_gradient(&mut self, gradient: Option<Gradient>) {
        self.gradient = gradient;
    }

    pub fn get_gradient(&self) -> Option<&Gradient> {
        self.gradient.as_ref()
    }

    pub fn show_bar(mut self, show: bool) -> Self {
        self.show_bar = show;
        self
    }

    pub fn show_percentage(mut self, show: bool) -> Self {
        self.show_percentage = show;
        self
    }

    pub fn show_eta(mut self, show: bool) -> Self {
        self.show_eta = show;
        self
    }

    pub fn total(&self) -> Option<f64> {
        self.total
    }

    pub fn progress(&self) -> f64 {
        self.progress
    }

    /// Completed fraction in `0.0..=1.0`, or `None` while the total is unknown.
    /// A total of zero reports `1.0`.
    pub fn percentage(&self) -> Option<f64> {
        match self.total {
            Some(t) if t > 0.0 => Some((self.progress / t).clamp(0.0, 1.0)),
            Some(_) => Some(1.0),
            None => None,
        }
    }

    /// Estimated seconds to completion (refreshed on updates and every second).
    pub fn eta(&self) -> Option<i64> {
        self.display_eta
    }

    /// Seconds since the bar was mounted (drives the ETA and the indeterminate animation).
    pub fn clock(&self) -> f64 {
        self.clock_start.elapsed().as_secs_f64()
    }

    /// Advance the progress by `steps`.
    pub fn advance(&mut self, steps: f64) {
        self.apply(None, None, Some(steps));
    }

    /// Set both the total (`None` = indeterminate) and the progress.
    pub fn update(&mut self, total: Option<f64>, progress: f64) {
        self.apply(Some(total), Some(progress), None);
    }

    pub fn set_total(&mut self, total: Option<f64>) {
        self.apply(Some(total), None, None);
    }

    pub fn set_progress(&mut self, progress: f64) {
        self.apply(None, Some(progress), None);
    }

    /// `ProgressBar.update` from Textual (`None` = parameter not given).
    fn apply(&mut self, total: Option<Option<f64>>, progress: Option<f64>, advance: Option<f64>) {
        let now = self.clock();
        if let Some(total) = total {
            let total = total.map(|t| t.max(0.0));
            if total.is_none() || total != self.total {
                self.eta.reset();
            }
            self.total = total;
        }
        if let Some(p) = progress {
            self.progress = p;
            self.add_sample(now);
        }
        if let Some(a) = advance {
            self.progress += a;
            self.add_sample(now);
        }
        self.display_eta = match self.total {
            None => None,
            Some(_) => self.eta.get_eta(now),
        };
    }

    fn add_sample(&mut self, now: f64) {
        if let Some(t) = self.total.filter(|t| *t > 0.0) {
            self.eta.add_sample(now, self.progress / t);
        }
    }

    /// Run the 15 fps animation timer only while indeterminate.
    fn sync_animation(&mut self, ctx: &mut Ctx) {
        let indeterminate = self.percentage().is_none();
        match (indeterminate, self.anim_timer) {
            (true, None) => {
                let id = ctx.set_interval(
                    Duration::from_secs_f64(1.0 / 15.0),
                    |pb: &mut ProgressBar, ctx| {
                        if pb.percentage().is_some() {
                            if let Some(id) = pb.anim_timer.take() {
                                ctx.cancel_timer(id);
                            }
                        }
                    },
                );
                self.anim_timer = Some(id);
            }
            (false, Some(id)) => {
                ctx.cancel_timer(id);
                self.anim_timer = None;
            }
            _ => {}
        }
    }
}

impl Widget for ProgressBar {
    fn type_names(&self) -> &'static [&'static str] {
        &["ProgressBar"]
    }

    fn default_css(&self) -> &'static str {
        PROGRESS_BAR_CSS
    }

    fn compose(&mut self) -> Vec<Child> {
        let mut out = Vec::new();
        if self.show_bar {
            out.push(Child::from(Bar::new()).id("bar"));
        }
        if self.show_percentage {
            out.push(Child::from(PercentageStatus).id("percentage"));
        }
        if self.show_eta {
            out.push(Child::from(EtaStatus).id("eta"));
        }
        out
    }

    fn on_mount(&mut self, ctx: &mut Ctx) {
        self.clock_start = Instant::now();
        self.apply(None, None, None);
        ctx.set_interval(Duration::from_secs(1), |pb: &mut ProgressBar, ctx| {
            pb.apply(None, None, None);
            pb.sync_animation(ctx);
        });
        self.sync_animation(ctx);
    }
}

fn parent_progress_bar<'a>(ctx: &RenderCtx<'a>) -> Option<&'a ProgressBar> {
    let dom = ctx.dom;
    dom.parent(ctx.node)
        .and_then(|p| dom.widget::<ProgressBar>(p))
}

/// The bar portion of a progress bar. Component classes: `bar--bar`, `bar--complete`,
/// `bar--indeterminate`. Inside a [`ProgressBar`] it shows the parent's progress; standalone it
/// shows its own [`Bar::set_percentage`].
#[derive(Debug, Clone)]
pub struct Bar {
    percentage: Option<f64>,
    clock_start: Instant,
    gradient: Option<Gradient>,
}

impl Default for Bar {
    fn default() -> Self {
        Self::new()
    }
}

impl Bar {
    pub fn new() -> Self {
        Bar {
            percentage: None,
            clock_start: Instant::now(),
            gradient: None,
        }
    }

    /// Color the bar with a gradient (standalone bars; inside a `ProgressBar` the parent's
    /// gradient is used).
    pub fn gradient(mut self, gradient: Option<Gradient>) -> Self {
        self.gradient = gradient;
        self
    }

    pub fn set_percentage(&mut self, percentage: Option<f64>) {
        self.percentage = percentage;
    }

    /// Render a bar for `percentage` (or the indeterminate animation frame at `time` seconds).
    pub fn render_bar(ctx: &RenderCtx, percentage: Option<f64>, time: f64) -> Strip {
        Self::render_bar_with_gradient(ctx, percentage, time, None)
    }

    /// Like [`Bar::render_bar`], coloring the highlight with an optional gradient.
    pub fn render_bar_with_gradient(
        ctx: &RenderCtx,
        percentage: Option<f64>,
        time: f64,
        gradient: Option<&Gradient>,
    ) -> Strip {
        let width = ctx.size.width;
        let w = width as f64;
        match percentage {
            Some(p) => {
                let style = if p < 1.0 {
                    ctx.component_style("bar--bar")
                } else {
                    ctx.component_style("bar--complete")
                };
                BarRenderable::new(
                    (0.0, w * p),
                    Style {
                        fg: style.fg,
                        ..Style::new()
                    },
                    Style {
                        fg: style.bg,
                        ..Style::new()
                    },
                    width,
                )
                .render_with_gradient(gradient)
            }
            None => {
                let highlighted = 0.25 * w;
                let total_imaginary = w + highlighted;
                let speed = 30.0; // cells per second
                let mut start = if total_imaginary > 0.0 {
                    (speed * time).rem_euclid(2.0 * total_imaginary)
                } else {
                    0.0
                };
                if start > total_imaginary {
                    start = 2.0 * total_imaginary - start;
                }
                start -= highlighted;
                let end = start + highlighted;
                let style = ctx.component_style("bar--indeterminate");
                BarRenderable::new(
                    (start.max(0.0), end.min(w)),
                    Style {
                        fg: style.fg,
                        ..Style::new()
                    },
                    Style {
                        fg: style.bg,
                        ..Style::new()
                    },
                    width,
                )
                .render_with_gradient(gradient)
            }
        }
    }
}

impl Widget for Bar {
    fn type_names(&self) -> &'static [&'static str] {
        &["Bar"]
    }

    fn default_css(&self) -> &'static str {
        BAR_CSS
    }

    fn on_mount(&mut self, _ctx: &mut Ctx) {
        self.clock_start = Instant::now();
    }

    fn render(&self, ctx: &RenderCtx) -> Content {
        let (percentage, time, gradient) = match parent_progress_bar(ctx) {
            Some(pb) => (pb.percentage(), pb.clock(), pb.gradient.as_ref()),
            None => (
                self.percentage,
                self.clock_start.elapsed().as_secs_f64(),
                self.gradient.as_ref(),
            ),
        };
        Content::Lines(vec![Self::render_bar_with_gradient(
            ctx, percentage, time, gradient,
        )])
    }
}

/// The percentage label of a progress bar (`--%` while indeterminate).
#[derive(Debug, Clone, Default)]
pub struct PercentageStatus;

impl Widget for PercentageStatus {
    fn type_names(&self) -> &'static [&'static str] {
        &["PercentageStatus"]
    }

    fn default_css(&self) -> &'static str {
        PERCENTAGE_CSS
    }

    fn render(&self, ctx: &RenderCtx) -> Content {
        let text = match parent_progress_bar(ctx).and_then(|pb| pb.percentage()) {
            None => "--%".to_string(),
            Some(p) => format!("{}%", py_round(p * 100.0) as i64),
        };
        Content::Text(Text::plain(text))
    }
}

/// The ETA label of a progress bar (`--:--:--` when unknown). Textual: `ETAStatus`.
#[derive(Debug, Clone, Default)]
pub struct EtaStatus;

/// Format an ETA in seconds like Textual's `ETAStatus`.
pub fn format_eta(eta: Option<i64>) -> String {
    let Some(eta) = eta else {
        return "--:--:--".to_string();
    };
    let (minutes, seconds) = (eta.div_euclid(60), eta.rem_euclid(60));
    let (hours, minutes) = (minutes.div_euclid(60), minutes.rem_euclid(60));
    if hours > 999_999 {
        "+999999h".to_string()
    } else if hours > 99 {
        format!("{hours}h")
    } else {
        format!("{hours:02}:{minutes:02}:{seconds:02}")
    }
}

impl Widget for EtaStatus {
    fn type_names(&self) -> &'static [&'static str] {
        &["EtaStatus", "ETAStatus"]
    }

    fn default_css(&self) -> &'static str {
        ETA_CSS
    }

    fn render(&self, ctx: &RenderCtx) -> Content {
        let eta = parent_progress_bar(ctx).and_then(|pb| pb.eta());
        Content::Text(Text::plain(format_eta(eta)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bar(range: (f64, f64), width: i32) -> String {
        BarRenderable::new(range, Style::new(), Style::new(), width)
            .render()
            .plain_text()
    }

    #[test]
    fn bar_glyphs() {
        assert_eq!(bar((0.0, 0.0), 6), "━━━━━━");
        assert_eq!(bar((0.0, 6.0), 6), "━━━━━━");
        assert_eq!(bar((0.0, 3.0), 6), "━━━╺━━");
        assert_eq!(bar((0.0, 2.5), 6), "━━╸━━━");
        assert_eq!(bar((2.0, 4.0), 6), "━╸━━╺━");
        assert_eq!(bar((1.5, 4.0), 6), "━╺━━╺━");
        for r in [(0.0, 2.5), (1.5, 4.0), (2.0, 4.0), (0.0, 3.0)] {
            assert_eq!(bar(r, 6).chars().count(), 6, "{r:?}");
        }
    }

    #[test]
    fn eta_estimates() {
        let mut eta = Eta::default();
        eta.reset();
        eta.add_sample(0.0, 0.0);
        assert_eq!(eta.get_eta(0.0), None);
        eta.add_sample(10.0, 0.5);
        assert_eq!(eta.speed(), Some(0.05));
        assert_eq!(eta.get_eta(10.0), Some(10));
        assert_eq!(eta.get_eta(15.0), Some(5));
        eta.add_sample(20.0, 1.0);
        assert_eq!(eta.get_eta(20.0), Some(0));
    }

    #[test]
    fn eta_format() {
        assert_eq!(format_eta(None), "--:--:--");
        assert_eq!(format_eta(Some(3725)), "01:02:05");
        assert_eq!(format_eta(Some(100 * 3600)), "100h");
    }
}

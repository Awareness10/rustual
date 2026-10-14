//! `Switch` (port of `textual/widgets/_switch.py`): an on/off slider rendered like a
//! horizontal scrollbar whose thumb sits left (off) or right (on). The slider slides over 0.3s
//! (`in_out_cubic`, animation level `basic`); the `-on` class is set when it reaches the right.

use crate::animation::{AnimationLevel, Easing};
use crate::binding::Binding;
use crate::context::Ctx;
use crate::events::Event;
use crate::message::Flow;
use crate::widget::{Content, RenderCtx, Widget};
use std::time::Duration;
use textual_render::{Span, Strip, Style};

const DEFAULT_CSS: &str = r#"
Switch {
    border: tall $border-blurred;
    background: $surface;
    height: auto;
    width: auto;
    pointer: pointer;

    padding: 0 2;
    &.-on .switch--slider {
        color: $success;
    }
    & .switch--slider {
        color: $panel;
        background: $panel-darken-2;
    }
    &:hover {
        & > .switch--slider {
            color: $panel-lighten-1
        }
        &.-on > .switch--slider {
            color: $success-lighten-1;
        }
    }
    &:focus {
        border: tall $border;
        background-tint: $foreground 5%;
    }

    &:light {
        &.-on .switch--slider {
            color: $success;
        }
        & .switch--slider {
            color: $primary 15%;
            background: $panel-darken-2;
        }
        &:hover {
            & > .switch--slider {
                color: $primary 25%;
            }
            &.-on > .switch--slider {
                color: $success-lighten-1;
            }
        }
    }
}
"#;

/// Glyphs for the partial ends of a horizontal bar (Textual's `ScrollBarRender.HORIZONTAL_BARS`).
const HORIZONTAL_BARS: [&str; 8] = ["▉", "▊", "▋", "▌", "▍", "▎", "▏", " "];

/// Posted when the switch value changes (Textual: `Switch.Changed`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Changed {
    pub value: bool,
    pub switch_id: Option<String>,
}

/// A switch representing a boolean value. Toggle with `enter`, `space` or a click.
#[derive(Debug, Clone, Default)]
pub struct Switch {
    value: bool,
    /// 0.0 (off) ..= 1.0 (on).
    slider_position: f32,
    animate: bool,
}

impl Switch {
    pub fn new() -> Self {
        Switch {
            value: false,
            slider_position: 0.0,
            animate: true,
        }
    }

    /// Initial value (no `Changed` message is posted for it).
    pub fn value(mut self, value: bool) -> Self {
        self.value = value;
        self.slider_position = if value { 1.0 } else { 0.0 };
        self
    }

    /// Whether toggling slides the slider (default true).
    pub fn animate(mut self, animate: bool) -> Self {
        self.animate = animate;
        self
    }

    pub fn get_value(&self) -> bool {
        self.value
    }

    /// Slider position: 0.0 (off) ..= 1.0 (on); in between while sliding.
    pub fn slider_position(&self) -> f32 {
        self.slider_position
    }

    /// Set the value, posting [`Changed`] if it differs.
    pub fn set_value(&mut self, value: bool, ctx: &mut Ctx) {
        if self.value == value {
            return;
        }
        self.value = value;
        let target = if value { 1.0 } else { 0.0 };
        // Only animate with this switch's own ctx (the animation is keyed by the ctx node).
        let own_ctx = ctx
            .dom()
            .get(ctx.id())
            .map(|n| n.is_type("Switch"))
            .unwrap_or(false);
        if self.animate && own_ctx && ctx.animation_level() != AnimationLevel::None {
            ctx.animate_value_with::<Switch>(
                "slider_position",
                self.slider_position,
                target,
                Duration::from_millis(300),
                Easing::DEFAULT,
                AnimationLevel::Basic,
                |sw, v, ctx| {
                    sw.slider_position = v;
                    ctx.set_class(v == 1.0, "-on");
                },
                None,
            );
        } else {
            self.slider_position = target;
            ctx.set_class(self.slider_position == 1.0, "-on");
        }
        let switch_id = ctx.css_id(ctx.id()).map(String::from);
        ctx.post_message(Changed { value, switch_id });
    }

    pub fn toggle(&mut self, ctx: &mut Ctx) {
        let v = !self.value;
        self.set_value(v, ctx);
    }
}

/// Render a horizontal bar of `size` cells (port of `ScrollBarRender.render_bar`).
fn render_bar(
    size: i32,
    virtual_size: f32,
    window_size: f32,
    position: f32,
    back: Option<textual_render::Color>,
    bar: Option<textual_render::Color>,
) -> Strip {
    let len_bars = HORIZONTAL_BARS.len() as i32;
    let back_style = Style {
        bg: back,
        ..Style::default()
    };
    if size <= 0 {
        return Strip::default();
    }
    if window_size == 0.0 || virtual_size == 0.0 || size as f32 == virtual_size {
        return Strip::blank(size, back_style);
    }
    let bar_ratio = virtual_size / size as f32;
    let thumb_size = (window_size / bar_ratio).max(1.0);
    let position_ratio = position / (virtual_size - window_size);
    let position = (size as f32 - thumb_size) * position_ratio;
    let start = (position * len_bars as f32) as i32;
    let end = start + (thumb_size * len_bars as f32).ceil() as i32;
    let (start_index, start_bar) = (start.max(0) / len_bars, start.max(0) % len_bars);
    let (end_index, end_bar) = (end.max(0) / len_bars, end.max(0) % len_bars);

    // The thumb is drawn as reversed blanks in the bar color; use the bar color as background.
    let thumb_style = Style {
        bg: bar,
        ..Style::default()
    };
    let mut cells: Vec<(&str, Style)> = (0..size)
        .map(|i| {
            if i >= start_index && i < end_index {
                (" ", thumb_style.clone())
            } else {
                (" ", back_style.clone())
            }
        })
        .collect();
    if start_index < size {
        let ch = HORIZONTAL_BARS[(len_bars - 1 - start_bar) as usize];
        if ch != " " {
            // Horizontal: bgcolor=back, color=bar, reverse.
            cells[start_index as usize] = (
                ch,
                Style {
                    fg: back,
                    bg: bar,
                    ..Style::default()
                },
            );
        }
    }
    if end_index < size {
        let ch = HORIZONTAL_BARS[(len_bars - 1 - end_bar) as usize];
        if ch != " " {
            cells[end_index as usize] = (
                ch,
                Style {
                    fg: bar,
                    bg: back,
                    ..Style::default()
                },
            );
        }
    }
    Strip::new(cells.into_iter().map(|(c, s)| Span::new(c, s)).collect()).simplify()
}

impl Widget for Switch {
    fn type_names(&self) -> &'static [&'static str] {
        &["Switch"]
    }

    fn default_css(&self) -> &'static str {
        DEFAULT_CSS
    }

    fn can_focus(&self) -> bool {
        true
    }

    fn on_mount(&mut self, ctx: &mut Ctx) {
        ctx.set_class(self.slider_position == 1.0, "-on");
    }

    fn bindings(&self) -> Vec<Binding> {
        vec![Binding::new("enter,space", "toggle_switch", "Toggle").hidden()]
    }

    fn run_action(&mut self, action: &str, _args: &[String], ctx: &mut Ctx) -> bool {
        if action == "toggle_switch" {
            self.toggle(ctx);
            return true;
        }
        false
    }

    fn on_event(&mut self, event: &Event, ctx: &mut Ctx) -> Flow {
        if let Event::Click(_) = event {
            self.toggle(ctx);
            return Flow::Stop;
        }
        Flow::Continue
    }

    fn content_width(&self, _ctx: &RenderCtx) -> Option<i32> {
        Some(4)
    }

    fn content_height(&self, _ctx: &RenderCtx, _width: i32) -> Option<i32> {
        Some(1)
    }

    fn render(&self, ctx: &RenderCtx) -> Content {
        let style = ctx.component_style("switch--slider");
        let strip = render_bar(
            ctx.size.width,
            100.0,
            50.0,
            self.slider_position * 50.0,
            style.bg,
            style.fg,
        );
        let mut lines = vec![strip];
        for _ in 1..ctx.size.height.max(1) {
            lines.push(Strip::blank(ctx.size.width, Style::default()));
        }
        Content::Lines(lines)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use textual_render::Color;

    #[test]
    fn bar_positions() {
        let back = Some(Color::parse("#000000").unwrap());
        let bar = Some(Color::parse("#00ff00").unwrap());
        let off = render_bar(4, 100.0, 50.0, 0.0, back, bar);
        let bgs: Vec<_> = off
            .spans
            .iter()
            .map(|s| (s.text.len(), s.style.bg))
            .collect();
        assert_eq!(bgs, vec![(2, bar), (2, back)]);
        let on = render_bar(4, 100.0, 50.0, 50.0, back, bar);
        let bgs: Vec<_> = on
            .spans
            .iter()
            .map(|s| (s.text.len(), s.style.bg))
            .collect();
        assert_eq!(bgs, vec![(2, back), (2, bar)]);
    }
}

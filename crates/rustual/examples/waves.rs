//! Streaming waves: four sparklines animated at 60 frames a second, new samples flowing in from
//! the right. Every color comes from the theme (`$primary`, `$secondary`, `$accent`,
//! `$success` → `$warning`), so switching themes recolors the whole app.
//!
//! Run with `cargo run -p rustual --example waves`. `t` switches to the next theme (or pick one
//! from the command palette, `ctrl+p`), `+` / `-` change the speed, space pauses, `ctrl+q`
//! quits. The same app in Python: `crates/rustual-py/python/rustual/demo/waves.py`.

use rustual::prelude::*;
use std::f64::consts::TAU;
use std::time::Duration;

const CSS: &str = "
#waves { padding: 0 1; }
.wave { height: 1fr; padding: 0 1; border-subtitle-color: $text-muted; }
.wave Sparkline { height: 1fr; }

.sine { border: round $primary; border-title-color: $primary; }
.sine Sparkline > .sparkline--min-color { color: $primary 20%; }
.sine Sparkline > .sparkline--max-color { color: $primary; }

.beats { border: round $secondary; border-title-color: $secondary; }
.beats Sparkline > .sparkline--min-color { color: $secondary 20%; }
.beats Sparkline > .sparkline--max-color { color: $secondary; }

.packets { border: round $accent; border-title-color: $accent; }
.packets Sparkline > .sparkline--min-color { color: $accent 20%; }
.packets Sparkline > .sparkline--max-color { color: $accent; }

.tide { border: round $success; border-title-color: $warning; }
.tide Sparkline > .sparkline--min-color { color: $success; }
.tide Sparkline > .sparkline--max-color { color: $warning; }
";

/// Frames per second of the animation.
const FPS: f64 = 60.0;
/// Speeds in columns per second (`+` / `-`).
const SPEEDS: [f64; 6] = [3.0, 6.0, 12.0, 24.0, 48.0, 96.0];
const DEFAULT_SPEED: usize = 2;

/// One wave: `f(x, t)` is the height at position `x` (in columns) at time `t` (in seconds).
struct Wave {
    name: &'static str,
    title: &'static str,
    formula: &'static str,
    f: fn(f64, f64) -> f64,
}

const WAVES: [Wave; 4] = [
    Wave {
        name: "sine",
        title: "Sine",
        formula: "sin(2πx/24)",
        f: |x, _| (TAU * x / 24.0).sin(),
    },
    Wave {
        name: "beats",
        title: "Beats",
        formula: "sin(2πx/11) + sin(2πx/13)",
        f: |x, _| (TAU * x / 11.0).sin() + (TAU * x / 13.0).sin(),
    },
    Wave {
        name: "packets",
        title: "Wave packets",
        formula: "cos(2πx/6) · e^(−d²/40)",
        f: |x, _| {
            let d = x.rem_euclid(48.0) - 24.0;
            (TAU * x / 6.0).cos() * (-d * d / 40.0).exp()
        },
    },
    Wave {
        name: "tide",
        title: "Tide",
        formula: "sin(2πx/40) + ½sin(2πx/17 + t) + ¼sin(2πx/7)",
        f: |x, t| {
            (TAU * x / 40.0).sin() + 0.5 * (TAU * x / 17.0 + t).sin() + 0.25 * (TAU * x / 7.0).sin()
        },
    },
];

/// The samples shown in a sparkline `width` columns wide when the stream is at `offset`.
fn samples(wave: &Wave, offset: f64, time: f64, width: usize) -> Vec<f64> {
    (0..width)
        .map(|i| (wave.f)(offset + i as f64, time))
        .collect()
}

/// The themes `t` cycles through (the ANSI themes use the terminal's own colors).
fn theme_cycle(names: Vec<String>) -> Vec<String> {
    let mut names: Vec<String> = names.into_iter().filter(|n| !n.contains("ansi")).collect();
    names.sort();
    names
}

struct Waves {
    /// Seconds of animation so far (frozen while paused).
    time: f64,
    /// How far the stream has moved, in columns.
    offset: f64,
    speed: usize,
    paused: bool,
}

impl Waves {
    fn new() -> Self {
        Waves {
            time: 0.0,
            offset: 0.0,
            speed: DEFAULT_SPEED,
            paused: false,
        }
    }

    fn tick(&mut self, ctx: &mut Ctx) {
        if !self.paused {
            self.time += 1.0 / FPS;
            self.offset += SPEEDS[self.speed] / FPS;
        }
        self.refresh(ctx);
    }

    fn refresh(&self, ctx: &mut Ctx) {
        for wave in &WAVES {
            let selector = format!("#{}", wave.name);
            let Some(id) = ctx.query_id(&selector) else {
                continue;
            };
            // Zero before the first layout: the next frame fills it.
            let width = ctx.region_of(id).width.max(0) as usize;
            let data = samples(wave, self.offset, self.time, width);
            ctx.with_id::<Sparkline, _>(id, |s, _| s.set_data(data));
        }
        let state = if self.paused { " · paused" } else { "" };
        let sub_title = format!("{} · {} columns/s{state}", ctx.theme(), SPEEDS[self.speed]);
        ctx.set_sub_title(&sub_title);
    }
}

impl Widget for Waves {
    fn compose(&mut self) -> Vec<Child> {
        let panels: Vec<Child> = WAVES
            .iter()
            .map(|w| {
                Vertical::new()
                    .classes(&format!("wave {}", w.name))
                    .with(compose![Sparkline::new(Vec::<f64>::new()).id(w.name)])
                    .border_title(w.title)
                    .border_subtitle(w.formula)
            })
            .collect();
        compose![
            Header::new(),
            Vertical::new().id("waves").with(panels),
            Footer::new(),
        ]
    }

    fn on_mount(&mut self, ctx: &mut Ctx) {
        ctx.set_title("Waves");
        self.refresh(ctx);
        ctx.set_interval(Duration::from_secs_f64(1.0 / FPS), |w: &mut Waves, ctx| {
            w.tick(ctx)
        });
    }

    fn bindings(&self) -> Vec<Binding> {
        vec![
            Binding::new("t", "next_theme", "Theme"),
            Binding::new("plus", "speed(1)", "Faster"),
            Binding::new("minus", "speed(-1)", "Slower"),
            Binding::new("space", "pause", "Pause"),
            Binding::new("ctrl+q", "quit", "Quit").priority(),
        ]
    }

    fn run_action(&mut self, action: &str, args: &[String], ctx: &mut Ctx) -> bool {
        match action {
            "next_theme" => {
                let names = theme_cycle(ctx.theme_names());
                let next = names
                    .iter()
                    .position(|n| n == ctx.theme())
                    .map_or(0, |i| (i + 1) % names.len());
                ctx.set_theme(&names[next]);
            }
            "speed" => {
                let step: i32 = args.first().and_then(|a| a.parse().ok()).unwrap_or(1);
                let last = SPEEDS.len() as i32 - 1;
                self.speed = (self.speed as i32 + step).clamp(0, last) as usize;
            }
            "pause" => self.paused = !self.paused,
            _ => return false,
        }
        self.refresh(ctx);
        true
    }
}

fn app() -> App {
    App::new(Waves::new()).css(CSS)
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    app().run().await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data(p: &Pilot, name: &str) -> Vec<f64> {
        p.query_one::<Sparkline>(&format!("#{name}"))
            .unwrap()
            .data()
            .to_vec()
    }

    /// Foreground colors of the cells in the sparkline `name`.
    fn colors(p: &mut Pilot, name: &str) -> Vec<Color> {
        let id = p
            .with_ctx(None, |ctx| ctx.query_id(&format!("#{name}")))
            .unwrap();
        let region = p.with_ctx(None, |ctx| ctx.region_of(id));
        let canvas = p.canvas();
        let mut out = Vec::new();
        for y in region.y..region.y + region.height {
            for x in region.x..region.x + region.width {
                let cell = canvas.get(x, y).unwrap();
                if cell.symbol.trim() != "" {
                    out.push(cell.fg);
                }
            }
        }
        out
    }

    fn theme_color(theme: &str, variable: &str) -> Color {
        let theme = rustual::css::Theme::builtin()
            .into_iter()
            .find(|t| t.name == theme)
            .unwrap();
        Color::parse(&theme.to_variables()[variable]).unwrap()
    }

    #[tokio::test(start_paused = true)]
    async fn waves_stream_in_from_the_right() {
        let mut p = app().test(Size::new(100, 30)).await;
        assert!(p.css_errors().is_empty(), "{:?}", p.css_errors());
        let text = p.screen_text();
        for title in ["Sine", "Beats", "Wave packets", "Tide"] {
            assert!(text.contains(title), "{text}");
        }
        assert!(text.contains("textual-dark · 12 columns/s"), "{text}");
        p.wait(Duration::from_secs_f64(1.0 / FPS)).await;

        // One sample per column, filled from the formula.
        let width = p.with_ctx(None, |ctx| {
            let id = ctx.query_id("#sine").unwrap();
            ctx.region_of(id).width as usize
        });
        let before = data(&p, "sine");
        assert_eq!(before.len(), width);

        // After a second at 12 columns/s, the samples have moved 12 columns to the left.
        p.wait(Duration::from_secs(1)).await;
        let after = data(&p, "sine");
        for i in 0..width - 12 {
            assert!((after[i] - before[i + 12]).abs() < 1e-6, "column {i}");
        }
        let w = p.app::<Waves>().unwrap();
        assert!((w.time - 1.0).abs() < 0.1, "{}", w.time);
    }

    #[tokio::test(start_paused = true)]
    async fn colors_follow_the_theme() {
        let mut p = app().test(Size::new(100, 30)).await;
        p.wait(Duration::from_millis(100)).await;
        // The highest bars use $primary / $accent, the lowest $success, in every theme.
        for theme in ["textual-dark", "nord"] {
            p.with_ctx(None, |ctx| ctx.set_theme(theme));
            p.wait(Duration::from_millis(100)).await;
            assert!(colors(&mut p, "sine").contains(&theme_color(theme, "primary")));
            assert!(colors(&mut p, "packets").contains(&theme_color(theme, "accent")));
            assert!(colors(&mut p, "tide").contains(&theme_color(theme, "success")));
        }
        let nord = colors(&mut p, "sine");

        // `t` moves on to the next theme (alphabetically, skipping the ANSI ones).
        p.press(&["t"]).await;
        p.wait(Duration::from_millis(100)).await;
        assert_eq!(p.theme(), "rose-pine");
        assert!(p.screen_text().contains("rose-pine · 12 columns/s"));
        assert_ne!(colors(&mut p, "sine"), nord);
    }

    #[tokio::test(start_paused = true)]
    async fn pause_and_speed() {
        let mut p = app().test(Size::new(100, 30)).await;
        p.press(&["space"]).await;
        let frozen = data(&p, "tide");
        p.wait(Duration::from_millis(500)).await;
        assert_eq!(data(&p, "tide"), frozen);
        assert!(p.screen_text().contains("· paused"));

        p.press(&["space", "plus", "plus", "plus", "plus", "plus"])
            .await;
        assert!(p.screen_text().contains("96 columns/s"));
        p.press(&["minus"]).await;
        assert!(p.screen_text().contains("48 columns/s"));
    }

    #[test]
    fn theme_cycle_skips_ansi_themes() {
        let names = theme_cycle(vec!["nord".into(), "ansi-dark".into(), "dracula".into()]);
        assert_eq!(names, ["dracula", "nord"]);
    }
}

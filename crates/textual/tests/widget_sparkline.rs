//! Sparkline: bars, colors from component classes, multi-row, updates.

use textual::prelude::*;

struct T {
    data: Vec<f64>,
}
impl Widget for T {
    fn compose(&mut self) -> Vec<Child> {
        compose![Sparkline::new(self.data.clone()).id("s")]
    }
}

#[tokio::test]
async fn sparkline_renders_and_colors() {
    let data: Vec<f64> = (0..8).map(f64::from).collect();
    let mut pilot = App::new(T { data })
        .css("#s { width: 8; }")
        .test(Size::new(20, 3))
        .await;
    assert!(pilot.css_errors().is_empty(), "{:?}", pilot.css_errors());
    assert_eq!(pilot.screen_text().lines().next().unwrap(), "▁▂▃▄▅▆▇█");
    let c = pilot.canvas();
    let lo = c.get(0, 0).unwrap().fg;
    let hi = c.get(7, 0).unwrap().fg;
    assert_ne!(lo, hi);
    // Min color is $primary at 30% over the background: closer to the background than max.
    let bg = c.get(0, 0).unwrap().bg;
    let dist = |a: Color, b: Color| {
        (a.r as i32 - b.r as i32).abs()
            + (a.g as i32 - b.g as i32).abs()
            + (a.b as i32 - b.b as i32).abs()
    };
    assert!(dist(lo, bg) < dist(hi, bg));
}

#[tokio::test]
async fn custom_colors_and_summary() {
    struct C;
    impl Widget for C {
        fn compose(&mut self) -> Vec<Child> {
            compose![Sparkline::new(vec![1.0, 9.0, 1.0, 9.0])
                .min_color(Color::parse("#00ff00").unwrap())
                .max_color(Color::parse("#ff0000").unwrap())
                .summary_function(|v: &[f64]| v.iter().sum::<f64>() / v.len() as f64)
                .id("s")]
        }
    }
    let mut pilot = App::new(C)
        .css("#s { width: 2; }")
        .test(Size::new(10, 2))
        .await;
    // Two buckets of [1, 9]: mean 5 each → same bar.
    let line = pilot.screen_text().lines().next().unwrap().to_string();
    let chars: Vec<char> = line.chars().collect();
    assert_eq!(chars.len(), 2);
    assert_eq!(chars[0], chars[1]);
    let c = pilot.canvas();
    assert_eq!(c.get(0, 0).unwrap().fg.hex(), "#7F7F00");
}

#[tokio::test]
async fn multi_row_and_update() {
    struct U;
    impl Widget for U {
        fn compose(&mut self) -> Vec<Child> {
            compose![Sparkline::new(vec![]).id("s")]
        }
        fn bindings(&self) -> Vec<Binding> {
            vec![Binding::new("p", "push", "Push")]
        }
        fn run_action(&mut self, a: &str, _: &[String], ctx: &mut Ctx) -> bool {
            if a == "push" {
                let s = ctx.query_one::<Sparkline>("#s").unwrap();
                let n = s.data().len() as f64;
                s.push(n, 3);
                return true;
            }
            false
        }
    }
    let mut pilot = App::new(U)
        .css("#s { width: 3; height: 2; }")
        .test(Size::new(10, 3))
        .await;
    let text = pilot.screen_text();
    assert_eq!(text.lines().nth(1).unwrap(), "▁▁▁");
    pilot.press(&["p", "p", "p", "p"]).await;
    assert_eq!(
        pilot.query_one::<Sparkline>("#s").unwrap().data(),
        &[1.0, 2.0, 3.0]
    );
    let text = pilot.screen_text();
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines[0], "  █", "{text}");
    assert_eq!(lines[1], "▁██");
}

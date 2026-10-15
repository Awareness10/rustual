//! LoadingIndicator: animated dots; input is stopped.

use rustual::prelude::*;
use std::time::Duration;

#[derive(Default)]
struct T {
    clicks: usize,
}
impl Widget for T {
    fn compose(&mut self) -> Vec<Child> {
        compose![LoadingIndicator::new().id("li")]
    }
    fn on_event(&mut self, e: &Event, _ctx: &mut Ctx) -> Flow {
        if let Event::Click(_) = e {
            self.clicks += 1;
        }
        Flow::Continue
    }
}

fn dot_colors(pilot: &mut Pilot) -> Vec<Color> {
    let c = pilot.canvas();
    // 9 cells "● ● ● ● ●" centered in 20x5 → x = 5..14, y = 2.
    (0..5).map(|i| c.get(5 + i * 2, 2).unwrap().fg).collect()
}

#[tokio::test(start_paused = true)]
async fn dots_are_centered_and_animate() {
    let mut pilot = App::new(T::default()).test(Size::new(20, 5)).await;
    assert!(pilot.css_errors().is_empty(), "{:?}", pilot.css_errors());
    let text = pilot.screen_text();
    assert_eq!(text.lines().nth(2).unwrap(), "     ● ● ● ● ●", "{text}");
    let a = dot_colors(&mut pilot);
    // Dots are at different phases.
    assert_ne!(a[0], a[2]);
    pilot.wait(Duration::from_millis(250)).await;
    let b = dot_colors(&mut pilot);
    assert_ne!(a, b);
}

#[tokio::test]
async fn input_is_stopped() {
    let mut pilot = App::new(T::default()).test(Size::new(20, 5)).await;
    pilot.click("#li").await;
    assert_eq!(pilot.app::<T>().unwrap().clicks, 0);
}

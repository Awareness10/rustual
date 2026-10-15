//! ProgressBar gradient tests.

use rustual::prelude::*;
use rustual::render::Gradient;

struct G(f64);

impl Widget for G {
    fn compose(&mut self) -> Vec<Child> {
        let g = Gradient::from_colors(&[Color::rgb(255, 0, 0), Color::rgb(0, 0, 255)]).unwrap();
        let mut pb = ProgressBar::new(Some(10.0)).gradient(g);
        pb.set_progress(self.0);
        compose![pb.id("pb"), ProgressBar::new(Some(10.0)).id("plain")]
    }
}

#[tokio::test]
async fn gradient_colors_the_highlight() {
    let mut p = App::new(G(10.0)).test(Size::new(60, 6)).await;
    assert!(p.css_errors().is_empty(), "{:?}", p.css_errors());
    assert!(p.screen_text().contains("━━━━"));
    let c = p.canvas();
    // Bar is 32 cells wide at x 0..32 of row 0.
    let first = c.get(0, 0).unwrap().fg;
    let mid = c.get(15, 0).unwrap().fg;
    let last = c.get(31, 0).unwrap().fg;
    assert_ne!(first, last);
    assert_ne!(first, mid);
    // Textual applies the gradient from the head backwards: the last cell is nearest the
    // start color (red), the first nearest the end color (blue).
    assert!(last.r > last.b, "{last:?}");
    assert!(first.b > first.r, "{first:?}");
    assert!(p
        .query_one::<ProgressBar>("#pb")
        .unwrap()
        .get_gradient()
        .is_some());
}

#[tokio::test]
async fn gradient_half_progress_keeps_background() {
    let mut p = App::new(G(5.0)).test(Size::new(60, 6)).await;
    let c = p.canvas();
    // The un-highlighted tail uses the CSS background color, not the gradient.
    let tail = c.get(30, 0).unwrap().fg;
    let plain_tail = c.get(30, 1).unwrap().fg;
    assert_eq!(tail, plain_tail);
    assert_ne!(c.get(2, 0).unwrap().fg, c.get(2, 1).unwrap().fg);
}

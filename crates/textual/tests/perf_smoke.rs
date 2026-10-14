//! Guards against pathological slowness in the restyle → layout → paint pipeline.
use std::time::Instant;
use textual::prelude::*;

struct Many(usize);
impl Widget for Many {
    fn compose(&mut self) -> Vec<Child> {
        let items: Vec<Child> = (0..self.0)
            .map(|i| Label::new(format!("item [b]{i}[/]")).classes(if i % 2 == 0 { "even" } else { "odd" }))
            .collect();
        compose![VerticalScroll::new().with(items)]
    }
}

#[tokio::test]
async fn thousand_widgets_frame_time() {
    let mut pilot = App::new(Many(1000))
        .css(".even { background: $boost; } VerticalScroll > Label:hover { color: red; }")
        .test(Size::new(100, 40))
        .await;
    let start = Instant::now();
    let n = 10;
    for _ in 0..n {
        pilot.canvas();
    }
    let per_frame = start.elapsed() / n;
    println!("frame time with 1000 widgets: {per_frame:?}");
    // Debug builds are slow; this only catches quadratic blowups.
    assert!(per_frame.as_millis() < 1000, "{per_frame:?}");
}

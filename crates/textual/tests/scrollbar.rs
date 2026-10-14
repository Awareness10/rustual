//! Tests for scrollbar interaction (drag, track click, hover/active colors), mouse capture,
//! implicit drag capture and click chains.

use std::time::Duration;
use textual::prelude::*;

/// Records mouse events it receives; captures the mouse on MouseDown if `capture`.
#[derive(Default)]
struct Pad {
    capture: bool,
    events: Vec<String>,
    chains: Vec<u32>,
}

impl Widget for Pad {
    fn type_names(&self) -> &'static [&'static str] {
        &["Pad"]
    }
    fn on_event(&mut self, event: &Event, ctx: &mut Ctx) -> Flow {
        match event {
            Event::MouseDown(m) => {
                if self.capture {
                    ctx.capture_mouse();
                }
                self.events.push(format!("down {},{}", m.x, m.y));
            }
            Event::MouseMove(m) => self.events.push(format!("move {},{}", m.x, m.y)),
            Event::MouseUp(_) => {
                self.events.push("up".into());
                ctx.release_mouse();
            }
            Event::Click(_) => self.chains.push(ctx.click_chain()),
            _ => return Flow::Continue,
        }
        Flow::Stop
    }
}

struct TestApp;

impl Widget for TestApp {
    fn compose(&mut self) -> Vec<Child> {
        compose![
            ScrollableContainer::new()
                .id("sc")
                .with((0..100).map(|i| Child::from(Label::new(format!("line {i}"))))),
            Pad {
                capture: true,
                ..Default::default()
            }
            .id("a"),
            Pad::default().id("b"),
        ]
    }
}

const CSS: &str = "
#sc {
    height: 10;
    scrollbar-color: #ff0000;
    scrollbar-color-hover: #00ff00;
    scrollbar-color-active: #0000ff;
    scrollbar-background: #000000;
    scrollbar-background-hover: #000000;
    scrollbar-background-active: #000000;
}
Pad { height: 3; }
";

async fn pilot() -> Pilot {
    App::new(TestApp).css(CSS).test(Size::new(30, 20)).await
}

/// x of the vertical scrollbar of #sc.
fn bar_x(p: &Pilot) -> i32 {
    let id = p.query("#sc")[0];
    p.dom().get(id).unwrap().layout.content_region.right()
}

fn offset_y(p: &Pilot) -> i32 {
    let id = p.query("#sc")[0];
    p.dom().get(id).unwrap().scroll_offset.y
}

fn red() -> Color {
    Color::rgb(255, 0, 0)
}

#[tokio::test(start_paused = true)]
async fn thumb_is_painted_with_eighth_cell_ends() {
    let mut p = pilot().await;
    assert!(p.css_errors().is_empty(), "{:?}", p.css_errors());
    let x = bar_x(&p);
    let c = p.canvas();
    // 10 rows of 100: a 1-cell thumb at the top.
    assert_eq!(c.get(x, 0).unwrap().bg, red());
    assert_eq!(c.get(x + 1, 0).unwrap().bg, red(), "2 cells wide");
    assert_eq!(c.get(x, 1).unwrap().bg, Color::rgb(0, 0, 0));
    // Scroll a bit: the thumb straddles two cells with ⅛-cell glyphs.
    p.with::<ScrollableContainer, _>("#sc", |_w, ctx| ctx.scroll_to(0, 5))
        .await;
    let c = p.canvas();
    let glyphs: String = (0..3)
        .map(|y| c.get(x, y).unwrap().symbol.clone())
        .collect();
    assert!(
        glyphs.chars().any(|g| "▁▂▃▄▅▆▇".contains(g)),
        "partial thumb glyphs: {glyphs:?}"
    );
}

#[tokio::test(start_paused = true)]
async fn dragging_thumb_scrolls_proportionally() {
    let mut p = pilot().await;
    let x = bar_x(&p);
    p.mouse_down_at(x, 0).await;
    // Grabbed: active color.
    assert_eq!(p.canvas().get(x, 0).unwrap().bg, Color::rgb(0, 0, 255));
    p.mouse_move_to(x, 3).await;
    // 3 cells * (100 virtual / 10 window) = 30 lines.
    assert_eq!(offset_y(&p), 30);
    p.mouse_move_to(x, 5).await;
    assert_eq!(offset_y(&p), 50);
    // Dragging keeps working outside the bar, and clamps.
    p.mouse_move_to(0, 19).await;
    assert_eq!(offset_y(&p), 90);
    p.mouse_up_at(0, 19).await;
    p.mouse_move_to(x, 2).await;
    assert_eq!(offset_y(&p), 90, "released");
    // Pads never saw any of it.
    assert!(p.query_one::<Pad>("#a").unwrap().events.is_empty());
    assert!(p.query_one::<Pad>("#b").unwrap().events.is_empty());
}

#[tokio::test(start_paused = true)]
async fn track_click_pages() {
    let mut p = pilot().await;
    let x = bar_x(&p);
    p.click_at(x, 8).await;
    // Animated page down (Textual's `scroll_page_down`).
    p.wait(Duration::from_millis(500)).await;
    assert_eq!(offset_y(&p), 10);
    p.click_at(x, 8).await;
    p.wait(Duration::from_millis(500)).await;
    assert_eq!(offset_y(&p), 20);
    // Above the thumb: page up.
    p.click_at(x, 0).await;
    p.wait(Duration::from_millis(500)).await;
    assert_eq!(offset_y(&p), 10);
}

#[tokio::test(start_paused = true)]
async fn scrollbar_hover_color() {
    let mut p = pilot().await;
    let x = bar_x(&p);
    assert_eq!(p.canvas().get(x, 0).unwrap().bg, red());
    p.hover_at(x, 5).await;
    assert_eq!(p.canvas().get(x, 0).unwrap().bg, Color::rgb(0, 255, 0));
    p.hover_at(1, 1).await;
    assert_eq!(p.canvas().get(x, 0).unwrap().bg, red());
}

#[tokio::test(start_paused = true)]
async fn capture_routes_events_to_capturer() {
    let mut p = pilot().await;
    let a = p.query("#a")[0];
    let b = p.query("#b")[0];
    let ra = p.dom().get(a).unwrap().layout.region;
    let rb = p.dom().get(b).unwrap().layout.region;
    p.mouse_down_at(ra.x + 1, ra.y + 1).await;
    // Plain move (no button) over #b still goes to the capturing #a.
    p.hover_at(rb.x + 2, rb.y + 1).await;
    p.mouse_up_at(rb.x + 2, rb.y + 1).await;
    let ev = &p.query_one::<Pad>("#a").unwrap().events;
    assert_eq!(
        ev,
        &vec![
            "move 1,1".to_string(),
            "down 1,1".into(),
            format!("move 2,{}", rb.y - ra.y + 1),
            "up".into()
        ]
    );
    // Released on MouseUp: moves go to the widget under the pointer again.
    p.hover_at(rb.x + 3, rb.y + 1).await;
    assert_eq!(p.query_one::<Pad>("#a").unwrap().events.len(), 4);
    assert_eq!(p.query_one::<Pad>("#b").unwrap().events, vec!["move 3,1"]);
}

#[tokio::test(start_paused = true)]
async fn drag_moves_go_to_mouse_down_widget() {
    let mut p = pilot().await;
    let a = p.query("#a")[0];
    let b = p.query("#b")[0];
    let ra = p.dom().get(a).unwrap().layout.region;
    let rb = p.dom().get(b).unwrap().layout.region;
    // #b does not capture; a drag starting on it still delivers moves to it.
    p.mouse_down_at(rb.x + 1, rb.y).await;
    p.mouse_move_to(ra.x + 1, ra.y).await;
    p.mouse_up_at(ra.x + 1, ra.y).await;
    let evb = &p.query_one::<Pad>("#b").unwrap().events;
    assert_eq!(evb[1], "down 1,0");
    assert_eq!(evb[2], format!("move 1,{}", ra.y - rb.y));
    // The release happens over #a (not captured): MouseUp goes there.
    let eva = &p.query_one::<Pad>("#a").unwrap().events;
    assert_eq!(eva, &vec!["up".to_string()]);
}

#[tokio::test(start_paused = true)]
async fn click_chain_counts_double_clicks() {
    let mut p = pilot().await;
    let b = p.query("#b")[0];
    let rb = p.dom().get(b).unwrap().layout.region;
    p.click_at(rb.x, rb.y).await;
    p.click_at(rb.x, rb.y).await;
    p.click_at(rb.x, rb.y).await;
    p.wait(Duration::from_millis(600)).await;
    p.click_at(rb.x, rb.y).await;
    p.click_at(rb.x + 1, rb.y).await;
    assert_eq!(
        p.query_one::<Pad>("#b").unwrap().chains,
        vec![1, 2, 3, 1, 1]
    );
}

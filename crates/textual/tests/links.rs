use std::sync::{Arc, Mutex};
use textual::prelude::*;
use textual::widgets::link::{self, set_url_opener};

#[derive(Default)]
struct L {
    opened: Vec<String>,
    clicks: usize,
}

struct Clicky;
impl Widget for Clicky {
    fn render(&self, _ctx: &RenderCtx) -> Content {
        Content::markup("[@click=app.bell]Ring[/] plain")
    }
    fn on_event(&mut self, e: &Event, ctx: &mut Ctx) -> Flow {
        if let Event::Click(_) = e {
            ctx.post_message(Clicked);
        }
        Flow::Continue
    }
}
struct Clicked;

impl Widget for L {
    fn compose(&mut self) -> Vec<Child> {
        compose![
            Clicky.id("clicky").style("height: 1;"),
            Link::new("Docs").url("https://example.com/docs").id("link"),
        ]
    }
    fn on_message(&mut self, m: &Message, _ctx: &mut Ctx) -> Flow {
        if let Some(o) = m.get::<link::Opened>() {
            self.opened.push(o.url.clone());
        }
        if m.is::<Clicked>() {
            self.clicks += 1;
        }
        Flow::Continue
    }
}

#[tokio::test]
async fn click_markup_runs_action_and_link_opens_url() {
    let calls: Arc<Mutex<Vec<String>>> = Arc::default();
    let c = calls.clone();
    set_url_opener(Some(Arc::new(move |u: &str| {
        c.lock().unwrap().push(u.to_string());
        Ok(())
    })));

    let mut p = App::new(L::default()).test(Size::new(40, 10)).await;
    assert!(p.css_errors().is_empty(), "{:?}", p.css_errors());
    let id = p.query("#clicky")[0];
    let r = p.dom().get(id).unwrap().layout.content_region;

    // Clicking the linked word rings the bell; the widget still sees the click.
    p.click_at(r.x + 1, r.y).await;
    assert_eq!(p.bell_count(), 1);
    assert_eq!(p.app::<L>().unwrap().clicks, 1);
    // Clicking plain text does not.
    p.click_at(r.x + 7, r.y).await;
    assert_eq!(p.bell_count(), 1);
    assert_eq!(p.app::<L>().unwrap().clicks, 2);

    assert!(p.screen_text().contains("Docs"));
    p.click("#link").await;
    assert_eq!(
        p.app::<L>().unwrap().opened,
        vec!["https://example.com/docs"]
    );
    // Enter on the focused link opens it too.
    p.press(&["enter"]).await;
    assert_eq!(p.app::<L>().unwrap().opened.len(), 2);
    assert_eq!(calls.lock().unwrap().len(), 2);
    set_url_opener(None);
}

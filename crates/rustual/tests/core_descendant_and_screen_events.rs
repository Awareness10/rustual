//! `DescendantFocus` / `DescendantBlur` (bubbling to ancestors) and `ScreenSuspend` /
//! `ScreenResume` (sent to screens as they stop / start being active).

use rustual::events::{DescendantBlur, DescendantFocus, ScreenResume, ScreenSuspend};
use rustual::prelude::*;
use std::sync::{Arc, Mutex};

type Log = Arc<Mutex<Vec<String>>>;

struct Recorder {
    name: &'static str,
    log: Log,
}
impl Widget for Recorder {
    fn on_message(&mut self, msg: &Message, ctx: &mut Ctx) -> Flow {
        let name = |id| ctx.css_id(id).unwrap_or("?").to_string();
        let entry = if let Some(m) = msg.get::<DescendantFocus>() {
            format!("{} focus {}", self.name, name(m.widget))
        } else if let Some(m) = msg.get::<DescendantBlur>() {
            format!("{} blur {}", self.name, name(m.widget))
        } else if msg.is::<ScreenResume>() {
            format!("{} resume", self.name)
        } else if msg.is::<ScreenSuspend>() {
            format!("{} suspend", self.name)
        } else {
            return Flow::Continue;
        };
        self.log.lock().unwrap().push(entry);
        Flow::Continue
    }
}

struct Root {
    log: Log,
}
impl Widget for Root {
    fn compose(&mut self) -> Vec<Child> {
        compose![Child::from(Recorder {
            name: "outer",
            log: self.log.clone()
        })
        .id("outer")
        .with(compose![Input::new().id("a"), Input::new().id("b"),])]
    }
}

#[tokio::test]
async fn descendant_focus_and_blur_bubble_to_ancestors() {
    let log: Log = Default::default();
    let mut p = App::new(Root { log: log.clone() })
        .test(Size::new(40, 10))
        .await;
    p.press(&["tab"]).await;
    let entries = log.lock().unwrap().clone();
    // Auto-focus put #a in focus; tab moves it to #b.
    assert!(
        entries.contains(&"outer focus a".to_string()),
        "{entries:?}"
    );
    assert!(entries.contains(&"outer blur a".to_string()), "{entries:?}");
    assert!(
        entries.contains(&"outer focus b".to_string()),
        "{entries:?}"
    );
}

struct Modal {
    log: Log,
}
impl Widget for Modal {
    fn type_names(&self) -> &'static [&'static str] {
        &["Modal", "Screen"]
    }
    fn on_message(&mut self, msg: &Message, ctx: &mut Ctx) -> Flow {
        Recorder {
            name: "modal",
            log: self.log.clone(),
        }
        .on_message(msg, ctx)
    }
}

#[tokio::test]
async fn screens_get_suspend_and_resume() {
    let log: Log = Default::default();
    let mut p = App::new(Recorder {
        name: "app",
        log: log.clone(),
    })
    .test(Size::new(40, 10))
    .await;
    let log2 = log.clone();
    p.with::<Recorder, _>("App", |_, ctx| ctx.push_screen(Modal { log: log2 }))
        .await;
    p.with::<Recorder, _>("App", |_, ctx| ctx.pop_screen())
        .await;
    let entries = log.lock().unwrap().clone();
    // The events don't bubble, so only the modal (a recorder) logs its own: resumed when
    // pushed, suspended when popped.
    assert_eq!(
        entries,
        vec!["modal resume", "modal suspend"],
        "{entries:?}"
    );
}

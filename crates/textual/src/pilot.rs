//! [`Pilot`]: drive an app headlessly in tests (like Textual's `App.run_test()` pilot).
//!
//! ```ignore
//! #[tokio::test]
//! async fn press_button() {
//!     let mut pilot = App::new(MyApp::default()).test(Size::new(80, 24)).await;
//!     pilot.click("#ok").await;
//!     pilot.press(&["tab", "enter"]).await;
//!     assert!(pilot.screen_text().contains("Pressed"));
//! }
//! ```

use crate::app::{AppState, MouseKind, Signal};
use crate::context::Ctx;
use crate::dom::{Dom, NodeId};
use crate::driver::DriverEvent;
use crate::events::{KeyEvent, MouseButton, MouseEvent};
use crate::widget::Widget;
use std::any::Any;
use std::time::Duration;
use textual_render::{Canvas, Size};
use tokio::sync::mpsc;

pub struct Pilot {
    state: Box<AppState>,
    rx: mpsc::UnboundedReceiver<Signal>,
    /// Button held by `mouse_down_at` (sent with moves, like a terminal reports drags).
    held: Option<MouseButton>,
}

impl Pilot {
    pub(crate) fn new(state: Box<AppState>, rx: mpsc::UnboundedReceiver<Signal>) -> Self {
        Pilot {
            state,
            rx,
            held: None,
        }
    }

    /// Process everything pending (queue, worker signals, due timers), then lay out.
    pub async fn pause(&mut self) {
        for _ in 0..1000 {
            tokio::task::yield_now().await;
            let mut did = self.state.process_queue();
            while let Ok(sig) = self.rx.try_recv() {
                self.state.handle_signal(sig);
                did = true;
            }
            did |= self.state.fire_due_timers();
            if !did {
                break;
            }
        }
        self.state.layout();
    }

    /// Wait (in real or tokio-paused time) and then process what became due.
    /// Timers fire at each of their deadlines along the way, like in the real event loop, and
    /// running animations get a final frame at exactly the target time (so animated values are
    /// deterministic under `#[tokio::test(start_paused = true)]`).
    pub async fn wait(&mut self, d: Duration) {
        let target = tokio::time::Instant::now() + d;
        while let Some(next) = self.state.next_deadline().filter(|n| *n <= target) {
            tokio::time::sleep_until(next).await;
            self.pause().await;
        }
        tokio::time::sleep_until(target).await;
        self.state.tick_animations(target, true);
        self.pause().await;
    }

    /// Number of running animations.
    pub fn animations_running(&self) -> usize {
        self.state.animator.len()
    }

    /// Wait until worker results arrive (up to `timeout`), processing them.
    pub async fn wait_for_signal(&mut self, timeout: Duration) -> bool {
        match tokio::time::timeout(timeout, self.rx.recv()).await {
            Ok(Some(sig)) => {
                self.state.handle_signal(sig);
                self.pause().await;
                true
            }
            _ => false,
        }
    }

    /// Press keys in order (key names as in [`KeyEvent`], e.g. `"tab"`, `"ctrl+c"`, `"a"`).
    pub async fn press(&mut self, keys: &[&str]) {
        for k in keys {
            self.state
                .handle_driver_event(DriverEvent::Key(KeyEvent::new(*k)));
            self.pause().await;
        }
    }

    /// Type text one character at a time.
    pub async fn type_text(&mut self, text: &str) {
        for c in text.chars() {
            let key = if c == ' ' {
                "space".to_string()
            } else {
                c.to_string()
            };
            self.state.handle_driver_event(DriverEvent::Key(KeyEvent {
                key,
                character: Some(c),
            }));
            self.pause().await;
        }
    }

    /// Simulate a bracketed paste of `text` (an [`crate::Event::Paste`] to the focused widget).
    pub async fn paste(&mut self, text: &str) {
        self.state
            .handle_driver_event(DriverEvent::Paste(text.to_string()));
        self.pause().await;
    }

    /// Press the left button at screen coordinates (without releasing).
    pub async fn mouse_down_at(&mut self, x: i32, y: i32) {
        self.mouse(MouseKind::Move, x, y, None);
        self.mouse(MouseKind::Down, x, y, Some(MouseButton::Left));
        self.held = Some(MouseButton::Left);
        self.pause().await;
    }

    /// Move the mouse to screen coordinates (a drag if a button is held).
    pub async fn mouse_move_to(&mut self, x: i32, y: i32) {
        let held = self.held;
        self.mouse(MouseKind::Move, x, y, held);
        self.pause().await;
    }

    /// Release the held button at screen coordinates.
    pub async fn mouse_up_at(&mut self, x: i32, y: i32) {
        let held = self.held.take().or(Some(MouseButton::Left));
        self.mouse(MouseKind::Up, x, y, held);
        self.pause().await;
    }

    /// Drag with the left button from one point to another (press, move, release).
    pub async fn drag(&mut self, from: (i32, i32), to: (i32, i32)) {
        self.mouse_down_at(from.0, from.1).await;
        self.mouse_move_to(to.0, to.1).await;
        self.mouse_up_at(to.0, to.1).await;
    }

    /// Move the mouse to screen coordinates without a button held (hover).
    pub async fn hover_at(&mut self, x: i32, y: i32) {
        self.mouse(MouseKind::Move, x, y, None);
        self.pause().await;
    }

    fn mouse(&mut self, kind: MouseKind, x: i32, y: i32, button: Option<MouseButton>) {
        let m = MouseEvent {
            x,
            y,
            screen_x: x,
            screen_y: y,
            button,
            ctrl: false,
            alt: false,
            shift: false,
        };
        self.state.handle_driver_event(DriverEvent::Mouse(kind, m));
    }

    /// Click at screen coordinates.
    pub async fn click_at(&mut self, x: i32, y: i32) {
        self.mouse(MouseKind::Move, x, y, None);
        self.mouse(MouseKind::Down, x, y, Some(MouseButton::Left));
        self.mouse(MouseKind::Up, x, y, Some(MouseButton::Left));
        self.pause().await;
    }

    /// Click the first widget matching `selector` at the center of its visible area. Returns false if not found / not visible.
    pub async fn click(&mut self, selector: &str) -> bool {
        let Some((x, y)) = self.center_of(selector) else {
            return false;
        };
        self.click_at(x, y).await;
        true
    }

    pub async fn hover(&mut self, selector: &str) -> bool {
        let Some((x, y)) = self.center_of(selector) else {
            return false;
        };
        self.mouse(MouseKind::Move, x, y, None);
        self.pause().await;
        true
    }

    pub async fn scroll_down_at(&mut self, x: i32, y: i32) {
        self.mouse(MouseKind::ScrollDown, x, y, None);
        self.pause().await;
    }

    pub async fn scroll_up_at(&mut self, x: i32, y: i32) {
        self.mouse(MouseKind::ScrollUp, x, y, None);
        self.pause().await;
    }

    pub async fn resize(&mut self, size: Size) {
        self.state.handle_driver_event(DriverEvent::Resize(size));
        self.pause().await;
    }

    fn center_of(&self, selector: &str) -> Option<(i32, i32)> {
        let id = self.query(selector).into_iter().next()?;
        let node = self.state.dom.get(id)?;
        if !node.layout.visible {
            return None;
        }
        let r = node.layout.clip;
        Some((r.x + r.width / 2, r.y + r.height / 2))
    }

    /// Node ids matching `selector` (active screen first, then whole DOM).
    pub fn query(&self, selector: &str) -> Vec<NodeId> {
        let mut ids = match self.state.active_screen() {
            Some(s) => self.state.dom.query_str(s, selector),
            None => Vec::new(),
        };
        for id in self.state.dom.query_str(self.state.root, selector) {
            if !ids.contains(&id) {
                ids.push(id);
            }
        }
        ids
    }

    pub fn query_one<W: Widget>(&self, selector: &str) -> Option<&W> {
        self.query(selector)
            .into_iter()
            .find_map(|id| self.state.dom.widget::<W>(id))
    }

    /// Run `f` with mutable access to a widget and a `Ctx` for it, then process the consequences.
    pub async fn with<W: Widget, R>(
        &mut self,
        selector: &str,
        f: impl FnOnce(&mut W, &mut Ctx) -> R,
    ) -> Option<R> {
        let id = self
            .query(selector)
            .into_iter()
            .find(|id| self.state.dom.widget::<W>(*id).is_some())?;
        let r = self
            .state
            .call_widget(id, |w, ctx| {
                (w as &mut dyn Any).downcast_mut::<W>().map(|w| f(w, ctx))
            })
            .flatten();
        self.pause().await;
        r
    }

    /// The app root widget.
    pub fn app<W: Widget>(&self) -> Option<&W> {
        self.state.dom.widget::<W>(self.state.root)
    }

    pub fn dom(&self) -> &Dom {
        &self.state.dom
    }

    /// CSS id of the focused widget.
    pub fn focused_id(&self) -> Option<String> {
        self.state
            .dom
            .focused
            .and_then(|f| self.state.dom.get(f))
            .and_then(|n| n.css_id.clone())
    }

    pub fn focused(&self) -> Option<NodeId> {
        self.state.dom.focused
    }

    pub fn has_exited(&self) -> bool {
        self.state.exit_requested
    }

    pub fn bell_count(&self) -> usize {
        self.state.bell_count
    }

    pub fn screen_count(&self) -> usize {
        self.state.screens.len()
    }

    pub fn title(&self) -> &str {
        &self.state.env.title
    }

    pub fn theme(&self) -> &str {
        &self.state.theme
    }

    pub fn css_errors(&self) -> &[String] {
        &self.state.css_errors
    }

    pub fn notifications(&self) -> Vec<String> {
        self.state
            .notifications
            .iter()
            .map(|n| n.message.clone())
            .collect()
    }

    /// Render the current frame.
    pub fn canvas(&mut self) -> Canvas {
        self.state.render_frame()
    }

    /// Current frame as text (trailing spaces trimmed per line).
    pub fn screen_text(&mut self) -> String {
        self.canvas().to_text()
    }
}

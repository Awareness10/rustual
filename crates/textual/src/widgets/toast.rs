//! `Toast`, `ToastHolder` and `ToastRack` (port of `textual/widgets/_toast.py`): notifications
//! (`ctx.notify`) shown stacked at the bottom-right of the screen.
//!
//! The runtime mounts one `ToastRack` as the last child of every screen. The rack is docked to
//! the bottom in its own layer (`_toastrack`), so it overlays content without taking space; it
//! and the `ToastHolder`s are `visibility: hidden` (not painted, transparent to the mouse), only
//! the `Toast`s themselves are visible.
//!
//! The rack syncs its toasts with the app's notifications when it mounts and whenever a
//! notification is added (the runtime posts [`Notified`](crate::app::Notified) to every rack).
//! A toast removes itself when its notification times out, or when clicked (which also
//! dismisses the notification).

use crate::app::{Notification, Notified, Severity};
use crate::context::Ctx;
use crate::events::Event;
use crate::message::{Flow, Message};
use crate::widget::{Child, Content, RenderCtx, Widget};
use textual_render::Text;
use tokio::time::Instant;

const TOAST_HOLDER_CSS: &str = "
ToastHolder {
    align-horizontal: right;
    width: 1fr;
    height: auto;
    visibility: hidden;
}
";

const TOAST_CSS: &str = "
Toast {
    width: 60;
    max-width: 50%;
    height: auto;
    margin-top: 1;
    visibility: visible;
    padding: 1 1;
    background: $panel-lighten-1;
    link-color: $foreground;
    link-style: underline;
    link-background-hover: $primary;
    link-color-hover: $foreground;
    link-style-hover: bold not underline;
}
.toast--title {
    text-style: bold;
    color: $foreground;
}
Toast.-information {
    border-left: outer $success;
}
Toast.-information .toast--title {
    color: $text-success;
}
Toast.-warning {
    border-left: outer $warning;
}
Toast.-warning .toast--title {
    color: $text-warning;
}
Toast.-error {
    border-left: outer $error;
}
Toast.-error .toast--title {
   color: $text-error;
}
";

const TOAST_RACK_CSS: &str = "
ToastRack {
    display: none;
    layer: _toastrack;
    width: 1fr;
    height: auto;
    dock: bottom;
    align: right bottom;
    visibility: hidden;
    layout: vertical;
    overflow-y: scroll;
    margin-bottom: 1;
}
";

fn severity_class(s: Severity) -> &'static str {
    match s {
        Severity::Information => "-information",
        Severity::Warning => "-warning",
        Severity::Error => "-error",
    }
}

/// Re-sync every toast rack in the app with the current notifications.
fn sync_all_racks(ctx: &mut Ctx) {
    for rack in ctx.query("ToastRack") {
        ctx.with_id::<ToastRack, _>(rack, |r, ctx| r.sync(ctx));
    }
}

/// Displays one notification. Click to dismiss.
#[derive(Debug, Clone)]
pub struct Toast {
    notification: Notification,
}

impl Toast {
    pub fn new(notification: Notification) -> Self {
        Toast { notification }
    }

    pub fn notification(&self) -> &Notification {
        &self.notification
    }

    /// Remove this toast (and its holder) and forget the notification.
    pub fn dismiss(&mut self, ctx: &mut Ctx) {
        ctx.app.unnotify(&self.notification);
        match ctx.parent() {
            Some(p) if ctx.dom().widget::<ToastHolder>(p).is_some() => ctx.remove(p),
            _ => ctx.remove_self(),
        }
        sync_all_racks(ctx);
    }
}

impl Widget for Toast {
    fn type_names(&self) -> &'static [&'static str] {
        &["Toast"]
    }

    fn default_css(&self) -> &'static str {
        TOAST_CSS
    }

    fn on_mount(&mut self, ctx: &mut Ctx) {
        ctx.add_class(severity_class(self.notification.severity));
        ctx.add_class("-textual-system");
        let left = self
            .notification
            .expires
            .saturating_duration_since(Instant::now());
        ctx.set_timer(left, |t: &mut Toast, ctx| t.dismiss(ctx));
    }

    fn on_event(&mut self, event: &Event, ctx: &mut Ctx) -> Flow {
        if let Event::Click(_) = event {
            self.dismiss(ctx);
            return Flow::Stop;
        }
        Flow::Continue
    }

    fn render(&self, ctx: &RenderCtx) -> Content {
        let n = &self.notification;
        let message = ctx.markup(&n.message);
        if n.title.is_empty() {
            return Content::Text(message);
        }
        let mut text = Text::styled(n.title.clone(), ctx.component_style("toast--title"));
        text.push("\n", Default::default());
        text.append(message);
        Content::Text(text)
    }
}

/// Holds a single toast (controls its alignment within the rack).
#[derive(Debug, Clone, Default)]
pub struct ToastHolder;

impl Widget for ToastHolder {
    fn type_names(&self) -> &'static [&'static str] {
        &["ToastHolder"]
    }

    fn default_css(&self) -> &'static str {
        TOAST_HOLDER_CSS
    }
}

/// Container for toasts, mounted on every screen by the runtime.
#[derive(Debug, Clone, Default)]
pub struct ToastRack;

impl ToastRack {
    pub fn new() -> Self {
        ToastRack
    }

    /// Show the app's current (unexpired) notifications as toasts (Textual: `ToastRack.show`).
    pub fn sync(&mut self, ctx: &mut Ctx) {
        let now = Instant::now();
        let notifications: Vec<Notification> = ctx
            .app
            .notifications
            .iter()
            .filter(|n| n.expires > now)
            .cloned()
            .collect();
        let me = ctx.id();
        ctx.set_display(me, !notifications.is_empty());

        // Remove stale toasts; collect the notifications already shown.
        let mut shown: Vec<Notification> = Vec::new();
        for holder in ctx.children() {
            let toast = ctx
                .dom()
                .children(holder)
                .iter()
                .find_map(|c| ctx.dom().widget::<Toast>(*c))
                .map(|t| t.notification.clone());
            match toast {
                Some(n) if !ctx.app.pending_removals.contains(&holder) => {
                    let wanted = notifications.iter().filter(|x| **x == n).count();
                    let have = shown.iter().filter(|x| **x == n).count();
                    if have < wanted {
                        shown.push(n);
                    } else {
                        ctx.remove(holder);
                    }
                }
                _ => ctx.remove(holder),
            }
        }

        // Mount toasts for new notifications.
        let mut added = false;
        for n in notifications {
            if let Some(i) = shown.iter().position(|s| *s == n) {
                shown.remove(i);
                continue;
            }
            ctx.mount(Child::from(ToastHolder).with([Child::from(Toast::new(n))]));
            added = true;
        }
        if added {
            ctx.scroll_to(0, i32::MAX);
        }
    }
}

impl Widget for ToastRack {
    fn type_names(&self) -> &'static [&'static str] {
        &["ToastRack"]
    }

    fn default_css(&self) -> &'static str {
        TOAST_RACK_CSS
    }

    fn can_focus_children(&self) -> bool {
        false
    }

    fn on_mount(&mut self, ctx: &mut Ctx) {
        ctx.add_class("-textual-system");
        self.sync(ctx);
    }

    fn on_message(&mut self, message: &Message, ctx: &mut Ctx) -> Flow {
        if message.get::<Notified>().is_some() {
            self.sync(ctx);
            return Flow::Stop;
        }
        Flow::Continue
    }
}

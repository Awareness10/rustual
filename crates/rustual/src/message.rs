//! Messages: typed payloads posted by widgets that bubble up the DOM.

use crate::dom::NodeId;
use std::any::Any;
use std::fmt;

/// Return value of event / message handlers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Flow {
    /// Not handled (or handled but should keep bubbling).
    #[default]
    Continue,
    /// Stop propagation to parents (and for keys, skip default handling such as bindings).
    Stop,
}

/// A message: `payload` is any `Send + 'static` value. Handlers match with [`Message::get`].
///
/// ```
/// # use rustual::prelude::*;
/// # struct MyApp;
/// # impl Widget for MyApp {
/// fn on_message(&mut self, msg: &Message, ctx: &mut Ctx) -> Flow {
///     if msg.get::<button::Pressed>().is_some() && msg.sender_id() == Some("ok") {
///         ctx.exit();
///     }
///     Flow::Continue
/// }
/// # }
/// ```
pub struct Message {
    /// Unique id (per process), so observers can recognise the same message while it bubbles.
    pub id: u64,
    /// The node that posted the message.
    pub sender: NodeId,
    /// The CSS id of the sender at post time, if it had one.
    pub sender_css_id: Option<String>,
    /// Whether the message bubbles to ancestors after the target handles it.
    pub bubble: bool,
    payload: Box<dyn Any + Send>,
}

fn next_id() -> u64 {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

impl Message {
    pub fn new<T: Any + Send>(sender: NodeId, sender_css_id: Option<String>, payload: T) -> Self {
        Message {
            id: next_id(),
            sender,
            sender_css_id,
            bubble: true,
            payload: Box::new(payload),
        }
    }

    pub fn from_boxed(
        sender: NodeId,
        sender_css_id: Option<String>,
        payload: Box<dyn Any + Send>,
    ) -> Self {
        Message {
            id: next_id(),
            sender,
            sender_css_id,
            bubble: true,
            payload,
        }
    }

    pub fn get<T: Any>(&self) -> Option<&T> {
        self.payload.downcast_ref::<T>()
    }

    /// The `TypeId` of the payload (for dispatch tables keyed by message type).
    pub fn payload_type_id(&self) -> std::any::TypeId {
        (*self.payload).type_id()
    }

    pub fn is<T: Any>(&self) -> bool {
        self.payload.is::<T>()
    }

    pub fn sender_id(&self) -> Option<&str> {
        self.sender_css_id.as_deref()
    }

    pub fn into_payload(self) -> Box<dyn Any + Send> {
        self.payload
    }
}

impl fmt::Debug for Message {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Message")
            .field("sender", &self.sender)
            .field("sender_css_id", &self.sender_css_id)
            .field("bubble", &self.bubble)
            .finish_non_exhaustive()
    }
}

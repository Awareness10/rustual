//! [`Ctx`]: the handle widgets use to talk to the app from handlers.

use crate::app::AppState;
use crate::dom::NodeId;

pub struct Ctx<'a> {
    pub(crate) app: &'a mut AppState,
    pub(crate) node: NodeId,
}

impl<'a> Ctx<'a> {
    /// The node of the widget whose handler is running.
    pub fn id(&self) -> NodeId {
        self.node
    }
}

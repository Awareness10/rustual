//! The built-in default [`Screen`] widget and screen CSS.
//!
//! Any widget can be pushed as a screen (`ctx.push_screen(MyScreen::new())`); the runtime adds the
//! `Screen` (and for modals `ModalScreen`) CSS type names automatically.

use crate::widget::Widget;

pub(crate) const SCREEN_CSS: &str = "
Screen {
    layout: vertical;
    overflow-y: auto;
    background: $background;
}
";

pub(crate) const MODAL_SCREEN_CSS: &str = "
ModalScreen {
    layout: vertical;
    overflow-y: auto;
    background: $background 60%;
}
";

/// The app's default screen (holds the root widget's composed children).
#[derive(Debug, Default)]
pub struct Screen;

impl Screen {
    pub fn new() -> Self {
        Screen
    }
}

impl Widget for Screen {
    fn type_names(&self) -> &'static [&'static str] {
        &["Screen"]
    }
}

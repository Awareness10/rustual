//! `Welcome` (port of `textual/widgets/_welcome.py`): a welcome screen with Textual's welcome
//! text and an "OK" button.
//!
//! Textual renders `WELCOME_MD` with Rich's Markdown; here the same document is laid out with
//! `Static` widgets and CSS that mimic Rich's rendering (boxed, centered `h1`; bold underlined
//! centered `h2`; a `▌` bar for the block quote).
//!
//! Pressing the "OK" button (`#close`) exits the app, like Textual's welcome example app
//! (`Welcome` itself does not exit in Textual); disable with [`Welcome::exit_on_close`].
//! The button's `button::Pressed` message still bubbles to the app.

use crate::context::Ctx;
use crate::message::{Flow, Message};
use crate::widget::{Child, Widget, WidgetExt};
use crate::widgets::button::{self, Button, ButtonVariant};
use crate::widgets::{Container, Static};

/// The welcome text (Textual's `WELCOME_MD`).
pub const WELCOME_MD: &str = r#"# Welcome!

Textual is a TUI, or *Text User Interface*, framework for Python inspired by modern web development. **We hope you enjoy using Textual!**

## Dune quote

> "I must not fear.
Fear is the mind-killer.
Fear is the little-death that brings total obliteration.
I will face my fear.
I will permit it to pass over me and through me.
And when it has gone past, I will turn the inner eye to see its path.
Where the fear has gone there will be nothing. Only I will remain."
"#;

const DEFAULT_CSS: &str = r#"
Welcome {
    width: 100%;
    height: 100%;
    background: $surface;
}

Welcome Container {
    padding: 1;
    color: $foreground;
}

Welcome #text {
    margin: 0 1;
    height: auto;
}

Welcome #close {
    dock: bottom;
    width: 100%;
}

Welcome .welcome--h1 {
    border: heavy $foreground;
    text-align: center;
    text-style: bold;
    height: auto;
}

Welcome .welcome--p {
    margin: 1 0;
    height: auto;
}

Welcome .welcome--h2 {
    text-align: center;
    text-style: bold underline;
    height: auto;
}

Welcome .welcome--quote {
    margin: 1 0;
    padding-left: 1;
    border-left: outer #F4005F;
    color: #F4005F;
    height: auto;
}
"#;

const PARAGRAPH: &str = "Textual is a TUI, or [i]Text User Interface[/i], framework for Python \
inspired by modern web development. [b]We hope you enjoy using Textual![/b]";

const QUOTE: &str = "\"I must not fear. Fear is the mind-killer. Fear is the little-death \
that brings total obliteration. I will face my fear. I will permit it to pass over me and \
through me. And when it has gone past, I will turn the inner eye to see its path. Where the \
fear has gone there will be nothing. Only I will remain.\"";

/// A welcome widget: the welcome text and an "OK" button that exits the app.
#[derive(Debug, Clone)]
pub struct Welcome {
    exit_on_close: bool,
}

impl Default for Welcome {
    fn default() -> Self {
        Self::new()
    }
}

impl Welcome {
    pub fn new() -> Self {
        Welcome {
            exit_on_close: true,
        }
    }

    /// Exit the app when "OK" is pressed (default true).
    pub fn exit_on_close(mut self, exit: bool) -> Self {
        self.exit_on_close = exit;
        self
    }
}

impl Widget for Welcome {
    fn type_names(&self) -> &'static [&'static str] {
        &["Welcome", "Static"]
    }

    fn default_css(&self) -> &'static str {
        DEFAULT_CSS
    }

    fn compose(&mut self) -> Vec<Child> {
        let text = Container.id("text").style("height: auto;").with([
            Static::new("Welcome!").classes("welcome--h1"),
            Static::new(PARAGRAPH).classes("welcome--p"),
            Static::new("Dune quote").classes("welcome--h2"),
            Static::new(QUOTE).classes("welcome--quote"),
        ]);
        vec![
            Container.id("md").with([text]),
            Button::new("OK")
                .variant(ButtonVariant::Success)
                .id("close"),
        ]
    }

    fn on_message(&mut self, msg: &Message, ctx: &mut Ctx) -> Flow {
        if let Some(p) = msg.get::<button::Pressed>() {
            if self.exit_on_close && p.button_id.as_deref() == Some("close") {
                ctx.exit();
            }
        }
        Flow::Continue
    }
}

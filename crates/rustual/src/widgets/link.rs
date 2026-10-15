//! `Link` (port of `textual/widgets/_link.py`): a focusable, clickable link that opens a URL
//! with the system opener (`xdg-open` / `open` / `start`).
//!
//! Opening goes through [`open_url`], which calls the opener installed with
//! [`set_url_opener`] (tests stub it) or the OS default. After opening, the link posts
//! [`Opened`].

use crate::binding::Binding;
use crate::context::Ctx;
use crate::events::Event;
use crate::message::Flow;
use crate::widget::{Content, RenderCtx, Widget};
use rustual_render::Text;
use std::io;
use std::sync::{Arc, RwLock};

const DEFAULT_CSS: &str = "
Link {
    width: auto;
    height: auto;
    min-height: 1;
    color: $text-accent;
    text-style: underline;
    &:hover { color: $accent; }
    &:focus { text-style: bold reverse; }
    pointer: pointer;
}
";

/// Posted by a [`Link`] after it asked the system to open its URL (not in Textual).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Opened {
    pub url: String,
    /// Whether the opener reported success.
    pub ok: bool,
}

type Opener = Arc<dyn Fn(&str) -> io::Result<()> + Send + Sync>;

static OPENER: RwLock<Option<Opener>> = RwLock::new(None);

/// Replace the function used to open URLs (process-wide). `None` restores the OS default.
pub fn set_url_opener(opener: Option<Opener>) {
    if let Ok(mut o) = OPENER.write() {
        *o = opener;
    }
}

/// Open `url` with the installed opener, or the OS default (Textual: `App.open_url`).
pub fn open_url(url: &str) -> io::Result<()> {
    let custom = OPENER.read().ok().and_then(|o| o.clone());
    match custom {
        Some(f) => f(url),
        None => system_open(url),
    }
}

/// Launch the platform's URL opener without waiting for it.
fn system_open(url: &str) -> io::Result<()> {
    use std::process::{Command, Stdio};
    let mut cmd = if cfg!(target_os = "windows") {
        let mut c = Command::new("cmd");
        c.args(["/C", "start", ""]).arg(url);
        c
    } else if cfg!(target_os = "macos") {
        let mut c = Command::new("open");
        c.arg(url);
        c
    } else {
        let mut c = Command::new("xdg-open");
        c.arg(url);
        c
    };
    let mut child = cmd
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    // Reap the process in the background.
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

impl Ctx<'_> {
    /// Open a URL with the system opener (Textual: `App.open_url`).
    pub fn open_url(&mut self, url: &str) -> io::Result<()> {
        open_url(url)
    }
}

/// A clickable link.
#[derive(Debug, Clone)]
pub struct Link {
    text: String,
    url: String,
}

impl Link {
    /// A link whose URL is its text.
    pub fn new(text: impl Into<String>) -> Self {
        let text = text.into();
        Link {
            url: text.clone(),
            text,
        }
    }

    /// Set the URL to open (defaults to the text).
    pub fn url(mut self, url: impl Into<String>) -> Self {
        self.url = url.into();
        self
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn get_url(&self) -> &str {
        &self.url
    }

    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
    }

    pub fn set_url(&mut self, url: impl Into<String>) {
        self.url = url.into();
    }

    /// Open the URL and post [`Opened`].
    pub fn action_open_link(&mut self, ctx: &mut Ctx) {
        if self.url.is_empty() {
            return;
        }
        let ok = open_url(&self.url).is_ok();
        ctx.post_message(Opened {
            url: self.url.clone(),
            ok,
        });
    }
}

impl Widget for Link {
    fn type_names(&self) -> &'static [&'static str] {
        &["Link", "Static"]
    }

    fn default_css(&self) -> &'static str {
        DEFAULT_CSS
    }

    fn can_focus(&self) -> bool {
        true
    }

    fn bindings(&self) -> Vec<Binding> {
        vec![Binding::new("enter", "open_link", "Open link")]
    }

    fn run_action(&mut self, action: &str, _args: &[String], ctx: &mut Ctx) -> bool {
        if action == "open_link" {
            self.action_open_link(ctx);
            return true;
        }
        false
    }

    fn on_event(&mut self, event: &Event, ctx: &mut Ctx) -> Flow {
        if let Event::Click(_) = event {
            self.action_open_link(ctx);
            return Flow::Stop;
        }
        Flow::Continue
    }

    fn render(&self, _ctx: &RenderCtx) -> Content {
        // Textual creates the Static with `markup=False`.
        Content::Text(Text::plain(self.text.clone()))
    }
}

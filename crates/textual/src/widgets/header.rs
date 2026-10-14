//! `Header` (port of `textual/widgets/_header.py`): the app title bar, docked at the top.
//!
//! Composed of a [`HeaderIcon`] (left), a [`HeaderTitle`] (center: app title — sub-title from
//! [`RenderEnv`](crate::widget::RenderEnv)) and either a [`HeaderClock`] or a [`HeaderClockSpace`]
//! (right). Clicking the header toggles the tall (3 line) mode.
//!
//! The clock shows `HH:MM:SS` in **UTC** by default (the standard library has no portable
//! local-timezone API); use [`Header::clock_utc_offset`] to shift it to a local zone.

use crate::context::Ctx;
use crate::events::Event;
use crate::message::Flow;
use crate::widget::{Child, Content, RenderCtx, Widget};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use textual_render::{Style, Text};
use tokio::time::Instant;

const HEADER_CSS: &str = "
Header {
    dock: top;
    width: 100%;
    background: $panel;
    color: $foreground;
    height: 1;
}
Header.-tall {
    height: 3;
}
";

const HEADER_ICON_CSS: &str = "
HeaderIcon {
    dock: left;
    padding: 0 1;
    width: 8;
    content-align: left middle;
}
HeaderIcon:hover {
    background: $foreground 10%;
}
";

const HEADER_CLOCK_SPACE_CSS: &str = "
HeaderClockSpace {
    dock: right;
    width: 10;
    padding: 0 1;
}
";

const HEADER_CLOCK_CSS: &str = "
HeaderClock {
    dock: right;
    width: 10;
    padding: 0 1;
    background: $foreground-darken-1 5%;
    color: $foreground;
    text-opacity: 85%;
    content-align: center middle;
}
";

const HEADER_TITLE_CSS: &str = "
HeaderTitle {
    text-wrap: nowrap;
    text-overflow: ellipsis;
    content-align: center middle;
    width: 100%;
}
";

/// The app header: icon, title / sub-title and an optional clock.
#[derive(Debug, Clone)]
pub struct Header {
    show_clock: bool,
    icon: String,
    tall: bool,
    utc_offset: i64,
}

impl Default for Header {
    fn default() -> Self {
        Self::new()
    }
}

impl Header {
    pub fn new() -> Self {
        Header {
            show_clock: false,
            icon: "⭘".to_string(),
            tall: false,
            utc_offset: 0,
        }
    }

    /// Show a clock on the right of the header (updated every second).
    pub fn show_clock(mut self, show: bool) -> Self {
        self.show_clock = show;
        self
    }

    /// The icon shown on the left (default `⭘`).
    pub fn icon(mut self, icon: &str) -> Self {
        self.icon = icon.to_string();
        self
    }

    /// Start in tall (3 line) mode.
    pub fn tall(mut self, tall: bool) -> Self {
        self.tall = tall;
        self
    }

    /// Offset of the clock from UTC, in seconds (e.g. `3600` for UTC+1).
    pub fn clock_utc_offset(mut self, seconds: i64) -> Self {
        self.utc_offset = seconds;
        self
    }

    pub fn is_tall(&self) -> bool {
        self.tall
    }

    /// Switch tall mode on or off.
    pub fn set_tall(&mut self, tall: bool, ctx: &mut Ctx) {
        self.tall = tall;
        ctx.set_class(tall, "-tall");
    }
}

impl Widget for Header {
    fn type_names(&self) -> &'static [&'static str] {
        &["Header"]
    }

    fn default_css(&self) -> &'static str {
        HEADER_CSS
    }

    fn compose(&mut self) -> Vec<Child> {
        let right: Child = if self.show_clock {
            HeaderClock::new().utc_offset(self.utc_offset).into()
        } else {
            HeaderClockSpace.into()
        };
        vec![
            HeaderIcon::new(&self.icon).into(),
            HeaderTitle.into(),
            right,
        ]
    }

    fn on_mount(&mut self, ctx: &mut Ctx) {
        ctx.set_class(self.tall, "-tall");
    }

    fn on_event(&mut self, event: &Event, ctx: &mut Ctx) -> Flow {
        if let Event::Click(_) = event {
            let tall = !self.tall;
            self.set_tall(tall, ctx);
            return Flow::Stop;
        }
        Flow::Continue
    }
}

/// The icon on the left of the header. (Textual opens the command palette on click;
/// textual-rs has no command palette, so clicks fall through to the header.)
#[derive(Debug, Clone)]
pub struct HeaderIcon {
    icon: String,
}

impl HeaderIcon {
    pub fn new(icon: &str) -> Self {
        HeaderIcon {
            icon: icon.to_string(),
        }
    }

    pub fn icon(&self) -> &str {
        &self.icon
    }

    pub fn set_icon(&mut self, icon: &str) {
        self.icon = icon.to_string();
    }
}

impl Widget for HeaderIcon {
    fn type_names(&self) -> &'static [&'static str] {
        &["HeaderIcon"]
    }

    fn default_css(&self) -> &'static str {
        HEADER_ICON_CSS
    }

    fn render(&self, _ctx: &RenderCtx) -> Content {
        Content::Text(Text::plain(self.icon.clone()))
    }
}

/// The (empty) space reserved on the right of the header when there is no clock.
#[derive(Debug, Clone, Default)]
pub struct HeaderClockSpace;

impl Widget for HeaderClockSpace {
    fn type_names(&self) -> &'static [&'static str] {
        &["HeaderClockSpace"]
    }

    fn default_css(&self) -> &'static str {
        HEADER_CLOCK_SPACE_CSS
    }
}

/// A clock (`HH:MM:SS`) on the right of the header, refreshed every second.
///
/// Time is measured as the wall-clock time at mount plus the (tokio) time elapsed since, so it
/// advances deterministically under `tokio::time::pause`.
#[derive(Debug, Clone)]
pub struct HeaderClock {
    utc_offset: i64,
    base: Duration,
    started: Option<Instant>,
}

impl Default for HeaderClock {
    fn default() -> Self {
        Self::new()
    }
}

impl HeaderClock {
    pub fn new() -> Self {
        HeaderClock {
            utc_offset: 0,
            base: Duration::ZERO,
            started: None,
        }
    }

    /// Offset from UTC in seconds.
    pub fn utc_offset(mut self, seconds: i64) -> Self {
        self.utc_offset = seconds;
        self
    }

    /// The displayed time, `HH:MM:SS`.
    pub fn time_text(&self) -> String {
        let elapsed = self.started.map(|s| s.elapsed()).unwrap_or_default();
        let secs = (self.base + elapsed).as_secs() as i64 + self.utc_offset;
        format_hms(secs)
    }
}

/// Format seconds since midnight (mod one day) as `HH:MM:SS`.
pub fn format_hms(secs: i64) -> String {
    let day = secs.rem_euclid(86_400);
    format!("{:02}:{:02}:{:02}", day / 3600, (day / 60) % 60, day % 60)
}

impl Widget for HeaderClock {
    fn type_names(&self) -> &'static [&'static str] {
        &["HeaderClock", "HeaderClockSpace"]
    }

    fn default_css(&self) -> &'static str {
        HEADER_CLOCK_CSS
    }

    fn on_mount(&mut self, ctx: &mut Ctx) {
        self.base = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        self.started = Some(Instant::now());
        // The runtime repaints after every timer callback.
        ctx.set_interval(Duration::from_secs(1), |_: &mut HeaderClock, _ctx| {});
    }

    fn render(&self, _ctx: &RenderCtx) -> Content {
        Content::Text(Text::plain(self.time_text()))
    }
}

/// The title / sub-title in the middle of the header (reads `title` and `sub_title` from the
/// render environment, formatted like Textual's `App.format_title`).
#[derive(Debug, Clone, Default)]
pub struct HeaderTitle;

impl Widget for HeaderTitle {
    fn type_names(&self) -> &'static [&'static str] {
        &["HeaderTitle"]
    }

    fn default_css(&self) -> &'static str {
        HEADER_TITLE_CSS
    }

    fn render(&self, ctx: &RenderCtx) -> Content {
        Content::Text(format_title(&ctx.env.title, &ctx.env.sub_title))
    }
}

/// `App.format_title`: `title — sub_title` with the sub-title dimmed.
pub fn format_title(title: &str, sub_title: &str) -> Text {
    let mut text = Text::plain(title);
    if !sub_title.is_empty() {
        text.push(" — ", Style::new().dim());
        text.push(sub_title, Style::new().dim());
    }
    text
}

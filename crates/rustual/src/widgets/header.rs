//! `Header` (port of `textual/widgets/_header.py`): the app title bar, docked at the top.
//!
//! Composed of a [`HeaderIcon`] (left), a [`HeaderTitle`] (center: app title — sub-title from
//! [`RenderEnv`](crate::widget::RenderEnv)) and either a [`HeaderClock`] or a [`HeaderClockSpace`]
//! (right). Clicking the header toggles the tall (3 line) mode.
//!
//! The clock shows the local time formatted with [`Header::time_format`] (strftime, default
//! `%X` = `HH:MM:SS`), like Textual; [`Header::clock_utc_offset`] pins it to a fixed offset.

use crate::context::Ctx;
use crate::events::Event;
use crate::message::Flow;
use crate::widget::{Child, Content, RenderCtx, Widget};
use chrono::format::StrftimeItems;
use chrono::{DateTime, FixedOffset, Local, Utc};
use rustual_render::{Style, Text};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
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
    time_format: String,
    utc_offset: Option<i64>,
}

/// Textual's default clock format (`HeaderClock.time_format`).
pub const DEFAULT_TIME_FORMAT: &str = "%X";

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
            time_format: DEFAULT_TIME_FORMAT.to_string(),
            utc_offset: None,
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

    /// The clock's strftime format (default `%X`, e.g. `%H:%M` or `%a %H:%M`).
    pub fn time_format(mut self, format: &str) -> Self {
        self.time_format = format.to_string();
        self
    }

    /// Show the clock at a fixed offset from UTC, in seconds (e.g. `3600` for UTC+1), instead
    /// of the system's local time zone.
    pub fn clock_utc_offset(mut self, seconds: i64) -> Self {
        self.utc_offset = Some(seconds);
        self
    }

    pub fn get_time_format(&self) -> &str {
        &self.time_format
    }

    /// Change the clock format (Textual: `header.time_format = ...`).
    pub fn set_time_format(&mut self, format: &str, ctx: &mut Ctx) {
        self.time_format = format.to_string();
        for child in ctx.children() {
            if let Some(clock) = ctx.get::<HeaderClock>(child) {
                clock.time_format = self.time_format.clone();
            }
        }
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
            HeaderClock {
                time_format: self.time_format.clone(),
                utc_offset: self.utc_offset,
                ..HeaderClock::new()
            }
            .into()
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

/// The icon on the left of the header. Clicking it opens the command palette (the icon is
/// disabled when the palette is disabled).
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

    fn on_mount(&mut self, ctx: &mut Ctx) {
        if ctx.app.ux.palette_enabled {
            ctx.set_tooltip(Some("Open the command palette"));
        } else {
            let me = ctx.id();
            ctx.set_disabled(me, true);
        }
    }

    fn on_event(&mut self, event: &Event, ctx: &mut Ctx) -> Flow {
        if let Event::Click(_) = event {
            if !ctx.is_disabled() {
                ctx.run_action("app.command_palette");
                return Flow::Stop;
            }
        }
        Flow::Continue
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

/// A clock on the right of the header (local time, `HH:MM:SS` by default), refreshed every
/// second.
///
/// Time is measured as the wall-clock time at mount plus the (tokio) time elapsed since, so it
/// advances deterministically under `tokio::time::pause`.
#[derive(Debug, Clone)]
pub struct HeaderClock {
    time_format: String,
    utc_offset: Option<i64>,
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
            time_format: DEFAULT_TIME_FORMAT.to_string(),
            utc_offset: None,
            base: Duration::ZERO,
            started: None,
        }
    }

    /// Fixed offset from UTC in seconds (default: the system's local time zone).
    pub fn utc_offset(mut self, seconds: i64) -> Self {
        self.utc_offset = Some(seconds);
        self
    }

    /// strftime format (default `%X`).
    pub fn time_format(mut self, format: &str) -> Self {
        self.time_format = format.to_string();
        self
    }

    /// The displayed time.
    pub fn time_text(&self) -> String {
        let elapsed = self.started.map(|s| s.elapsed()).unwrap_or_default();
        format_clock(self.base + elapsed, self.utc_offset, &self.time_format)
    }
}

/// Format a time (since the Unix epoch) with a strftime `format`, in the system's local time
/// zone or at a fixed `utc_offset` (seconds). An invalid format is returned as-is.
pub fn format_clock(since_epoch: Duration, utc_offset: Option<i64>, format: &str) -> String {
    let Ok(items) = StrftimeItems::new(format).parse() else {
        return format.to_string();
    };
    let utc =
        DateTime::<Utc>::from_timestamp(since_epoch.as_secs() as i64, since_epoch.subsec_nanos())
            .unwrap_or_default();
    let offset = utc_offset.and_then(|s| FixedOffset::east_opt(i32::try_from(s).ok()?));
    match offset {
        Some(offset) => utc.with_timezone(&offset).format_with_items(items.iter()),
        None => utc.with_timezone(&Local).format_with_items(items.iter()),
    }
    .to_string()
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

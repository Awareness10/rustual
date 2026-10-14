//! `Select` (port of `textual/widgets/_select.py`).
//!
//! ```ignore
//! Select::new([("Red", 1), ("Green", 2)]).prompt("Colour").id("colour")
//! Select::from_values(["a", "b"])            // prompts from `to_string()`
//! // on_message: if let Some(c) = msg.get::<select::Changed<i32>>() { .. }
//! ```
//!
//! Deviation from Textual: the layout engine has no `overlay: screen`, so the drop-down
//! (`SelectOverlay`) is shown in a transparent **modal screen** pushed by the `Select`, with
//! the list placed under the widget via an inline `offset` (moved up if it would leave the
//! screen). `escape` or a click outside the list dismisses it; `enter` / click selects.
//! Focus returns to the `Select` when the overlay closes. The list is a small private widget
//! (not `OptionList`). Type-to-search jumps to the first option containing the typed text
//! (the query resets after 0.7 s, as in Textual).

use crate::binding::Binding;
use crate::compose;
use crate::context::Ctx;
use crate::dom::NodeId;
use crate::events::Event;
use crate::message::{Flow, Message};
use crate::widget::{Child, Content, RenderCtx, Widget, WidgetExt};
use crate::widgets::Static;
use crate::TimerId;
use std::time::Duration;
use textual_render::{Region, Size, Span, Strip, Style, Text};

const SELECT_CSS: &str = r#"
Select {
    height: auto;
    color: $foreground;
    &.-textual-compact {
        & > SelectCurrent {
            padding: 0 1 0 0;
            border: none !important;
        }
    }
    .up-arrow {
        display: none;
    }
    &:focus > SelectCurrent {
        border: tall $border;
        background-tint: $foreground 5%;
    }
    &.-expanded {
        .down-arrow {
            display: none;
        }
        .up-arrow {
            display: block;
        }
    }
}
"#;

const CURRENT_CSS: &str = r#"
SelectCurrent {
    layout: horizontal;
    overflow: hidden hidden;
    border: tall $border-blurred;
    color: $foreground;
    background: $surface;
    width: 1fr;
    height: auto;
    padding: 0 2;
    pointer: pointer;
    &.-textual-compact {
        border: none !important;
    }
    Static#label {
        width: 1fr;
        height: auto;
        color: $foreground 50%;
        background: transparent;
    }
    &.-has-value Static#label {
        color: $foreground;
    }
    .arrow {
        box-sizing: content-box;
        width: 1;
        height: 1;
        padding: 0 0 0 1;
        color: $foreground 50%;
        background: transparent;
    }
}
"#;

const OVERLAY_CSS: &str = r#"
SelectOverlay {
    color: $foreground;
    border: tall $border-blurred;
    background: $surface;
    overflow-x: hidden;
    overflow-y: auto;
    padding: 0 1;
    &:focus {
        border: tall $border;
        background-tint: $foreground 5%;
    }
    & > .option-list--option-highlighted {
        color: $foreground;
        background: $block-cursor-blurred-background;
    }
    &:focus > .option-list--option-highlighted {
        color: $block-cursor-foreground;
        background: $block-cursor-background;
        text-style: $block-cursor-text-style;
    }
    & > .option-list--option-hover {
        background: $block-hover-background;
    }
}
"#;

/// Maximum overlay height (Textual: `max-height: 12`).
const OVERLAY_MAX_HEIGHT: i32 = 12;

/// Posted when the value of a [`Select`] changes (Textual: `Select.Changed`).
/// `value` is `None` for the blank selection (`Select.NULL`).
#[derive(Debug, Clone, PartialEq)]
pub struct Changed<T> {
    pub value: Option<T>,
    pub select_id: Option<String>,
}

/// Posted by [`SelectCurrent`] when clicked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToggleOverlay;

/// Result of the overlay screen: the chosen option index, or `None` if dismissed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverlayResult(pub Option<usize>);

// ---------------------------------------------------------------- SelectCurrent

/// Displays the current value (or the prompt) and an arrow.
#[derive(Debug, Clone)]
pub struct SelectCurrent {
    placeholder: String,
}

impl Widget for SelectCurrent {
    fn type_names(&self) -> &'static [&'static str] {
        &["SelectCurrent"]
    }

    fn default_css(&self) -> &'static str {
        CURRENT_CSS
    }

    fn compose(&mut self) -> Vec<Child> {
        compose![
            Static::new(self.placeholder.clone()).id("label"),
            Static::new("▼").classes("arrow down-arrow"),
            Static::new("▲").classes("arrow up-arrow"),
        ]
    }

    fn on_event(&mut self, event: &Event, ctx: &mut Ctx) -> Flow {
        if let Event::Click(_) = event {
            ctx.post_message(ToggleOverlay);
            return Flow::Stop;
        }
        Flow::Continue
    }
}

// ---------------------------------------------------------------- overlay

/// The drop-down list shown in the overlay screen (a minimal option list).
#[derive(Debug, Clone)]
pub struct SelectOverlay {
    options: Vec<String>,
    /// Index of the blank option (rendered dim), if any.
    blank: Option<usize>,
    highlighted: Option<usize>,
    hover: Option<usize>,
    search: String,
    search_timer: Option<TimerId>,
}

impl SelectOverlay {
    pub fn highlighted(&self) -> Option<usize> {
        self.highlighted
    }

    pub fn options(&self) -> &[String] {
        &self.options
    }

    fn highlight(&mut self, index: Option<usize>, ctx: &mut Ctx) {
        self.highlighted = index.map(|i| i.min(self.options.len().saturating_sub(1)));
        if let Some(i) = self.highlighted {
            ctx.scroll_to_show(Region::new(0, i as i32, 1, 1));
        }
    }

    fn find_match(&self, query: &str) -> Option<usize> {
        let query = query.to_lowercase();
        let mut best: Option<(usize, usize)> = None;
        for (i, o) in self.options.iter().enumerate() {
            if Some(i) == self.blank {
                continue;
            }
            let plain = Text::from_markup(o).plain_text().to_lowercase();
            if let Some(pos) = plain.find(&query) {
                if best.map(|(_, p)| pos < p).unwrap_or(true) {
                    best = Some((i, pos));
                }
            }
        }
        best.map(|(i, _)| i)
    }

    fn line_at(&self, screen_y: i32, ctx: &Ctx) -> Option<usize> {
        let r = ctx.content_region();
        let y = screen_y - r.y + ctx.scroll_offset().y;
        (screen_y >= r.y && screen_y < r.bottom() && y >= 0 && (y as usize) < self.options.len())
            .then_some(y as usize)
    }
}

impl Widget for SelectOverlay {
    fn type_names(&self) -> &'static [&'static str] {
        &["SelectOverlay"]
    }

    fn default_css(&self) -> &'static str {
        OVERLAY_CSS
    }

    fn can_focus(&self) -> bool {
        true
    }

    fn on_mount(&mut self, ctx: &mut Ctx) {
        let h = self.highlighted;
        self.highlight(h, ctx);
    }

    fn bindings(&self) -> Vec<Binding> {
        vec![
            Binding::new("escape", "dismiss", "Dismiss menu"),
            Binding::new("enter", "select", "Select").hidden(),
            Binding::new("up", "cursor_up", "Up").hidden(),
            Binding::new("down", "cursor_down", "Down").hidden(),
            Binding::new("home", "first", "First").hidden(),
            Binding::new("end", "last", "Last").hidden(),
            Binding::new("pageup", "page_up", "Page up").hidden(),
            Binding::new("pagedown", "page_down", "Page down").hidden(),
        ]
    }

    fn run_action(&mut self, action: &str, _args: &[String], ctx: &mut Ctx) -> bool {
        let n = self.options.len();
        if n == 0 && action != "dismiss" {
            return true;
        }
        let page = ctx.size().height.max(1) as usize;
        match action {
            "dismiss" => ctx.dismiss(OverlayResult(None)),
            "select" => {
                if let Some(i) = self.highlighted {
                    ctx.dismiss(OverlayResult(Some(i)));
                }
            }
            "cursor_up" => {
                let i = match self.highlighted {
                    None | Some(0) => n - 1,
                    Some(i) => i - 1,
                };
                self.highlight(Some(i), ctx);
            }
            "cursor_down" => {
                let i = match self.highlighted {
                    None => 0,
                    Some(i) => (i + 1) % n,
                };
                self.highlight(Some(i), ctx);
            }
            "first" => self.highlight(Some(0), ctx),
            "last" => self.highlight(Some(n - 1), ctx),
            "page_up" => {
                let i = self.highlighted.unwrap_or(0).saturating_sub(page);
                self.highlight(Some(i), ctx);
            }
            "page_down" => {
                let i = (self.highlighted.unwrap_or(0) + page).min(n - 1);
                self.highlight(Some(i), ctx);
            }
            _ => return false,
        }
        true
    }

    fn on_event(&mut self, event: &Event, ctx: &mut Ctx) -> Flow {
        match event {
            Event::Key(k) => {
                if let Some(c) = k.character.filter(|c| !c.is_control() && *c != ' ') {
                    self.search.push(c);
                    if let Some(t) = self.search_timer.take() {
                        ctx.cancel_timer(t);
                    }
                    self.search_timer = Some(ctx.set_timer(
                        Duration::from_millis(700),
                        |w: &mut SelectOverlay, _ctx| {
                            w.search.clear();
                            w.search_timer = None;
                        },
                    ));
                    if let Some(i) = self.find_match(&self.search.clone()) {
                        self.highlight(Some(i), ctx);
                    }
                    return Flow::Stop;
                }
                Flow::Continue
            }
            Event::MouseMove(m) => {
                self.hover = self.line_at(m.screen_y, ctx);
                Flow::Continue
            }
            Event::Leave => {
                self.hover = None;
                Flow::Continue
            }
            Event::Click(m) => {
                if let Some(i) = self.line_at(m.screen_y, ctx) {
                    self.highlighted = Some(i);
                    ctx.dismiss(OverlayResult(Some(i)));
                }
                Flow::Stop
            }
            _ => Flow::Continue,
        }
    }

    fn virtual_size(&self, ctx: &RenderCtx) -> Option<Size> {
        Some(Size::new(ctx.size.width, self.options.len() as i32))
    }

    fn render(&self, ctx: &RenderCtx) -> Content {
        let width = ctx.size.width;
        let scroll = ctx.scroll_offset.y;
        let highlighted = ctx.component_style("option-list--option-highlighted");
        let hover = ctx.component_style("option-list--option-hover");
        let mut lines = Vec::new();
        for y in 0..ctx.size.height {
            let i = (y + scroll) as usize;
            let Some(prompt) = self.options.get(i) else {
                lines.push(Strip::blank(width, Style::default()));
                continue;
            };
            let line_style = if Some(i) == self.highlighted {
                highlighted.clone()
            } else if Some(i) == self.hover {
                hover.clone()
            } else {
                Style::default()
            };
            let mut text = ctx.markup(prompt);
            if Some(i) == self.blank {
                text = Text::styled(text.plain_text(), Style::new().dim());
            }
            let mut strip = Strip::default();
            for s in text.spans {
                strip.push(Span::new(
                    s.text.replace('\n', " "),
                    line_style.combine(&s.style),
                ));
            }
            lines.push(strip.set_cell_len(width, &line_style));
        }
        Content::Lines(lines)
    }
}

/// The transparent modal screen hosting the [`SelectOverlay`]. Clicking outside the list
/// dismisses it.
#[derive(Debug, Clone)]
pub struct SelectOverlayScreen {
    overlay: Option<SelectOverlay>,
    region: Region,
}

impl Widget for SelectOverlayScreen {
    fn type_names(&self) -> &'static [&'static str] {
        &["SelectOverlayScreen"]
    }

    fn compose(&mut self) -> Vec<Child> {
        let r = self.region;
        let style = format!(
            "width: {}; height: {}; offset: {} {};",
            r.width, r.height, r.x, r.y
        );
        match self.overlay.take() {
            Some(o) => vec![o.style(&style)],
            None => Vec::new(),
        }
    }

    fn on_event(&mut self, event: &Event, ctx: &mut Ctx) -> Flow {
        if let Event::Click(_) = event {
            ctx.dismiss(OverlayResult(None));
            return Flow::Stop;
        }
        Flow::Continue
    }
}

// ---------------------------------------------------------------- Select

/// A control showing the current value that opens a drop-down list of options.
///
/// Keys: `enter`, `down`, `space`, `up` open the overlay.
#[derive(Debug, Clone)]
pub struct Select<T: Clone + PartialEq + Send + 'static> {
    options: Vec<(String, Option<T>)>,
    allow_blank: bool,
    prompt: String,
    value: Option<T>,
    initial: Option<T>,
    expanded: bool,
    current_node: Option<NodeId>,
}

impl<T: Clone + PartialEq + Send + 'static> Select<T> {
    /// A select from `(prompt, value)` pairs (prompts are markup).
    pub fn new<S: Into<String>>(options: impl IntoIterator<Item = (S, T)>) -> Self {
        let mut s = Select {
            options: Vec::new(),
            allow_blank: true,
            prompt: "Select".into(),
            value: None,
            initial: None,
            expanded: false,
            current_node: None,
        };
        s.setup_options(options.into_iter().map(|(p, v)| (p.into(), v)).collect());
        s
    }

    /// Text shown when nothing is selected (default `"Select"`).
    pub fn prompt(mut self, prompt: impl Into<String>) -> Self {
        self.prompt = prompt.into();
        self
    }

    /// Allow the blank (no selection) state (default true). If false, the first option is
    /// selected initially.
    pub fn allow_blank(mut self, allow: bool) -> Self {
        let opts: Vec<(String, T)> = self
            .options
            .drain(..)
            .filter_map(|(p, v)| v.map(|v| (p, v)))
            .collect();
        self.allow_blank = allow;
        self.setup_options(opts);
        self
    }

    /// Initial value.
    pub fn value(mut self, value: T) -> Self {
        self.initial = Some(value);
        self
    }

    fn setup_options(&mut self, options: Vec<(String, T)>) {
        self.options.clear();
        if self.allow_blank {
            self.options.push((String::new(), None));
        }
        self.options
            .extend(options.into_iter().map(|(p, v)| (p, Some(v))));
    }

    /// The current value (`None` = blank).
    pub fn get_value(&self) -> Option<&T> {
        self.value.as_ref()
    }

    pub fn is_blank(&self) -> bool {
        self.value.is_none()
    }

    pub fn is_expanded(&self) -> bool {
        self.expanded
    }

    /// Prompts of the selectable options (excluding the blank entry).
    pub fn option_prompts(&self) -> Vec<&str> {
        self.options
            .iter()
            .filter(|(_, v)| v.is_some())
            .map(|(p, _)| p.as_str())
            .collect()
    }

    /// Set the value. `None` clears (only if blank is allowed). Returns false for an
    /// illegal value. Posts [`Changed`] if the value changed.
    pub fn set_value(&mut self, value: Option<T>, ctx: &mut Ctx) -> bool {
        if !self.options.iter().any(|(_, v)| *v == value) {
            return false;
        }
        if value != self.value {
            self.value = value;
            self.update_current(ctx);
            self.post_changed(ctx);
        }
        true
    }

    /// Clear the selection. Returns false if blank is not allowed.
    pub fn clear(&mut self, ctx: &mut Ctx) -> bool {
        self.set_value(None, ctx)
    }

    /// Replace the options; the selection is reset (to blank, or the first option).
    pub fn set_options<S: Into<String>>(
        &mut self,
        options: impl IntoIterator<Item = (S, T)>,
        ctx: &mut Ctx,
    ) {
        self.setup_options(options.into_iter().map(|(p, v)| (p.into(), v)).collect());
        let v = self.default_value(None);
        if v != self.value {
            self.value = v;
            self.post_changed(ctx);
        }
        self.update_current(ctx);
    }

    fn default_value(&self, hint: Option<T>) -> Option<T> {
        match hint {
            Some(h) if self.options.iter().any(|(_, v)| v.as_ref() == Some(&h)) => Some(h),
            _ if !self.allow_blank => self.options.first().and_then(|(_, v)| v.clone()),
            _ => None,
        }
    }

    fn post_changed(&self, ctx: &mut Ctx) {
        let select_id = ctx.css_id(ctx.id()).map(String::from);
        ctx.post_message(Changed {
            value: self.value.clone(),
            select_id,
        });
    }

    fn update_current(&self, ctx: &mut Ctx) {
        let Some(cur) = self.current_node else { return };
        let label = match &self.value {
            None => self.prompt.clone(),
            Some(v) => self
                .options
                .iter()
                .find(|(_, o)| o.as_ref() == Some(v))
                .map(|(p, _)| p.clone())
                .unwrap_or_default(),
        };
        ctx.set_class_on(cur, self.value.is_some(), "-has-value");
        if let Some(l) = ctx.dom().find_by_css_id(cur, "label") {
            if let Some(s) = ctx.get::<Static>(l) {
                s.update(label);
            }
        }
    }

    /// Open the overlay.
    pub fn show_overlay(&mut self, ctx: &mut Ctx) {
        if self.expanded || ctx.is_disabled() {
            return;
        }
        self.expanded = true;
        ctx.add_class("-expanded");
        if let Some(cur) = self.current_node {
            ctx.add_class_to(cur, "-has-value");
        }
        let highlighted = self
            .options
            .iter()
            .position(|(_, v)| self.value.is_some() && *v == self.value)
            .or(Some(0));
        let prompts: Vec<String> = self
            .options
            .iter()
            .map(|(p, v)| {
                if v.is_none() {
                    self.prompt.clone()
                } else {
                    p.clone()
                }
            })
            .collect();
        let blank = self.allow_blank.then_some(0);
        // Place the list under the `SelectCurrent`, kept inside the screen.
        let region = ctx.region();
        let below = self
            .current_node
            .map(|c| ctx.region_of(c).bottom())
            .unwrap_or(region.bottom());
        let height = (prompts.len() as i32 + 2).min(OVERLAY_MAX_HEIGHT);
        let screen = ctx.app_size();
        let y = if below + height > screen.height {
            (screen.height - height).max(0)
        } else {
            below
        };
        let overlay = SelectOverlay {
            options: prompts,
            blank,
            highlighted,
            hover: None,
            search: String::new(),
            search_timer: None,
        };
        // Inline style: must beat `ModalScreen { background: $background 60% }`.
        ctx.push_modal(
            SelectOverlayScreen {
                overlay: Some(overlay),
                region: Region::new(region.x, y, region.width.max(1), height),
            }
            .style("background: transparent;"),
        );
    }
}

impl<S: ToString, T: Clone + PartialEq + Send + 'static> FromIterator<(S, T)> for Select<T> {
    fn from_iter<I: IntoIterator<Item = (S, T)>>(iter: I) -> Self {
        Select::new(iter.into_iter().map(|(s, t)| (s.to_string(), t)))
    }
}

impl<T: Clone + PartialEq + Send + ToString + 'static> Select<T> {
    /// A select whose prompts are the values' `to_string()` (Textual: `Select.from_values`).
    pub fn from_values(values: impl IntoIterator<Item = T>) -> Self {
        Select::new(values.into_iter().map(|v| (v.to_string(), v)))
    }
}

impl<T: Clone + PartialEq + Send + 'static> Widget for Select<T> {
    fn type_names(&self) -> &'static [&'static str] {
        &["Select"]
    }

    fn default_css(&self) -> &'static str {
        SELECT_CSS
    }

    fn can_focus(&self) -> bool {
        true
    }

    fn compose(&mut self) -> Vec<Child> {
        compose![SelectCurrent {
            placeholder: self.prompt.clone()
        }]
    }

    fn on_mount(&mut self, ctx: &mut Ctx) {
        self.current_node = ctx
            .children()
            .into_iter()
            .find(|c| ctx.dom().widget::<SelectCurrent>(*c).is_some());
        let hint = self.initial.take();
        let v = self.default_value(hint);
        if v.is_some() {
            self.value = v;
            self.post_changed(ctx);
        }
        self.update_current(ctx);
    }

    fn bindings(&self) -> Vec<Binding> {
        vec![Binding::new("enter,down,space,up", "show_overlay", "Show menu").hidden()]
    }

    fn run_action(&mut self, action: &str, _args: &[String], ctx: &mut Ctx) -> bool {
        if action == "show_overlay" {
            self.show_overlay(ctx);
            return true;
        }
        false
    }

    fn on_message(&mut self, msg: &Message, ctx: &mut Ctx) -> Flow {
        if msg.is::<ToggleOverlay>() && Some(msg.sender) == self.current_node {
            if !self.expanded {
                ctx.focus_self();
                self.show_overlay(ctx);
            }
            return Flow::Stop;
        }
        if let Some(OverlayResult(choice)) = msg.get::<OverlayResult>() {
            self.expanded = false;
            ctx.remove_class("-expanded");
            if let Some(v) = choice
                .and_then(|i| self.options.get(i))
                .map(|(_, v)| v.clone())
            {
                self.set_value(v, ctx);
            }
            self.update_current(ctx);
            return Flow::Stop;
        }
        Flow::Continue
    }
}

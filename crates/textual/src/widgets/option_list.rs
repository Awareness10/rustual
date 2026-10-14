//! `OptionList` (port of `textual/widgets/_option_list.py`): a self-scrolling list of
//! options with markup prompts, separators and disabled options.
//!
//! ```ignore
//! OptionList::new()
//!     .option("One")
//!     .option(OptionItem::new("[b]Two[/]").id("two"))
//!     .separator()
//!     .option(OptionItem::new("Three").disabled(true))
//! ```

use crate::binding::Binding;
use crate::context::Ctx;
use crate::events::Event;
use crate::message::Flow;
use crate::widget::{parse_color_value, Content, RenderCtx, Widget};
use std::cell::RefCell;
use std::collections::HashMap;
use textual_render::{Justify, Region, Size, Strip, Style, Text, TextOverflow};

const DEFAULT_CSS: &str = r#"
OptionList {
    overflow-y: auto;
    overflow-x: auto;
}
OptionList {
    height: auto;
    max-height: 100%;
    color: $foreground;
    overflow-x: hidden;
    border: tall $border-blurred;
    padding: 0 1;
    background: $surface;
    &.-textual-compact {
        border: none !important;
        padding: 0;
        & > .option-list--option {
            padding: 0;
        }
    }
    & > .option-list--option-highlighted {
        color: $block-cursor-blurred-foreground;
        background: $block-cursor-blurred-background;
        text-style: $block-cursor-blurred-text-style;
    }
    &:focus {
        border: tall $border;
        background-tint: $foreground 5%;
        & > .option-list--option-highlighted {
            color: $block-cursor-foreground;
            background: $block-cursor-background;
            text-style: $block-cursor-text-style;
        }
    }
    & > .option-list--separator {
        color: $foreground 15%;
    }
    & > .option-list--option-highlighted {
        color: $foreground;
        background: $block-cursor-blurred-background;
    }
    & > .option-list--option-disabled {
        color: $text-disabled;
    }
    & > .option-list--option-hover {
        background: $block-hover-background;
    }
}
"#;

/// Posted when an option is highlighted (Textual: `OptionList.OptionHighlighted`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptionHighlighted {
    pub option_index: usize,
    pub option_id: Option<String>,
}

/// Posted when an option is selected (Textual: `OptionList.OptionSelected`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptionSelected {
    pub option_index: usize,
    pub option_id: Option<String>,
}

/// An option's prompt.
#[derive(Debug, Clone, PartialEq)]
pub enum Prompt {
    /// Content markup (with `$variable` support), unless the list has `markup(false)`.
    Markup(String),
    Text(Text),
}

/// One option of an [`OptionList`] (Textual: `Option`).
#[derive(Debug, Clone, PartialEq)]
pub struct OptionItem {
    prompt: Prompt,
    id: Option<String>,
    disabled: bool,
    /// A separator line follows this option.
    divider: bool,
}

impl OptionItem {
    /// An option with a markup prompt.
    pub fn new(prompt: impl Into<String>) -> Self {
        OptionItem {
            prompt: Prompt::Markup(prompt.into()),
            id: None,
            disabled: false,
            divider: false,
        }
    }

    /// An option with a pre-styled prompt.
    pub fn text(prompt: Text) -> Self {
        OptionItem {
            prompt: Prompt::Text(prompt),
            id: None,
            disabled: false,
            divider: false,
        }
    }

    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn get_id(&self) -> Option<&str> {
        self.id.as_deref()
    }

    pub fn is_disabled(&self) -> bool {
        self.disabled
    }

    pub fn prompt(&self) -> &Prompt {
        &self.prompt
    }

    /// The prompt as plain text.
    pub fn plain_prompt(&self) -> String {
        match &self.prompt {
            Prompt::Markup(m) => Text::from_markup(m).plain_text(),
            Prompt::Text(t) => t.plain_text(),
        }
    }
}

impl From<&str> for OptionItem {
    fn from(s: &str) -> Self {
        OptionItem::new(s)
    }
}

impl From<String> for OptionItem {
    fn from(s: String) -> Self {
        OptionItem::new(s)
    }
}

impl From<Text> for OptionItem {
    fn from(t: Text) -> Self {
        OptionItem::text(t)
    }
}

/// Error from option lookups / additions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptionListError {
    DuplicateId(String),
    OptionDoesNotExist(String),
}

impl std::fmt::Display for OptionListError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OptionListError::DuplicateId(id) => write!(f, "duplicate option id {id:?}"),
            OptionListError::OptionDoesNotExist(id) => write!(f, "no option {id}"),
        }
    }
}

impl std::error::Error for OptionListError {}

/// Wrapped lines of all options for one width (`_LineCache`).
#[derive(Debug, Clone, Default)]
struct LineCache {
    width: i32,
    /// (option index, line offset) per display line; a divider is the line after the option's last.
    lines: Vec<(usize, usize)>,
    index_to_line: Vec<usize>,
    heights: Vec<usize>,
    /// Unstyled wrapped prompt lines per option.
    strips: Vec<Vec<Strip>>,
}

/// A navigable list of options.
#[derive(Debug, Clone)]
pub struct OptionList {
    options: Vec<OptionItem>,
    id_to_index: HashMap<String, usize>,
    highlighted: Option<usize>,
    hover: Option<usize>,
    markup: bool,
    compact: bool,
    /// Caches for the last two widths used (layout and render widths can differ by a scrollbar).
    cache: RefCell<Vec<LineCache>>,
}

impl Default for OptionList {
    fn default() -> Self {
        Self::new()
    }
}

impl OptionList {
    pub fn new() -> Self {
        OptionList {
            options: Vec::new(),
            id_to_index: HashMap::new(),
            highlighted: None,
            hover: None,
            markup: true,
            compact: false,
            cache: RefCell::new(Vec::new()),
        }
    }

    /// Build from prompts / options (the first enabled option is highlighted).
    pub fn from_options<I, O>(options: I) -> Self
    where
        I: IntoIterator<Item = O>,
        O: Into<OptionItem>,
    {
        let mut list = Self::new();
        for o in options {
            list = list.option(o);
        }
        list
    }

    /// Builder: add an option. Panics on a duplicate id (use [`OptionList::add_option`] to handle it).
    pub fn option(mut self, option: impl Into<OptionItem>) -> Self {
        self.add_option(option).expect("duplicate option id");
        if self.highlighted.is_none() {
            self.highlighted = self.find_first_enabled();
        }
        self
    }

    /// Builder: add a separator after the last option.
    pub fn separator(mut self) -> Self {
        self.add_separator();
        self
    }

    /// Render prompts as markup (default `true`) or plain text.
    pub fn markup(mut self, markup: bool) -> Self {
        self.markup = markup;
        self
    }

    /// Compact style (no border / padding); sets the `-textual-compact` class on mount.
    pub fn compact(mut self, compact: bool) -> Self {
        self.compact = compact;
        self
    }

    pub fn options(&self) -> &[OptionItem] {
        &self.options
    }

    pub fn option_count(&self) -> usize {
        self.options.len()
    }

    /// Index of the highlighted option.
    pub fn highlighted(&self) -> Option<usize> {
        self.highlighted
    }

    pub fn highlighted_option(&self) -> Option<&OptionItem> {
        self.highlighted.and_then(|i| self.options.get(i))
    }

    fn clear_caches(&mut self) {
        self.cache.borrow_mut().clear();
    }

    /// Add an option at the end.
    pub fn add_option(&mut self, option: impl Into<OptionItem>) -> Result<(), OptionListError> {
        let option = option.into();
        if let Some(id) = &option.id {
            if self.id_to_index.contains_key(id) {
                return Err(OptionListError::DuplicateId(id.clone()));
            }
            self.id_to_index.insert(id.clone(), self.options.len());
        }
        self.options.push(option);
        self.clear_caches();
        Ok(())
    }

    /// Add several options (checked for duplicate ids first; nothing is added on error).
    pub fn add_options<I, O>(&mut self, options: I) -> Result<(), OptionListError>
    where
        I: IntoIterator<Item = O>,
        O: Into<OptionItem>,
    {
        let options: Vec<OptionItem> = options.into_iter().map(Into::into).collect();
        let mut seen = std::collections::HashSet::new();
        for o in &options {
            if let Some(id) = &o.id {
                if self.id_to_index.contains_key(id) || !seen.insert(id.clone()) {
                    return Err(OptionListError::DuplicateId(id.clone()));
                }
            }
        }
        for o in options {
            self.add_option(o)?;
        }
        Ok(())
    }

    /// Add a separator after the last option (Textual: `add_option(None)`).
    pub fn add_separator(&mut self) {
        if let Some(last) = self.options.last_mut() {
            last.divider = true;
        }
        self.clear_caches();
    }

    /// Remove every option.
    pub fn clear_options(&mut self, ctx: &mut Ctx) {
        self.options.clear();
        self.id_to_index.clear();
        self.highlighted = None;
        self.hover = None;
        self.clear_caches();
        ctx.scroll_to(0, 0);
    }

    /// The option with the given id.
    pub fn get_option(&self, id: &str) -> Result<&OptionItem, OptionListError> {
        self.id_to_index
            .get(id)
            .map(|i| &self.options[*i])
            .ok_or_else(|| OptionListError::OptionDoesNotExist(format!("with id {id:?}")))
    }

    pub fn get_option_index(&self, id: &str) -> Result<usize, OptionListError> {
        self.id_to_index
            .get(id)
            .copied()
            .ok_or_else(|| OptionListError::OptionDoesNotExist(format!("with id {id:?}")))
    }

    pub fn get_option_at_index(&self, index: usize) -> Result<&OptionItem, OptionListError> {
        self.options
            .get(index)
            .ok_or_else(|| OptionListError::OptionDoesNotExist(format!("at index {index}")))
    }

    fn rebuild_id_index(&mut self) {
        self.id_to_index = self
            .options
            .iter()
            .enumerate()
            .filter_map(|(i, o)| o.id.clone().map(|id| (id, i)))
            .collect();
    }

    /// Remove the option at `index`.
    pub fn remove_option_at_index(
        &mut self,
        index: usize,
        ctx: &mut Ctx,
    ) -> Result<OptionItem, OptionListError> {
        if index >= self.options.len() {
            return Err(OptionListError::OptionDoesNotExist(format!(
                "at index {index}"
            )));
        }
        self.hover = None;
        let removed = self.options.remove(index);
        self.rebuild_id_index();
        self.clear_caches();
        let h = self.highlighted;
        self.set_highlighted(h, ctx);
        Ok(removed)
    }

    /// Remove the option with the given id.
    pub fn remove_option(
        &mut self,
        id: &str,
        ctx: &mut Ctx,
    ) -> Result<OptionItem, OptionListError> {
        let index = self.get_option_index(id)?;
        self.remove_option_at_index(index, ctx)
    }

    fn set_option_disabled(
        &mut self,
        index: usize,
        disabled: bool,
        ctx: &mut Ctx,
    ) -> Result<(), OptionListError> {
        let o = self
            .options
            .get_mut(index)
            .ok_or_else(|| OptionListError::OptionDoesNotExist(format!("at index {index}")))?;
        o.disabled = disabled;
        if Some(index) == self.highlighted {
            let next = self.find_next_enabled(Some(index), 1);
            self.set_highlighted(next, ctx);
        }
        Ok(())
    }

    pub fn enable_option_at_index(
        &mut self,
        index: usize,
        ctx: &mut Ctx,
    ) -> Result<(), OptionListError> {
        self.set_option_disabled(index, false, ctx)
    }

    pub fn disable_option_at_index(
        &mut self,
        index: usize,
        ctx: &mut Ctx,
    ) -> Result<(), OptionListError> {
        self.set_option_disabled(index, true, ctx)
    }

    pub fn enable_option(&mut self, id: &str, ctx: &mut Ctx) -> Result<(), OptionListError> {
        let i = self.get_option_index(id)?;
        self.set_option_disabled(i, false, ctx)
    }

    pub fn disable_option(&mut self, id: &str, ctx: &mut Ctx) -> Result<(), OptionListError> {
        let i = self.get_option_index(id)?;
        self.set_option_disabled(i, true, ctx)
    }

    /// Replace the prompt of the option at `index`.
    pub fn replace_option_prompt_at_index(
        &mut self,
        index: usize,
        prompt: impl Into<String>,
    ) -> Result<(), OptionListError> {
        let o = self
            .options
            .get_mut(index)
            .ok_or_else(|| OptionListError::OptionDoesNotExist(format!("at index {index}")))?;
        o.prompt = Prompt::Markup(prompt.into());
        self.clear_caches();
        Ok(())
    }

    pub fn replace_option_prompt(
        &mut self,
        id: &str,
        prompt: impl Into<String>,
    ) -> Result<(), OptionListError> {
        let i = self.get_option_index(id)?;
        self.replace_option_prompt_at_index(i, prompt)
    }

    // ------------------------------------------------------------------ navigation

    fn find_first_enabled(&self) -> Option<usize> {
        self.options.iter().position(|o| !o.disabled)
    }

    fn find_last_enabled(&self) -> Option<usize> {
        self.options.iter().rposition(|o| !o.disabled)
    }

    /// `_widget_navigation.find_next_enabled` (wraps around).
    fn find_next_enabled(&self, anchor: Option<usize>, direction: i32) -> Option<usize> {
        let n = self.options.len();
        let Some(anchor) = anchor else {
            return if direction > 0 {
                self.find_first_enabled()
            } else {
                self.find_last_enabled()
            };
        };
        for step in 1..=n {
            let i = (anchor as i64 + direction as i64 * step as i64).rem_euclid(n as i64) as usize;
            if !self.options[i].disabled {
                return Some(i);
            }
        }
        Some(anchor)
    }

    /// `find_next_enabled_no_wrap` with `with_anchor=True`.
    fn find_next_enabled_no_wrap(&self, anchor: usize, direction: i32) -> Option<usize> {
        if direction > 0 {
            (anchor..self.options.len()).find(|i| !self.options[*i].disabled)
        } else {
            (0..=anchor.min(self.options.len().saturating_sub(1)))
                .rev()
                .find(|i| !self.options[*i].disabled)
        }
    }

    /// Set the highlighted option (clamped). Scrolls it into view and posts
    /// [`OptionHighlighted`] if it is enabled.
    pub fn set_highlighted(&mut self, highlighted: Option<usize>, ctx: &mut Ctx) {
        let h = match highlighted {
            _ if self.options.is_empty() => None,
            None => None,
            Some(i) => Some(i.min(self.options.len() - 1)),
        };
        self.highlighted = h;
        let Some(index) = h else { return };
        if !self.options[index].disabled {
            self.scroll_to_highlight(ctx);
            ctx.post_message(OptionHighlighted {
                option_index: index,
                option_id: self.options[index].id.clone(),
            });
        }
    }

    /// Scroll so the highlighted option is visible.
    pub fn scroll_to_highlight(&self, ctx: &mut Ctx) {
        let Some(h) = self.highlighted else { return };
        let width = ctx.size().width;
        let (y, height) = self.with_cache_ctx(ctx, |c| (c.index_to_line[h], c.heights[h]));
        ctx.scroll_to_show(Region::new(0, y as i32, width, height as i32));
    }

    pub fn action_cursor_up(&mut self, ctx: &mut Ctx) {
        let n = self.find_next_enabled(self.highlighted, -1);
        self.set_highlighted(n, ctx);
    }

    pub fn action_cursor_down(&mut self, ctx: &mut Ctx) {
        let n = self.find_next_enabled(self.highlighted, 1);
        self.set_highlighted(n, ctx);
    }

    pub fn action_first(&mut self, ctx: &mut Ctx) {
        let n = self.find_first_enabled();
        self.set_highlighted(n, ctx);
    }

    pub fn action_last(&mut self, ctx: &mut Ctx) {
        let n = self.find_last_enabled();
        self.set_highlighted(n, ctx);
    }

    fn move_page(&mut self, direction: i32, ctx: &mut Ctx) {
        if self.options.is_empty() {
            return;
        }
        let height = ctx.size().height;
        let cur = self.highlighted.unwrap_or(0);
        let option_index = self.with_cache_ctx(ctx, |c| {
            let y = (c.index_to_line[cur] as i32 + direction * height)
                .clamp(0, c.lines.len() as i32 - 1);
            c.lines[y as usize].0
        });
        let n = self.find_next_enabled_no_wrap(option_index, direction);
        self.set_highlighted(n, ctx);
    }

    pub fn action_page_up(&mut self, ctx: &mut Ctx) {
        if self.highlighted.is_none() {
            self.action_first(ctx)
        } else {
            self.move_page(-1, ctx)
        }
    }

    pub fn action_page_down(&mut self, ctx: &mut Ctx) {
        if self.highlighted.is_none() {
            self.action_last(ctx)
        } else {
            self.move_page(1, ctx)
        }
    }

    /// Post [`OptionSelected`] for the highlighted option (if enabled).
    pub fn action_select(&mut self, ctx: &mut Ctx) {
        let Some(h) = self.highlighted else { return };
        let option = &self.options[h];
        if !option.disabled {
            ctx.post_message(OptionSelected {
                option_index: h,
                option_id: option.id.clone(),
            });
        }
    }

    // ------------------------------------------------------------------ line cache

    fn prompt_text(&self, option: &OptionItem, markup: &dyn Fn(&str) -> Text) -> Text {
        match &option.prompt {
            Prompt::Markup(m) if self.markup => markup(m),
            Prompt::Markup(m) => Text::plain(m.clone()),
            Prompt::Text(t) => t.clone(),
        }
    }

    fn build_cache(&self, width: i32, markup: &dyn Fn(&str) -> Text) -> LineCache {
        let mut c = LineCache {
            width,
            ..Default::default()
        };
        for (index, option) in self.options.iter().enumerate() {
            let text = self.prompt_text(option, markup);
            let mut strips = text.wrap(width.max(1), Justify::Left, TextOverflow::Fold, false);
            if strips.is_empty() {
                strips.push(Strip::default());
            }
            let height = strips.len() + usize::from(option.divider);
            c.index_to_line.push(c.lines.len());
            c.heights.push(height);
            c.lines.extend((0..height).map(|l| (index, l)));
            c.strips.push(strips);
        }
        c
    }

    fn with_cache<R>(
        &self,
        width: i32,
        markup: &dyn Fn(&str) -> Text,
        f: impl FnOnce(&LineCache) -> R,
    ) -> R {
        let mut caches = self.cache.borrow_mut();
        let pos = match caches.iter().position(|c| c.width == width) {
            Some(p) => p,
            None => {
                if caches.len() >= 2 {
                    caches.remove(0);
                }
                caches.push(self.build_cache(width, markup));
                caches.len() - 1
            }
        };
        f(&caches[pos])
    }

    fn with_cache_ctx<R>(&self, ctx: &Ctx, f: impl FnOnce(&LineCache) -> R) -> R {
        let vars = ctx.app.env.stylesheet.variables();
        let markup = |s: &str| {
            Text::from_markup_with(s, &|name| vars.get(name).and_then(|v| parse_color_value(v)))
        };
        self.with_cache(ctx.size().width, &markup, f)
    }

    fn virtual_height(c: &LineCache, options: &[OptionItem]) -> usize {
        let last_divider = options.last().is_some_and(|o| o.divider);
        c.lines.len() - usize::from(last_divider)
    }

    /// Option index at content row `y` (viewport coordinates), if any.
    fn option_at(&self, y: i32, ctx: &Ctx) -> Option<usize> {
        let line = (y + ctx.scroll_offset().y) as usize;
        self.with_cache_ctx(ctx, |c| {
            let (index, offset) = *c.lines.get(line)?;
            // A divider line does not belong to the option.
            (offset < c.strips[index].len()).then_some(index)
        })
    }
}

impl Widget for OptionList {
    fn type_names(&self) -> &'static [&'static str] {
        &["OptionList", "ScrollView"]
    }

    fn default_css(&self) -> &'static str {
        DEFAULT_CSS
    }

    fn can_focus(&self) -> bool {
        true
    }

    fn on_mount(&mut self, ctx: &mut Ctx) {
        ctx.set_class(self.compact, "-textual-compact");
    }

    fn bindings(&self) -> Vec<Binding> {
        vec![
            Binding::new("down", "cursor_down", "Down").hidden(),
            Binding::new("end", "last", "Last").hidden(),
            Binding::new("enter", "select", "Select").hidden(),
            Binding::new("home", "first", "First").hidden(),
            Binding::new("pagedown", "page_down", "Page Down").hidden(),
            Binding::new("pageup", "page_up", "Page Up").hidden(),
            Binding::new("up", "cursor_up", "Up").hidden(),
        ]
    }

    fn run_action(&mut self, action: &str, _args: &[String], ctx: &mut Ctx) -> bool {
        match action {
            "cursor_down" => self.action_cursor_down(ctx),
            "cursor_up" => self.action_cursor_up(ctx),
            "first" => self.action_first(ctx),
            "last" => self.action_last(ctx),
            "page_down" => self.action_page_down(ctx),
            "page_up" => self.action_page_up(ctx),
            "select" => self.action_select(ctx),
            _ => return false,
        }
        true
    }

    fn on_event(&mut self, event: &Event, ctx: &mut Ctx) -> Flow {
        match event {
            Event::Click(m) => {
                let content = ctx.content_region();
                if !content.contains(m.screen_x, m.screen_y) {
                    return Flow::Continue;
                }
                if let Some(index) = self.option_at(m.screen_y - content.y, ctx) {
                    if !self.options[index].disabled {
                        self.set_highlighted(Some(index), ctx);
                        self.action_select(ctx);
                    }
                }
                Flow::Stop
            }
            Event::MouseMove(m) => {
                let content = ctx.content_region();
                self.hover = if content.contains(m.screen_x, m.screen_y) {
                    self.option_at(m.screen_y - content.y, ctx)
                } else {
                    None
                };
                Flow::Continue
            }
            Event::Leave => {
                self.hover = None;
                Flow::Continue
            }
            _ => Flow::Continue,
        }
    }

    fn virtual_size(&self, ctx: &RenderCtx) -> Option<Size> {
        let width = ctx.size.width;
        let markup = |s: &str| ctx.markup(s);
        let h = self.with_cache(width, &markup, |c| Self::virtual_height(c, &self.options));
        Some(Size::new(width, h as i32))
    }

    fn content_width(&self, ctx: &RenderCtx) -> Option<i32> {
        let markup = |s: &str| ctx.markup(s);
        Some(
            self.options
                .iter()
                .map(|o| self.prompt_text(o, &markup).cell_len())
                .max()
                .unwrap_or(0),
        )
    }

    fn content_height(&self, ctx: &RenderCtx, width: i32) -> Option<i32> {
        let markup = |s: &str| ctx.markup(s);
        let h = self.with_cache(width, &markup, |c| Self::virtual_height(c, &self.options));
        Some(h as i32)
    }

    fn render(&self, ctx: &RenderCtx) -> Content {
        let Size { width, height } = ctx.size;
        let scroll_y = ctx.scroll_offset.y;
        let markup = |s: &str| ctx.markup(s);
        let base = ctx.component_style("option-list--option");
        let class_style = |class: &str| -> Style { base.combine(&ctx.component_style(class)) };
        let highlighted = class_style("option-list--option-highlighted");
        let disabled = class_style("option-list--option-disabled");
        let hover = class_style("option-list--option-hover");
        let separator = ctx.component_style("option-list--separator");
        self.with_cache(width, &markup, |c| {
            let mut out = Vec::with_capacity(height.max(0) as usize);
            for y in 0..height.max(0) {
                let line = (scroll_y + y) as usize;
                let Some(&(index, offset)) = c.lines.get(line) else {
                    break;
                };
                let option = &self.options[index];
                let strips = &c.strips[index];
                let Some(strip) = strips.get(offset) else {
                    // The divider after the option.
                    out.push(Strip::from_str(
                        &"─".repeat(width.max(0) as usize),
                        separator.clone(),
                    ));
                    continue;
                };
                let style = if option.disabled {
                    &disabled
                } else if self.highlighted == Some(index) {
                    &highlighted
                } else if self.hover == Some(index) {
                    &hover
                } else {
                    &base
                };
                out.push(strip.apply_base_style(style).set_cell_len(width, style));
            }
            Content::Lines(out)
        })
    }
}

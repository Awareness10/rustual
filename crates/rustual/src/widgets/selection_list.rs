//! `SelectionList` (port of `textual/widgets/_selection_list.py`): an option list where each
//! option has a check button (`▐X▌`) and a value; any number of options can be selected.
//!
//! ```ignore
//! SelectionList::new()
//!     .selection(("Falken's Maze", "secret_back_door"))
//!     .selection(("Black Jack", "black_jack", true))   // initially selected
//!     .selection(Selection::new("Gin Rummy", "gin_rummy").id("gin"))
//! ```
//!
//! `space` / `enter` / a click toggle the highlighted selection. Messages are generic over the
//! value type: match them with `msg.get::<selection_list::SelectedChanged<&str>>()`.
//!
//! Deviations from Textual:
//! - Self-contained (not an `OptionList` subclass), but it is styled by OptionList's CSS:
//!   its CSS type names are `SelectionList`, `OptionList`, `ScrollView`.
//! - Separators and plain options cannot be added (Textual raises `SelectionError` for them).
//! - Values are compared with `PartialEq` (Textual uses them as dict keys); they should be unique.
//! - Messages carry the selection's index, id and value instead of a reference to the list.

use crate::binding::Binding;
use crate::context::Ctx;
use crate::events::Event;
use crate::message::Flow;
use crate::widget::{Content, RenderCtx, Widget};
use crate::widgets::option_list::{option_list_css, OptionListError};
use crate::widgets::toggle_button::{BUTTON_LEFT, BUTTON_RIGHT};
use rustual_render::{Justify, Region, Size, Span, Strip, Style, Text, TextOverflow};
use std::collections::HashMap;

const DEFAULT_CSS: &str = concat!(
    option_list_css!(),
    r#"
SelectionList {
    height: auto;
    text-wrap: nowrap;
    text-overflow: ellipsis;

    & > .selection-list--button {
        color: $panel-darken-2;
        background: $panel;
    }

    & > .selection-list--button-highlighted {
        color: $panel-darken-2;
        background: $panel;
    }

    & > .selection-list--button-selected {
        color: $text-success;
        background: $panel;
    }

    & > .selection-list--button-selected-highlighted {
        color: $text-success;
        background: $panel;
    }
}
"#
);

/// The inner glyph of the check button.
const BUTTON_INNER: &str = "X";
/// Width of the button gutter: `▐X▌ `.
const GUTTER: i32 = 4;

// ---------------------------------------------------------------- messages

/// Posted when a selection is highlighted (Textual: `SelectionList.SelectionHighlighted`).
#[derive(Debug, Clone, PartialEq)]
pub struct SelectionHighlighted<T> {
    pub selection_index: usize,
    pub selection_id: Option<String>,
    pub value: T,
    /// CSS id of the selection list.
    pub selection_list_id: Option<String>,
}

/// Posted when a selection is explicitly toggled (by the user, `toggle` or `toggle_all`)
/// (Textual: `SelectionList.SelectionToggled`). One message per toggled option.
#[derive(Debug, Clone, PartialEq)]
pub struct SelectionToggled<T> {
    pub selection_index: usize,
    pub selection_id: Option<String>,
    pub value: T,
    pub selection_list_id: Option<String>,
}

/// Posted when the collection of selected values changes, however it changed
/// (Textual: `SelectionList.SelectedChanged`). Bulk changes post a single message.
#[derive(Debug, Clone, PartialEq)]
pub struct SelectedChanged<T> {
    /// The selected values, in the order they were selected.
    pub selected: Vec<T>,
    pub selection_list_id: Option<String>,
}

// ---------------------------------------------------------------- Selection

/// One option of a [`SelectionList`] (Textual: `Selection`).
#[derive(Debug, Clone, PartialEq)]
pub struct Selection<T> {
    prompt: String,
    value: T,
    initial_state: bool,
    id: Option<String>,
    disabled: bool,
}

impl<T> Selection<T> {
    /// A selection with a (markup) prompt; only its first line is used.
    pub fn new(prompt: impl Into<String>, value: T) -> Self {
        let prompt: String = prompt.into();
        Selection {
            prompt: prompt.lines().next().unwrap_or("").to_string(),
            value,
            initial_state: false,
            id: None,
            disabled: false,
        }
    }

    /// Initially selected?
    pub fn initial_state(mut self, selected: bool) -> Self {
        self.initial_state = selected;
        self
    }

    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn value(&self) -> &T {
        &self.value
    }

    pub fn prompt(&self) -> &str {
        &self.prompt
    }

    /// The prompt without markup.
    pub fn plain_prompt(&self) -> String {
        Text::from_markup(&self.prompt).plain_text()
    }

    pub fn get_id(&self) -> Option<&str> {
        self.id.as_deref()
    }

    pub fn is_disabled(&self) -> bool {
        self.disabled
    }

    pub fn get_initial_state(&self) -> bool {
        self.initial_state
    }
}

impl<P: Into<String>, T> From<(P, T)> for Selection<T> {
    fn from((prompt, value): (P, T)) -> Self {
        Selection::new(prompt, value)
    }
}

impl<P: Into<String>, T> From<(P, T, bool)> for Selection<T> {
    fn from((prompt, value, selected): (P, T, bool)) -> Self {
        Selection::new(prompt, value).initial_state(selected)
    }
}

// ---------------------------------------------------------------- SelectionList

/// A vertical list of selections, allowing multiple selections.
#[derive(Debug, Clone)]
pub struct SelectionList<T: 'static> {
    options: Vec<Selection<T>>,
    id_to_index: HashMap<String, usize>,
    /// Indices of the selected options, in selection order.
    selected: Vec<usize>,
    highlighted: Option<usize>,
    hover: Option<usize>,
    compact: bool,
}

impl<T: Clone + PartialEq + Send + 'static> Default for SelectionList<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Clone + PartialEq + Send + 'static> SelectionList<T> {
    pub fn new() -> Self {
        SelectionList {
            options: Vec::new(),
            id_to_index: HashMap::new(),
            selected: Vec::new(),
            highlighted: None,
            hover: None,
            compact: false,
        }
    }

    /// Build from selections or `(prompt, value)` / `(prompt, value, selected)` tuples.
    pub fn from_selections<I, S>(selections: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<Selection<T>>,
    {
        let mut list = Self::new();
        for s in selections {
            list = list.selection(s);
        }
        list
    }

    /// Builder: add a selection. Panics on a duplicate id (use [`SelectionList::add_option`]).
    pub fn selection(mut self, selection: impl Into<Selection<T>>) -> Self {
        self.add_option(selection).expect("duplicate selection id");
        self
    }

    /// Compact style (no border / padding); sets the `-textual-compact` class on mount.
    pub fn compact(mut self, compact: bool) -> Self {
        self.compact = compact;
        self
    }

    // ------------------------------------------------------------ options

    pub fn options(&self) -> &[Selection<T>] {
        &self.options
    }

    pub fn option_count(&self) -> usize {
        self.options.len()
    }

    /// Add a selection at the end (an initially selected one is selected without a message).
    pub fn add_option(
        &mut self,
        selection: impl Into<Selection<T>>,
    ) -> Result<(), OptionListError> {
        let selection = selection.into();
        if let Some(id) = &selection.id {
            if self.id_to_index.contains_key(id) {
                return Err(OptionListError::DuplicateId(id.clone()));
            }
            self.id_to_index.insert(id.clone(), self.options.len());
        }
        let index = self.options.len();
        let initially = selection.initial_state;
        let value = selection.value.clone();
        self.options.push(selection);
        if initially && !self.is_selected(&value) {
            self.selected.push(index);
        }
        if self.highlighted.is_none() {
            self.highlighted = self.find_next_enabled(None, 1);
        }
        Ok(())
    }

    /// Add several selections (checked for duplicate ids first; nothing is added on error).
    pub fn add_options<I, S>(&mut self, selections: I) -> Result<(), OptionListError>
    where
        I: IntoIterator<Item = S>,
        S: Into<Selection<T>>,
    {
        let selections: Vec<Selection<T>> = selections.into_iter().map(Into::into).collect();
        let mut seen = std::collections::HashSet::new();
        for s in &selections {
            if let Some(id) = &s.id {
                if self.id_to_index.contains_key(id) || !seen.insert(id.clone()) {
                    return Err(OptionListError::DuplicateId(id.clone()));
                }
            }
        }
        for s in selections {
            self.add_option(s)?;
        }
        Ok(())
    }

    pub fn get_option_at_index(&self, index: usize) -> Result<&Selection<T>, OptionListError> {
        self.options
            .get(index)
            .ok_or_else(|| OptionListError::OptionDoesNotExist(format!("at index {index}")))
    }

    pub fn get_option(&self, id: &str) -> Result<&Selection<T>, OptionListError> {
        self.get_option_index(id).map(|i| &self.options[i])
    }

    pub fn get_option_index(&self, id: &str) -> Result<usize, OptionListError> {
        self.id_to_index
            .get(id)
            .copied()
            .ok_or_else(|| OptionListError::OptionDoesNotExist(format!("with id {id:?}")))
    }

    /// Remove the selection at `index` (deselecting it first, which posts [`SelectedChanged`]).
    pub fn remove_option_at_index(
        &mut self,
        index: usize,
        ctx: &mut Ctx,
    ) -> Result<Selection<T>, OptionListError> {
        if index >= self.options.len() {
            return Err(OptionListError::OptionDoesNotExist(format!(
                "at index {index}"
            )));
        }
        if self.selected.contains(&index) {
            self.selected.retain(|i| *i != index);
            self.post_changed(ctx);
        }
        for i in &mut self.selected {
            if *i > index {
                *i -= 1;
            }
        }
        let removed = self.options.remove(index);
        self.id_to_index = self
            .options
            .iter()
            .enumerate()
            .filter_map(|(i, o)| o.id.clone().map(|id| (id, i)))
            .collect();
        self.hover = None;
        let h = self.highlighted.filter(|_| !self.options.is_empty());
        self.set_highlighted(h, ctx);
        Ok(removed)
    }

    /// Remove the selection with the given id.
    pub fn remove_option(
        &mut self,
        id: &str,
        ctx: &mut Ctx,
    ) -> Result<Selection<T>, OptionListError> {
        let index = self.get_option_index(id)?;
        self.remove_option_at_index(index, ctx)
    }

    /// Remove every selection.
    pub fn clear_options(&mut self, ctx: &mut Ctx) {
        self.options.clear();
        self.id_to_index.clear();
        self.selected.clear();
        self.highlighted = None;
        self.hover = None;
        ctx.scroll_to(0, 0);
    }

    /// Enable or disable the selection at `index`.
    pub fn set_option_disabled(
        &mut self,
        index: usize,
        disabled: bool,
    ) -> Result<(), OptionListError> {
        let o = self
            .options
            .get_mut(index)
            .ok_or_else(|| OptionListError::OptionDoesNotExist(format!("at index {index}")))?;
        o.disabled = disabled;
        Ok(())
    }

    // ------------------------------------------------------------ selection state

    /// The selected values, in the order they were selected.
    pub fn selected(&self) -> Vec<T> {
        self.selected
            .iter()
            .map(|i| self.options[*i].value.clone())
            .collect()
    }

    pub fn is_selected(&self, value: &T) -> bool {
        self.selected
            .iter()
            .any(|i| self.options[*i].value == *value)
    }

    fn index_of(&self, value: &T) -> Option<usize> {
        self.options.iter().position(|o| o.value == *value)
    }

    fn list_id(ctx: &Ctx) -> Option<String> {
        ctx.css_id(ctx.id()).map(String::from)
    }

    fn post_changed(&self, ctx: &mut Ctx) {
        let selection_list_id = Self::list_id(ctx);
        ctx.post_message(SelectedChanged {
            selected: self.selected(),
            selection_list_id,
        });
    }

    fn post_toggled(&self, index: usize, ctx: &mut Ctx) {
        let o = &self.options[index];
        let selection_list_id = Self::list_id(ctx);
        ctx.post_message(SelectionToggled {
            selection_index: index,
            selection_id: o.id.clone(),
            value: o.value.clone(),
            selection_list_id,
        });
    }

    /// Select without messages; returns whether anything changed.
    fn select_index(&mut self, index: usize) -> bool {
        if self.selected.contains(&index) {
            return false;
        }
        self.selected.push(index);
        true
    }

    fn deselect_index(&mut self, index: usize) -> bool {
        let before = self.selected.len();
        self.selected.retain(|i| *i != index);
        before != self.selected.len()
    }

    /// Mark the selection with `value` as selected. Posts [`SelectedChanged`] if it changed.
    pub fn select(&mut self, value: &T, ctx: &mut Ctx) -> bool {
        let changed = self.index_of(value).is_some_and(|i| self.select_index(i));
        if changed {
            self.post_changed(ctx);
        }
        changed
    }

    /// Mark the selection with `value` as not selected. Posts [`SelectedChanged`] if it changed.
    pub fn deselect(&mut self, value: &T, ctx: &mut Ctx) -> bool {
        let changed = self.index_of(value).is_some_and(|i| self.deselect_index(i));
        if changed {
            self.post_changed(ctx);
        }
        changed
    }

    /// Toggle the selection with `value`: posts [`SelectedChanged`] and [`SelectionToggled`].
    pub fn toggle(&mut self, value: &T, ctx: &mut Ctx) {
        if let Some(i) = self.index_of(value) {
            self.toggle_index(i, ctx);
        }
    }

    /// Toggle the selection at `index`.
    pub fn toggle_index(&mut self, index: usize, ctx: &mut Ctx) {
        if index >= self.options.len() {
            return;
        }
        if !self.deselect_index(index) {
            self.select_index(index);
        }
        self.post_changed(ctx);
        self.post_toggled(index, ctx);
    }

    /// Select every selection (one [`SelectedChanged`] if anything changed).
    pub fn select_all(&mut self, ctx: &mut Ctx) {
        let mut changed = false;
        for i in 0..self.options.len() {
            changed |= self.select_index(i);
        }
        if changed {
            self.post_changed(ctx);
        }
    }

    /// Deselect every selection (one [`SelectedChanged`] if anything changed).
    pub fn deselect_all(&mut self, ctx: &mut Ctx) {
        if !self.selected.is_empty() {
            self.selected.clear();
            self.post_changed(ctx);
        }
    }

    /// Toggle every selection: one [`SelectionToggled`] per option and one [`SelectedChanged`].
    pub fn toggle_all(&mut self, ctx: &mut Ctx) {
        if self.options.is_empty() {
            return;
        }
        for i in 0..self.options.len() {
            if !self.deselect_index(i) {
                self.select_index(i);
            }
            self.post_toggled(i, ctx);
        }
        self.post_changed(ctx);
    }

    // ------------------------------------------------------------ highlight & navigation

    /// Index of the highlighted selection.
    pub fn highlighted(&self) -> Option<usize> {
        self.highlighted
    }

    /// Set the highlighted selection (clamped). Scrolls it into view and posts
    /// [`SelectionHighlighted`] if it is enabled.
    pub fn set_highlighted(&mut self, highlighted: Option<usize>, ctx: &mut Ctx) {
        let h = match highlighted {
            _ if self.options.is_empty() => None,
            None => None,
            Some(i) => Some(i.min(self.options.len() - 1)),
        };
        self.highlighted = h;
        let Some(index) = h else { return };
        let o = &self.options[index];
        if !o.disabled {
            ctx.scroll_to_show(Region::new(0, index as i32, 1, 1));
            let selection_list_id = Self::list_id(ctx);
            ctx.post_message(SelectionHighlighted {
                selection_index: index,
                selection_id: o.id.clone(),
                value: o.value.clone(),
                selection_list_id,
            });
        }
    }

    /// Wrapping search for the next enabled option (`_widget_navigation.find_next_enabled`).
    fn find_next_enabled(&self, anchor: Option<usize>, direction: i32) -> Option<usize> {
        let n = self.options.len();
        let enabled = |i: &usize| !self.options[*i].disabled;
        let Some(anchor) = anchor else {
            return if direction > 0 {
                (0..n).find(enabled)
            } else {
                (0..n).rev().find(enabled)
            };
        };
        for step in 1..=n {
            let i = (anchor as i64 + direction as i64 * step as i64).rem_euclid(n as i64) as usize;
            if enabled(&i) {
                return Some(i);
            }
        }
        Some(anchor)
    }

    fn move_page(&mut self, direction: i32, ctx: &mut Ctx) {
        if self.options.is_empty() {
            return;
        }
        let page = ctx.size().height.max(1);
        let n = self.options.len() as i32;
        let target = (self.highlighted.unwrap_or(0) as i32 + direction * page).clamp(0, n - 1);
        let enabled = |i: &i32| !self.options[*i as usize].disabled;
        let found = if direction > 0 {
            (target..n)
                .find(enabled)
                .or_else(|| (0..target).rev().find(enabled))
        } else {
            (0..=target)
                .rev()
                .find(enabled)
                .or_else(|| (target..n).find(enabled))
        };
        self.set_highlighted(found.map(|i| i as usize), ctx);
    }

    fn action(&mut self, action: &str, ctx: &mut Ctx) -> bool {
        match action {
            "cursor_down" => {
                let n = self.find_next_enabled(self.highlighted, 1);
                self.set_highlighted(n, ctx);
            }
            "cursor_up" => {
                let n = self.find_next_enabled(self.highlighted, -1);
                self.set_highlighted(n, ctx);
            }
            "first" => {
                let n = self.find_next_enabled(None, 1);
                self.set_highlighted(n, ctx);
            }
            "last" => {
                let n = self.find_next_enabled(None, -1);
                self.set_highlighted(n, ctx);
            }
            "page_down" => self.move_page(1, ctx),
            "page_up" => self.move_page(-1, ctx),
            "select" => {
                if let Some(h) = self.highlighted.filter(|h| !self.options[*h].disabled) {
                    self.toggle_index(h, ctx);
                }
            }
            _ => return false,
        }
        true
    }

    /// Option index at content row `y` (viewport coordinates).
    fn option_at(&self, y: i32, ctx: &Ctx) -> Option<usize> {
        let i = y + ctx.scroll_offset().y;
        usize::try_from(i).ok().filter(|i| *i < self.options.len())
    }

    // ------------------------------------------------------------ rendering

    fn render_line(&self, ctx: &RenderCtx, index: usize, width: i32) -> Strip {
        let option = &self.options[index];
        let base = ctx.component_style("option-list--option");
        let class_style = |class: &str| -> Style { base.combine(&ctx.component_style(class)) };
        let highlighted = self.highlighted == Some(index);
        let line_style = if option.disabled {
            class_style("option-list--option-disabled")
        } else if highlighted {
            class_style("option-list--option-highlighted")
        } else if self.hover == Some(index) {
            class_style("option-list--option-hover")
        } else {
            base.clone()
        };

        let mut class = String::from("selection-list--button");
        if self.selected.contains(&index) {
            class.push_str("-selected");
        }
        if highlighted {
            class.push_str("-highlighted");
        }
        let button_style = ctx.component_style(&class);
        let under_bg = line_style.bg.or_else(|| Some(ctx.background()));
        let side_style = Style {
            fg: button_style.bg,
            bg: under_bg,
            ..Style::default()
        };

        let mut strip = Strip::default();
        strip.push(Span::new(BUTTON_LEFT, side_style.clone()));
        strip.push(Span::new(BUTTON_INNER, button_style));
        strip.push(Span::new(BUTTON_RIGHT, side_style));
        strip.push(Span::new(" ", line_style.clone()));
        let (overflow, _) = ctx.text_overflow();
        let overflow = match overflow {
            TextOverflow::Fold => TextOverflow::Clip,
            o => o,
        };
        let prompt = ctx.markup(&option.prompt);
        let prompt_width = (width - GUTTER).max(0);
        if let Some(line) = prompt
            .wrap(prompt_width, Justify::Left, overflow, true)
            .into_iter()
            .next()
        {
            for span in line.apply_base_style(&line_style).spans {
                strip.push(span);
            }
        }
        strip.set_cell_len(width, &line_style)
    }
}

impl<T: Clone + PartialEq + Send + 'static> Widget for SelectionList<T> {
    fn type_names(&self) -> &'static [&'static str] {
        &["SelectionList", "OptionList", "ScrollView"]
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
            Binding::new("space", "select", "Toggle option").hidden(),
        ]
    }

    fn run_action(&mut self, action: &str, _args: &[String], ctx: &mut Ctx) -> bool {
        self.action(action, ctx)
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
                        if self.highlighted != Some(index) {
                            self.set_highlighted(Some(index), ctx);
                        }
                        self.toggle_index(index, ctx);
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
        Some(Size::new(ctx.size.width, self.options.len() as i32))
    }

    fn content_width(&self, ctx: &RenderCtx) -> Option<i32> {
        Some(
            self.options
                .iter()
                .map(|o| ctx.markup(&o.prompt).cell_len() + GUTTER)
                .max()
                .unwrap_or(0),
        )
    }

    fn content_height(&self, _ctx: &RenderCtx, _width: i32) -> Option<i32> {
        Some(self.options.len() as i32)
    }

    fn render(&self, ctx: &RenderCtx) -> Content {
        let Size { width, height } = ctx.size;
        let scroll_y = ctx.scroll_offset.y.max(0) as usize;
        let lines = (0..height.max(0) as usize)
            .map_while(|y| {
                let index = scroll_y + y;
                (index < self.options.len()).then(|| self.render_line(ctx, index, width))
            })
            .collect();
        Content::Lines(lines)
    }
}

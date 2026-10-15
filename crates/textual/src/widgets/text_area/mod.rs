//! `TextArea`: a multi-line text editor (port of `textual/widgets/_text_area.py`).
//!
//! Features: a [`Document`] model with undoable [`Edit`]s batched into checkpoints
//! ([`EditHistory`]), soft wrapping ([`WrappedDocument`]) with wrapping-aware navigation,
//! selection (keyboard, mouse drag, double / triple click), an internal clipboard shared with
//! `Input`, line numbers, cursor-line / matching-bracket highlighting, read-only mode, a
//! placeholder, tab expansion, themes ([`TextAreaTheme`]) and pluggable syntax highlighting
//! ([`Highlighter`]; Textual uses tree-sitter, which is not used here).
//!
//! The widget scrolls itself (it reports a `virtual_size`) and renders only the visible lines,
//! so large documents stay responsive.
//!
//! Mutating methods take the widget's [`Ctx`] (they post messages and scroll), so from
//! elsewhere use `ctx.with::<TextArea, _>("#editor", |ta, ctx| ta.insert("hi", None, ctx))`.

pub mod document;
pub mod edit;
pub mod highlight;
pub mod navigator;
pub mod theme;
pub mod wrap;

pub use document::{Document, EditResult, Location, Newline, Selection};
pub use edit::{Edit, EditHistory};
pub use highlight::{Highlight, Highlighter, KeywordHighlighter, BUILTIN_LANGUAGES};
pub use navigator::Navigator;
pub use theme::TextAreaTheme;
pub use wrap::WrappedDocument;

use crate::app::TimerId;
use crate::binding::Binding;
use crate::context::Ctx;
use crate::events::{Event, MouseButton, MouseEvent};
use crate::message::Flow;
use crate::widget::{Content, RenderCtx, Widget};
use crate::widgets::input::set_clipboard_text;
use document::{display_char, line_cells};
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;
use std::time::Duration;
use textual_css::Overflow;
use textual_render::text::{Justify, Span};
use textual_render::{Region, Size, Strip, Style, Text, TextOverflow};
use tokio::time::Instant;

const DEFAULT_CSS: &str = r#"
TextArea {
    width: 1fr;
    height: 1fr;
    border: tall $border-blurred;
    padding: 0 1;
    color: $foreground;
    background: $surface;
    pointer: text;
    overflow-x: auto;
    overflow-y: auto;
    &.-textual-compact {
        border: none !important;
    }
    & .text-area--cursor {
        text-style: $input-cursor-text-style;
    }
    & .text-area--gutter {
        color: $foreground 40%;
    }

    & .text-area--cursor-gutter {
        color: $foreground 60%;
        background: $boost;
        text-style: bold;
    }

    & .text-area--cursor-line {
       background: $boost;
    }

    & .text-area--selection {
        background: $input-selection-background;
        color: $input-selection-foreground;
    }

    & .text-area--matching-bracket {
        background: $foreground 30%;
    }

    & .text-area--suggestion {
        color: $text-muted;
    }

    & .text-area--placeholder {
        color: $text 40%;
    }

    &:focus {
        border: tall $border;
    }

    &:ansi {
        .text-area--cursor {
            color: $input-cursor-foreground;
            background: $input-cursor-background;
            text-style: reverse;
        }
        & .text-area--selection {
            background: transparent;
            text-style: reverse;
        }
    }

    &:dark {
        .text-area--cursor {
            color: $input-cursor-foreground;
            background: $input-cursor-background;
        }
        &.-read-only .text-area--cursor {
            background: $warning-darken-1;
        }
    }

    &:light {
        .text-area--cursor {
            color: $text 90%;
            background: $foreground 70%;
        }
        &.-read-only .text-area--cursor {
            background: $warning-darken-1;
        }
    }
}
"#;

/// Cursor blink interval (Textual: 0.5s).
const BLINK_INTERVAL: Duration = Duration::from_millis(500);

/// Maximum delay between presses for a double / triple click.
const MULTI_CLICK_INTERVAL: Duration = Duration::from_millis(500);

const OPENING_BRACKETS: [(char, char); 3] = [('{', '}'), ('[', ']'), ('(', ')')];

/// Posted when the text changes (Textual: `TextArea.Changed`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Changed {
    /// The CSS id of the text area, if any.
    pub text_area_id: Option<String>,
}

/// Posted when the selection or cursor moves (Textual: `TextArea.SelectionChanged`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectionChanged {
    pub selection: Selection,
    pub text_area_id: Option<String>,
}

/// What the tab key does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TabBehavior {
    /// Move focus to the next widget (default).
    #[default]
    Focus,
    /// Insert an indent (escape then moves focus).
    Indent,
}

/// What an indent inserts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum IndentType {
    /// Spaces up to the next `indent_width` tab stop (default).
    #[default]
    Spaces,
    /// A tab character.
    Tabs,
}

/// Chrome styles resolved for one render (theme first, component classes as fallback).
struct Resolved {
    cursor: Style,
    gutter: Style,
    cursor_gutter: Style,
    cursor_line: Style,
    selection: Style,
    bracket: Style,
    placeholder: Style,
}

/// A multi-line text editor.
pub struct TextArea {
    document: Document,
    /// Lazily (re)wrapped at the width seen by the last render / handler.
    wrapped: RefCell<WrappedDocument>,
    selection: Selection,
    /// Remembered cursor x offset for vertical movement.
    last_x_offset: i32,
    history: EditHistory,
    language: Option<String>,
    highlighter: Option<Rc<dyn Highlighter>>,
    theme: TextAreaTheme,
    themes: HashMap<String, TextAreaTheme>,
    /// The theme's base style was written to the inline styles.
    theme_base_applied: bool,
    soft_wrap: bool,
    tab_behavior: TabBehavior,
    indent_type: IndentType,
    indent_width: usize,
    read_only: bool,
    show_cursor: bool,
    show_line_numbers: bool,
    line_number_start: usize,
    match_cursor_bracket: bool,
    cursor_blink: bool,
    highlight_cursor_line: bool,
    compact: bool,
    placeholder: String,
    cursor_visible: bool,
    blink_timer: Option<TimerId>,
    /// Mouse drag selection in progress.
    selecting: bool,
    /// Last mouse press (time, location, click count) for double / triple clicks.
    last_press: Option<(Instant, Location, u8)>,
    matching_bracket: Option<Location>,
}

impl fmt::Debug for TextArea {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TextArea")
            .field("line_count", &self.document.line_count())
            .field("selection", &self.selection)
            .field("language", &self.language)
            .field("theme", &self.theme.name)
            .field("soft_wrap", &self.soft_wrap)
            .field("read_only", &self.read_only)
            .finish_non_exhaustive()
    }
}

impl Default for TextArea {
    fn default() -> Self {
        Self::new("")
    }
}

impl TextArea {
    /// A text area containing `text` (soft wrap on, tab moves focus, `css` theme).
    pub fn new(text: impl AsRef<str>) -> Self {
        let document = Document::new(text.as_ref());
        let wrapped = WrappedDocument::new(&document, 0, 4);
        TextArea {
            document,
            wrapped: RefCell::new(wrapped),
            selection: Selection::default(),
            last_x_offset: 0,
            history: EditHistory::new(50),
            language: None,
            highlighter: None,
            theme: TextAreaTheme::get_builtin_theme("css").expect("css theme"),
            themes: HashMap::new(),
            theme_base_applied: false,
            soft_wrap: true,
            tab_behavior: TabBehavior::Focus,
            indent_type: IndentType::Spaces,
            indent_width: 4,
            read_only: false,
            show_cursor: true,
            show_line_numbers: false,
            line_number_start: 1,
            match_cursor_bracket: true,
            cursor_blink: true,
            highlight_cursor_line: true,
            compact: false,
            placeholder: String::new(),
            cursor_visible: true,
            blink_timer: None,
            selecting: false,
            last_press: None,
            matching_bracket: None,
        }
    }

    /// A text area set up for code: line numbers, no soft wrap, tab indents, `monokai` theme.
    pub fn code_editor(text: impl AsRef<str>, language: Option<&str>) -> Self {
        let mut ta = Self::new(text)
            .show_line_numbers(true)
            .soft_wrap(false)
            .tab_behavior(TabBehavior::Indent)
            .theme("monokai");
        if let Some(l) = language {
            ta = ta.language(l);
        }
        ta
    }

    // ------------------------------------------------------------------ builders

    /// Set the language (enables the built-in highlighter for it, if there is one).
    pub fn language(mut self, language: &str) -> Self {
        self.set_language(Some(language));
        self
    }

    /// Use a custom highlighter (overrides the language's built-in one).
    pub fn highlighter(mut self, highlighter: Rc<dyn Highlighter>) -> Self {
        self.highlighter = Some(highlighter);
        self
    }

    /// Select a theme by name (built-in or registered). Unknown names are ignored.
    pub fn theme(mut self, name: &str) -> Self {
        if let Some(t) = self.lookup_theme(name) {
            self.theme = t;
        }
        self
    }

    pub fn soft_wrap(mut self, soft_wrap: bool) -> Self {
        self.soft_wrap = soft_wrap;
        self
    }

    pub fn tab_behavior(mut self, behavior: TabBehavior) -> Self {
        self.tab_behavior = behavior;
        self
    }

    pub fn indent_type(mut self, indent_type: IndentType) -> Self {
        self.indent_type = indent_type;
        self
    }

    /// Tab stop width (also the number of spaces an indent aligns to). Default 4.
    pub fn indent_width(mut self, width: usize) -> Self {
        self.indent_width = width.max(1);
        self
    }

    pub fn read_only(mut self, read_only: bool) -> Self {
        self.read_only = read_only;
        self
    }

    /// Show the cursor in read-only mode (default true).
    pub fn show_cursor(mut self, show: bool) -> Self {
        self.show_cursor = show;
        self
    }

    pub fn show_line_numbers(mut self, show: bool) -> Self {
        self.show_line_numbers = show;
        self
    }

    pub fn line_number_start(mut self, start: usize) -> Self {
        self.line_number_start = start;
        self
    }

    pub fn max_checkpoints(mut self, max: usize) -> Self {
        self.history.max_checkpoints = max;
        self
    }

    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    pub fn cursor_blink(mut self, blink: bool) -> Self {
        self.cursor_blink = blink;
        self
    }

    pub fn match_cursor_bracket(mut self, on: bool) -> Self {
        self.match_cursor_bracket = on;
        self
    }

    pub fn highlight_cursor_line(mut self, on: bool) -> Self {
        self.highlight_cursor_line = on;
        self
    }

    /// Compact style (no border).
    pub fn compact(mut self, compact: bool) -> Self {
        self.compact = compact;
        self
    }

    // ------------------------------------------------------------------ accessors

    /// The whole text.
    pub fn text(&self) -> String {
        self.document.text()
    }

    pub fn document(&self) -> &Document {
        &self.document
    }

    /// The wrapped view (as of the last render / handler).
    pub fn wrapped_document(&self) -> std::cell::Ref<'_, WrappedDocument> {
        self.wrapped.borrow()
    }

    pub fn history(&self) -> &EditHistory {
        &self.history
    }

    pub fn selection(&self) -> Selection {
        self.selection
    }

    /// The cursor location (the end of the selection).
    pub fn cursor_location(&self) -> Location {
        self.selection.end
    }

    pub fn selected_text(&self) -> String {
        self.document
            .get_text_range(self.selection.start, self.selection.end)
    }

    pub fn get_text_range(&self, start: Location, end: Location) -> String {
        self.document.get_text_range(start, end)
    }

    pub fn get_language(&self) -> Option<&str> {
        self.language.as_deref()
    }

    pub fn theme_name(&self) -> &str {
        &self.theme.name
    }

    pub fn get_theme(&self) -> &TextAreaTheme {
        &self.theme
    }

    pub fn is_soft_wrap(&self) -> bool {
        self.soft_wrap
    }

    pub fn is_read_only(&self) -> bool {
        self.read_only
    }

    pub fn get_tab_behavior(&self) -> TabBehavior {
        self.tab_behavior
    }

    pub fn get_indent_width(&self) -> usize {
        self.indent_width
    }

    pub fn get_placeholder(&self) -> &str {
        &self.placeholder
    }

    pub fn is_showing_line_numbers(&self) -> bool {
        self.show_line_numbers
    }

    /// The location of the bracket matching the one at the cursor, if any.
    pub fn matching_bracket_location(&self) -> Option<Location> {
        self.matching_bracket
    }

    /// Whether the cursor is currently in the visible phase of its blink.
    pub fn cursor_visible(&self) -> bool {
        self.cursor_visible
    }

    pub fn available_themes(&self) -> Vec<String> {
        let mut names: Vec<String> = TextAreaTheme::builtin_names()
            .iter()
            .map(|s| s.to_string())
            .collect();
        names.extend(self.themes.keys().cloned());
        names.sort();
        names.dedup();
        names
    }

    pub fn available_languages(&self) -> Vec<&'static str> {
        BUILTIN_LANGUAGES.to_vec()
    }

    /// Width of the line-number gutter (0 when line numbers are hidden).
    pub fn gutter_width(&self) -> i32 {
        if !self.show_line_numbers {
            return 0;
        }
        let last = self.document.line_count() - 1 + self.line_number_start;
        last.to_string().len() as i32 + 2
    }

    pub fn cursor_at_first_line(&self) -> bool {
        self.selection.end.0 == 0
    }

    pub fn cursor_at_last_line(&self) -> bool {
        self.selection.end.0 + 1 == self.document.line_count()
    }

    pub fn cursor_at_start_of_line(&self) -> bool {
        self.selection.end.1 == 0
    }

    pub fn cursor_at_end_of_line(&self) -> bool {
        self.selection.end.1 == self.document.line_len(self.selection.end.0)
    }

    pub fn cursor_at_start_of_text(&self) -> bool {
        self.selection.end == (0, 0)
    }

    pub fn cursor_at_end_of_text(&self) -> bool {
        self.cursor_at_last_line() && self.cursor_at_end_of_line()
    }

    // ------------------------------------------------------------------ configuration

    /// Change the language (and the built-in highlighter). `None` = plain text.
    pub fn set_language(&mut self, language: Option<&str>) {
        self.language = language.map(String::from);
        self.highlighter = language.and_then(KeywordHighlighter::shared);
    }

    pub fn set_highlighter(&mut self, highlighter: Option<Rc<dyn Highlighter>>) {
        self.highlighter = highlighter;
    }

    /// Make a theme available to [`TextArea::set_theme`] (replacing one of the same name).
    pub fn register_theme(&mut self, theme: TextAreaTheme) {
        self.themes.insert(theme.name.clone(), theme);
    }

    fn lookup_theme(&self, name: &str) -> Option<TextAreaTheme> {
        self.themes
            .get(name)
            .cloned()
            .or_else(|| TextAreaTheme::get_builtin_theme(name))
    }

    /// Switch theme. Returns false if the theme doesn't exist.
    pub fn set_theme(&mut self, name: &str, ctx: &mut Ctx) -> bool {
        match self.lookup_theme(name) {
            Some(t) => {
                self.theme = t;
                self.apply_theme_base(ctx);
                true
            }
            None => false,
        }
    }

    /// Write the theme's base colors to the widget's inline styles (so padding matches),
    /// or clear them for themes without a base style.
    fn apply_theme_base(&mut self, ctx: &mut Ctx) {
        let base = self.theme.base_style.clone();
        match base {
            Some(base) => {
                ctx.set_styles(|s| {
                    if let Some(fg) = base.fg {
                        s.color = Some(textual_css::ColorValue::Color(fg));
                    }
                    if let Some(bg) = base.bg {
                        s.background = Some(bg);
                    }
                });
                self.theme_base_applied = true;
            }
            None if self.theme_base_applied => {
                ctx.set_styles(|s| {
                    s.color = None;
                    s.background = None;
                });
                self.theme_base_applied = false;
            }
            None => {}
        }
    }

    pub fn set_soft_wrap(&mut self, soft_wrap: bool, ctx: &mut Ctx) {
        self.soft_wrap = soft_wrap;
        if soft_wrap {
            let y = ctx.scroll_offset().y;
            ctx.scroll_to(0, y);
        }
        self.scroll_cursor_visible(ctx);
    }

    pub fn set_read_only(&mut self, read_only: bool, ctx: &mut Ctx) {
        self.read_only = read_only;
        ctx.set_class(read_only, "-read-only");
    }

    pub fn set_show_line_numbers(&mut self, show: bool, ctx: &mut Ctx) {
        self.show_line_numbers = show;
        self.scroll_cursor_visible(ctx);
    }

    pub fn set_indent_width(&mut self, width: usize, ctx: &mut Ctx) {
        self.indent_width = width.max(1);
        self.scroll_cursor_visible(ctx);
    }

    pub fn set_tab_behavior(&mut self, behavior: TabBehavior) {
        self.tab_behavior = behavior;
    }

    pub fn set_indent_type(&mut self, indent_type: IndentType) {
        self.indent_type = indent_type;
    }

    pub fn set_placeholder(&mut self, placeholder: impl Into<String>) {
        self.placeholder = placeholder.into();
    }

    pub fn set_cursor_blink(&mut self, blink: bool, ctx: &mut Ctx) {
        self.cursor_blink = blink;
        if blink && ctx.has_focus() {
            self.restart_blink(ctx);
        } else {
            let visible = ctx.has_focus();
            self.pause_blink(visible, ctx);
        }
    }

    pub fn set_match_cursor_bracket(&mut self, on: bool) {
        self.match_cursor_bracket = on;
    }

    pub fn set_highlight_cursor_line(&mut self, on: bool) {
        self.highlight_cursor_line = on;
    }

    pub fn set_max_checkpoints(&mut self, max: usize) {
        self.history.max_checkpoints = max;
    }

    // ------------------------------------------------------------------ text loading

    /// Replace the whole text, clearing the undo history (Textual's `load_text` / `text = ...`).
    pub fn set_text(&mut self, text: &str, ctx: &mut Ctx) {
        self.history.clear();
        self.document = Document::new(text);
        self.wrapped.get_mut().invalidate();
        self.last_x_offset = 0;
        self.move_cursor((0, 0), false, ctx);
        self.post_changed(ctx);
    }

    /// Alias of [`TextArea::set_text`].
    pub fn load_text(&mut self, text: &str, ctx: &mut Ctx) {
        self.set_text(text, ctx);
    }

    // ------------------------------------------------------------------ internals

    fn has_cursor(&self) -> bool {
        !self.read_only || self.show_cursor
    }

    fn wrap_width(&self, content_width: i32) -> i32 {
        if self.soft_wrap {
            (content_width - self.gutter_width() - 1).max(0)
        } else {
            0
        }
    }

    /// Make sure the wrapped document matches the current content width.
    fn sync(&mut self, ctx: &Ctx) {
        let w = self.wrap_width(ctx.size().width);
        let indent = self.indent_width;
        self.wrapped.get_mut().ensure(&self.document, w, indent);
    }

    fn with_nav<R>(&self, f: impl FnOnce(&Navigator) -> R) -> R {
        let wrapped = self.wrapped.borrow();
        let nav = Navigator::new(&self.document, &wrapped, self.last_x_offset);
        f(&nav)
    }

    fn text_area_id(ctx: &Ctx) -> Option<String> {
        ctx.css_id(ctx.id()).map(String::from)
    }

    fn post_changed(&self, ctx: &mut Ctx) {
        let text_area_id = Self::text_area_id(ctx);
        ctx.post_message(Changed { text_area_id });
    }

    /// Set the selection: clamp, track the matching bracket, keep the cursor visible and
    /// post [`SelectionChanged`] if it changed.
    fn update_selection(&mut self, selection: Selection, ctx: &mut Ctx) {
        let selection = Selection::new(
            self.document.clamp(selection.start),
            self.document.clamp(selection.end),
        );
        let previous = self.selection;
        self.selection = selection;
        self.matching_bracket = self.find_matching_bracket_at(selection.end);
        self.scroll_cursor_visible(ctx);
        if previous != selection {
            let text_area_id = Self::text_area_id(ctx);
            ctx.post_message(SelectionChanged {
                selection,
                text_area_id,
            });
        }
    }

    /// Remember the cursor's x offset for vertical movement.
    pub fn record_cursor_width(&mut self) {
        let wrapped = self.wrapped.borrow();
        self.last_x_offset = wrapped
            .location_to_offset(&self.document, self.selection.end)
            .0;
    }

    /// Scroll so the cursor is visible.
    pub fn scroll_cursor_visible(&mut self, ctx: &mut Ctx) {
        if !self.has_cursor() {
            return;
        }
        let size = ctx.size();
        if size.width <= 0 || size.height <= 0 {
            return;
        }
        self.sync(ctx);
        let (x, y) = self
            .wrapped
            .borrow()
            .location_to_offset(&self.document, self.selection.end);
        // The gutter is fixed on the left, so the text viewport is `gutter` cells narrower.
        let gutter = self.gutter_width();
        ctx.scroll_to_show(Region::new(x, y as i32, gutter + 1, 1));
    }

    /// The matching bracket for the bracket at `location`, if any.
    fn find_matching_bracket_at(&self, location: Location) -> Option<Location> {
        if !self.match_cursor_bracket {
            return None;
        }
        let line = self.document.get_line(location.0);
        let c = line.chars().nth(location.1)?;
        self.find_matching_bracket(c, location)
    }

    /// Find the bracket matching `bracket` (located at `from`), scanning the document.
    pub fn find_matching_bracket(&self, bracket: char, from: Location) -> Option<Location> {
        let closing_of = |c: char| OPENING_BRACKETS.iter().find(|p| p.0 == c).map(|p| p.1);
        let opening_of = |c: char| OPENING_BRACKETS.iter().find(|p| p.1 == c).map(|p| p.0);
        let mut stack: Vec<char> = Vec::new();
        let lines = self.document.lines();
        if closing_of(bracket).is_some() {
            let (mut row, mut col) = from;
            while row < lines.len() {
                for (i, c) in lines[row].chars().enumerate().skip(col) {
                    if closing_of(c).is_some() {
                        stack.push(c);
                    } else if let Some(open) = opening_of(c) {
                        if stack.last() == Some(&open) {
                            stack.pop();
                            if stack.is_empty() {
                                return Some((row, i));
                            }
                        }
                    }
                }
                row += 1;
                col = 0;
            }
        } else if opening_of(bracket).is_some() {
            let (row0, col0) = from;
            for row in (0..=row0.min(lines.len() - 1)).rev() {
                let chars: Vec<char> = lines[row].chars().collect();
                let start = if row == row0 {
                    (col0 + 1).min(chars.len())
                } else {
                    chars.len()
                };
                for i in (0..start).rev() {
                    let c = chars[i];
                    if opening_of(c).is_some() {
                        stack.push(c);
                    } else if let Some(close) = closing_of(c) {
                        if stack.last() == Some(&close) {
                            stack.pop();
                            if stack.is_empty() {
                                return Some((row, i));
                            }
                        }
                    }
                }
            }
        }
        None
    }

    // ------------------------------------------------------------------ edits

    /// Perform an [`Edit`] (recorded in the history; posts [`Changed`]).
    pub fn edit(&mut self, mut edit: Edit, ctx: &mut Ctx) -> EditResult {
        self.sync(ctx);
        edit.from_location = self.document.clamp(edit.from_location);
        edit.to_location = self.document.clamp(edit.to_location);
        let old_gutter = self.gutter_width();
        let top = edit.top().0;
        let old_bottom = edit.bottom().0;
        let result = edit.perform(&mut self.document, self.selection, true);
        let new_selection = edit.updated_selection().unwrap_or(self.selection);
        self.history.record(edit);
        self.rewrap(top, old_bottom, result.end_location.0, old_gutter, ctx);
        self.update_selection(new_selection, ctx);
        self.record_cursor_width();
        self.post_changed(ctx);
        result
    }

    fn rewrap(
        &mut self,
        top: usize,
        old_bottom: usize,
        new_bottom: usize,
        old_gutter: i32,
        ctx: &Ctx,
    ) {
        let gutter_changed = old_gutter != self.gutter_width();
        let wrapped = self.wrapped.get_mut();
        if gutter_changed && self.soft_wrap {
            wrapped.invalidate();
        } else {
            wrapped.wrap_range(&self.document, top, old_bottom, new_bottom);
        }
        self.sync(ctx);
    }

    /// Insert `text` at `location` (default: the cursor). The selection keeps its position
    /// relative to the surrounding text.
    pub fn insert(&mut self, text: &str, location: Option<Location>, ctx: &mut Ctx) -> EditResult {
        if text.chars().count() > 1 {
            self.restart_blink(ctx);
        }
        let location = location.unwrap_or(self.selection.end);
        self.edit(Edit::new(text, location, location, true), ctx)
    }

    /// Delete the text between two locations.
    pub fn delete(&mut self, start: Location, end: Location, ctx: &mut Ctx) -> EditResult {
        self.edit(Edit::new("", start, end, true), ctx)
    }

    /// Replace the text between two locations with `text`.
    pub fn replace(
        &mut self,
        text: &str,
        start: Location,
        end: Location,
        ctx: &mut Ctx,
    ) -> EditResult {
        self.edit(Edit::new(text, start, end, true), ctx)
    }

    /// Delete all text.
    pub fn clear(&mut self, ctx: &mut Ctx) -> EditResult {
        let end = self.document.end();
        self.edit(Edit::new("", (0, 0), end, false), ctx)
    }

    fn replace_via_keyboard(
        &mut self,
        text: &str,
        start: Location,
        end: Location,
        ctx: &mut Ctx,
    ) -> Option<EditResult> {
        if self.read_only {
            return None;
        }
        Some(self.edit(Edit::new(text, start, end, false), ctx))
    }

    fn delete_via_keyboard(
        &mut self,
        start: Location,
        end: Location,
        ctx: &mut Ctx,
    ) -> Option<EditResult> {
        self.replace_via_keyboard("", start, end, ctx)
    }

    /// Undo the most recent batch of edits.
    pub fn undo(&mut self, ctx: &mut Ctx) {
        let Some(mut batch) = self.history.pop_undo() else {
            return;
        };
        self.sync(ctx);
        let old_gutter = self.gutter_width();
        for edit in batch.iter_mut().rev() {
            let top = edit.top().0;
            let old_bottom = edit.result().map(|r| r.end_location.0).unwrap_or(top);
            let r = edit.undo(&mut self.document);
            self.wrapped
                .get_mut()
                .wrap_range(&self.document, top, old_bottom, r.end_location.0);
        }
        if old_gutter != self.gutter_width() {
            self.wrapped.get_mut().invalidate();
        }
        self.sync(ctx);
        let selection = batch
            .first()
            .and_then(|e| e.updated_selection())
            .unwrap_or(self.selection);
        self.history.push_redo(batch);
        self.update_selection(selection, ctx);
        self.record_cursor_width();
        self.post_changed(ctx);
    }

    /// Redo the most recently undone batch of edits.
    pub fn redo(&mut self, ctx: &mut Ctx) {
        let Some(mut batch) = self.history.pop_redo() else {
            return;
        };
        self.sync(ctx);
        let old_gutter = self.gutter_width();
        for edit in batch.iter_mut() {
            let top = edit.top().0;
            let old_bottom = edit.bottom().0;
            let r = edit.perform(&mut self.document, self.selection, false);
            self.wrapped
                .get_mut()
                .wrap_range(&self.document, top, old_bottom, r.end_location.0);
        }
        if old_gutter != self.gutter_width() {
            self.wrapped.get_mut().invalidate();
        }
        self.sync(ctx);
        let selection = batch
            .last()
            .and_then(|e| e.updated_selection())
            .unwrap_or(self.selection);
        self.history.push_undo(batch);
        self.update_selection(selection, ctx);
        self.record_cursor_width();
        self.post_changed(ctx);
    }

    // ------------------------------------------------------------------ cursor & selection

    /// Set the selection (clamped to the document).
    pub fn set_selection(&mut self, selection: Selection, ctx: &mut Ctx) {
        self.sync(ctx);
        self.update_selection(selection, ctx);
        self.record_cursor_width();
    }

    /// Move the cursor; with `select`, extend the selection from its anchor.
    pub fn move_cursor(&mut self, location: Location, select: bool, ctx: &mut Ctx) {
        self.move_cursor_with(location, select, true, ctx);
    }

    fn move_cursor_with(
        &mut self,
        location: Location,
        select: bool,
        record_width: bool,
        ctx: &mut Ctx,
    ) {
        if !self.has_cursor() {
            return;
        }
        self.sync(ctx);
        let selection = if select {
            Selection::new(self.selection.start, location)
        } else {
            Selection::cursor(location)
        };
        self.update_selection(selection, ctx);
        if record_width {
            self.record_cursor_width();
        }
        self.history.checkpoint();
    }

    /// Move the cursor relative to its location (in document space).
    pub fn move_cursor_relative(
        &mut self,
        rows: isize,
        columns: isize,
        select: bool,
        ctx: &mut Ctx,
    ) {
        let (row, col) = self.selection.end;
        let row = (row as isize + rows).max(0) as usize;
        let col = (col as isize + columns).max(0) as usize;
        let target = self.document.clamp((row, col));
        self.move_cursor(target, select, ctx);
    }

    /// Select a whole line (without its newline).
    pub fn select_line(&mut self, index: usize, ctx: &mut Ctx) {
        if index >= self.document.line_count() {
            return;
        }
        let len = self.document.line_len(index);
        self.set_selection(Selection::new((index, 0), (index, len)), ctx);
    }

    /// Select the whole document.
    pub fn select_all(&mut self, ctx: &mut Ctx) {
        let end = self.document.end();
        self.set_selection(Selection::new((0, 0), end), ctx);
    }

    // ------------------------------------------------------------------ blink

    fn restart_blink(&mut self, ctx: &mut Ctx) {
        self.cursor_visible = true;
        if let Some(t) = self.blink_timer.take() {
            ctx.cancel_timer(t);
        }
        if self.cursor_blink && ctx.has_focus() {
            self.blink_timer = Some(ctx.set_interval(BLINK_INTERVAL, |w: &mut TextArea, _ctx| {
                w.cursor_visible = !w.cursor_visible;
            }));
        }
    }

    fn pause_blink(&mut self, visible: bool, ctx: &mut Ctx) {
        self.cursor_visible = visible;
        if let Some(t) = self.blink_timer.take() {
            ctx.cancel_timer(t);
        }
    }

    // ------------------------------------------------------------------ actions

    fn target_or(
        &self,
        select: bool,
        collapsed: Location,
        f: impl FnOnce(&Navigator) -> Location,
    ) -> Location {
        if select || self.selection.is_empty() {
            self.with_nav(|n| f(n))
        } else {
            collapsed
        }
    }

    pub fn action_cursor_left(&mut self, select: bool, ctx: &mut Ctx) {
        if !self.has_cursor() {
            ctx.scroll_by(-1, 0);
            return;
        }
        let (min, _) = self.selection.sorted();
        let cursor = self.selection.end;
        let target = self.target_or(select, min, |n| n.get_location_left(cursor));
        self.move_cursor(target, select, ctx);
    }

    pub fn action_cursor_right(&mut self, select: bool, ctx: &mut Ctx) {
        if !self.has_cursor() {
            ctx.scroll_by(1, 0);
            return;
        }
        let (_, max) = self.selection.sorted();
        let cursor = self.selection.end;
        let target = self.target_or(select, max, |n| n.get_location_right(cursor));
        self.move_cursor(target, select, ctx);
    }

    pub fn action_cursor_up(&mut self, select: bool, ctx: &mut Ctx) {
        if !self.has_cursor() {
            ctx.scroll_by(0, -1);
            return;
        }
        let cursor = self.selection.end;
        let target = self.with_nav(|n| n.get_location_above(cursor));
        self.move_cursor_with(target, select, false, ctx);
    }

    pub fn action_cursor_down(&mut self, select: bool, ctx: &mut Ctx) {
        if !self.has_cursor() {
            ctx.scroll_by(0, 1);
            return;
        }
        let cursor = self.selection.end;
        let target = self.with_nav(|n| n.get_location_below(cursor));
        self.move_cursor_with(target, select, false, ctx);
    }

    pub fn action_cursor_line_start(&mut self, select: bool, ctx: &mut Ctx) {
        if !self.has_cursor() {
            let y = ctx.scroll_offset().y;
            ctx.scroll_to(0, y);
            return;
        }
        let cursor = self.selection.end;
        let target = self.with_nav(|n| n.get_location_home(cursor, true));
        self.move_cursor(target, select, ctx);
    }

    pub fn action_cursor_line_end(&mut self, select: bool, ctx: &mut Ctx) {
        if !self.has_cursor() {
            let y = ctx.scroll_offset().y;
            ctx.scroll_to(i32::MAX / 2, y);
            return;
        }
        let cursor = self.selection.end;
        let target = self.with_nav(|n| n.get_location_end(cursor));
        self.move_cursor(target, select, ctx);
    }

    pub fn action_cursor_word_left(&mut self, select: bool, ctx: &mut Ctx) {
        if !self.show_cursor || self.cursor_at_start_of_text() {
            return;
        }
        let cursor = self.selection.end;
        let target = self.with_nav(|n| n.get_location_word_left(cursor));
        self.move_cursor(target, select, ctx);
    }

    pub fn action_cursor_word_right(&mut self, select: bool, ctx: &mut Ctx) {
        if !self.show_cursor || self.cursor_at_end_of_text() {
            return;
        }
        let cursor = self.selection.end;
        let target = self.with_nav(|n| n.get_location_word_right(cursor));
        self.move_cursor(target, select, ctx);
    }

    pub fn action_cursor_document_start(&mut self, select: bool, ctx: &mut Ctx) {
        if !self.has_cursor() {
            let x = ctx.scroll_offset().x;
            ctx.scroll_to(x, 0);
            return;
        }
        self.move_cursor((0, 0), select, ctx);
    }

    pub fn action_cursor_document_end(&mut self, select: bool, ctx: &mut Ctx) {
        if !self.has_cursor() {
            let x = ctx.scroll_offset().x;
            ctx.scroll_to(x, i32::MAX / 2);
            return;
        }
        let end = self.document.end();
        self.move_cursor(end, select, ctx);
    }

    fn page(&mut self, direction: i32, ctx: &mut Ctx) {
        let height = ctx.size().height.max(1);
        if !self.show_cursor {
            ctx.scroll_by(0, direction * height);
            return;
        }
        let cursor = self.selection.end;
        let target = self.with_nav(|n| n.get_location_at_y_offset(cursor, direction * height));
        ctx.scroll_by(0, direction * height);
        self.move_cursor(target, false, ctx);
    }

    pub fn action_cursor_page_up(&mut self, ctx: &mut Ctx) {
        self.page(-1, ctx);
    }

    pub fn action_cursor_page_down(&mut self, ctx: &mut Ctx) {
        self.page(1, ctx);
    }

    pub fn action_select_line(&mut self, ctx: &mut Ctx) {
        let row = self.selection.end.0;
        self.select_line(row, ctx);
    }

    pub fn action_delete_left(&mut self, ctx: &mut Ctx) {
        if self.read_only {
            return;
        }
        let Selection { start, mut end } = self.selection;
        if self.selection.is_empty() {
            end = self.with_nav(|n| n.get_location_left(start));
        }
        self.delete_via_keyboard(start, end, ctx);
    }

    pub fn action_delete_right(&mut self, ctx: &mut Ctx) {
        if self.read_only {
            return;
        }
        let Selection { start, mut end } = self.selection;
        if self.selection.is_empty() {
            end = self.with_nav(|n| n.get_location_right(start));
        }
        self.delete_via_keyboard(start, end, ctx);
    }

    pub fn action_delete_word_left(&mut self, ctx: &mut Ctx) {
        if self.read_only || self.cursor_at_start_of_text() {
            return;
        }
        let Selection { start, end } = self.selection;
        if start != end {
            self.delete_via_keyboard(start, end, ctx);
            return;
        }
        let to = self.with_nav(|n| n.get_location_word_left(end));
        self.delete_via_keyboard(end, to, ctx);
    }

    pub fn action_delete_word_right(&mut self, ctx: &mut Ctx) {
        if self.read_only || self.cursor_at_end_of_text() {
            return;
        }
        let Selection { start, end } = self.selection;
        if start != end {
            self.delete_via_keyboard(start, end, ctx);
            return;
        }
        let to = self.with_nav(|n| n.get_delete_word_right_target(end));
        self.delete_via_keyboard(end, to, ctx);
    }

    /// Delete the lines the selection touches (including the newline).
    pub fn action_delete_line(&mut self, ctx: &mut Ctx) {
        if self.read_only {
            return;
        }
        self.delete_cursor_line(ctx);
    }

    fn delete_cursor_line(&mut self, ctx: &mut Ctx) -> Option<EditResult> {
        let (start, end) = self.selection.sorted();
        let (start_row, _) = start;
        let (mut end_row, end_column) = end;
        // Don't delete the last selected line if the selection ends at its start.
        if start_row != end_row && end_column == 0 {
            end_row -= 1;
        }
        let from = (start_row, 0);
        let to = if end_row + 1 < self.document.line_count() {
            (end_row + 1, 0)
        } else if start_row > 0 {
            // Deleting the last line: also remove the newline before it.
            let from_prev = (start_row - 1, self.document.line_len(start_row - 1));
            let r = self.delete_via_keyboard(from_prev, self.document.end(), ctx)?;
            let target = self.document.clamp((r.end_location.0, end_column));
            self.move_cursor_with(target, false, false, ctx);
            return Some(r);
        } else {
            self.document.end()
        };
        let r = self.delete_via_keyboard(from, to, ctx)?;
        self.move_cursor_relative(0, end_column as isize, false, ctx);
        Some(r)
    }

    pub fn action_cut(&mut self, ctx: &mut Ctx) {
        if self.read_only {
            return;
        }
        let Selection { start, end } = self.selection;
        let result = if start == end {
            self.delete_cursor_line(ctx)
        } else {
            self.delete_via_keyboard(start, end, ctx)
        };
        if let Some(r) = result {
            // Local clipboard + OSC 52 system clipboard (Textual: `app.copy_to_clipboard`).
            ctx.copy_to_clipboard(&r.replaced_text);
        }
    }

    /// Copy the selection. Returns false (letting the key fall through) if nothing is selected.
    pub fn action_copy(&mut self) -> bool {
        let text = self.selected_text();
        if text.is_empty() {
            return false;
        }
        set_clipboard_text(&text);
        true
    }

    pub fn action_paste(&mut self, ctx: &mut Ctx) {
        if self.read_only {
            return;
        }
        let text = crate::terminal::paste_text(ctx.app);
        let Selection { start, end } = self.selection;
        if let Some(r) = self.replace_via_keyboard(&text, start, end, ctx) {
            self.move_cursor(r.end_location, false, ctx);
        }
    }

    pub fn action_delete_to_start_of_line(&mut self, ctx: &mut Ctx) {
        if self.read_only {
            return;
        }
        if self.cursor_at_start_of_line() {
            self.action_delete_left(ctx);
        } else {
            let from = self.selection.end;
            let to = self.with_nav(|n| n.get_location_home(from, false));
            self.delete_via_keyboard(from, to, ctx);
        }
    }

    pub fn action_delete_to_end_of_line(&mut self, ctx: &mut Ctx) {
        if self.read_only {
            return;
        }
        let from = self.selection.end;
        let to = self.with_nav(|n| n.get_location_end(from));
        self.delete_via_keyboard(from, to, ctx);
    }

    /// Delete to the end of the line; on an empty line delete the line, at the end of a line
    /// join the next one.
    pub fn action_delete_to_end_of_line_or_delete_line(&mut self, ctx: &mut Ctx) {
        let cursor = self.selection.end;
        let (home, end) = self.with_nav(|n| {
            (
                n.get_location_home(cursor, false),
                n.get_location_end(cursor),
            )
        });
        if home == end {
            self.action_delete_line(ctx);
        } else if self.selection.is_empty() && cursor == end {
            self.action_delete_right(ctx);
        } else {
            self.action_delete_to_end_of_line(ctx);
        }
    }

    /// The text to insert for the tab key.
    fn indent_text(&self) -> String {
        match self.indent_type {
            IndentType::Tabs => "\t".to_string(),
            IndentType::Spaces => {
                let cursor = self.selection.end;
                let w = self.indent_width as i32;
                if self.document.get_line(cursor.0).is_empty() {
                    return " ".repeat(self.indent_width);
                }
                let before = self.with_nav(|n| n.column_width(cursor));
                " ".repeat((w - (w + before) % w) as usize)
            }
        }
    }

    // ------------------------------------------------------------------ mouse

    fn mouse_location(&self, m: &MouseEvent, ctx: &Ctx) -> Location {
        let region = ctx.content_region();
        let scroll = ctx.scroll_offset();
        let x = m.screen_x - region.x - self.gutter_width() + scroll.x;
        let y = m.screen_y - region.y + scroll.y;
        self.wrapped
            .borrow()
            .offset_to_location(&self.document, x, y)
    }

    fn end_mouse_selection(&mut self, ctx: &mut Ctx) {
        if self.selecting {
            self.selecting = false;
            self.record_cursor_width();
            self.restart_blink(ctx);
        }
    }

    // ------------------------------------------------------------------ rendering

    fn resolve_styles(&self, ctx: &RenderCtx) -> Resolved {
        let base = ctx.base_style();
        // Component styles inherit the widget's color; keep only what the class sets.
        let comp = |class: &str| {
            let mut s = ctx.component_style(class);
            if s.fg == base.fg {
                s.fg = None;
            }
            if s.bg == base.bg {
                s.bg = None;
            }
            s
        };
        let t = &self.theme;
        let cursor = t.cursor_style.clone().unwrap_or_else(|| {
            let s = comp("text-area--cursor");
            if s.bg.is_none() && s.fg.is_none() {
                let bg = base.bg.unwrap_or(ctx.background());
                Style::new().fg(bg).bg(bg.inverse())
            } else {
                s
            }
        });
        Resolved {
            cursor,
            gutter: t
                .gutter_style
                .clone()
                .unwrap_or_else(|| comp("text-area--gutter")),
            cursor_gutter: t
                .cursor_line_gutter_style
                .clone()
                .unwrap_or_else(|| comp("text-area--cursor-gutter")),
            cursor_line: t
                .cursor_line_style
                .clone()
                .unwrap_or_else(|| comp("text-area--cursor-line")),
            selection: t
                .selection_style
                .clone()
                .unwrap_or_else(|| comp("text-area--selection")),
            bracket: t
                .bracket_matching_style
                .clone()
                .unwrap_or_else(|| comp("text-area--matching-bracket")),
            placeholder: comp("text-area--placeholder"),
        }
    }

    fn draw_cursor(&self, focused: bool) -> bool {
        if self.read_only {
            return self.show_cursor && focused;
        }
        focused && (!self.cursor_blink || self.cursor_visible)
    }

    fn render_placeholder(&self, ctx: &RenderCtx, st: &Resolved, draw_cursor: bool) -> Content {
        let width = ctx.size.width;
        let mut lines = Text::plain(self.placeholder.as_str()).wrap(
            width,
            Justify::Left,
            TextOverflow::Fold,
            false,
        );
        lines.truncate(ctx.size.height.max(0) as usize);
        let out = lines
            .into_iter()
            .enumerate()
            .map(|(i, strip)| {
                let strip = strip.apply_style(&st.placeholder);
                if i == 0 && draw_cursor {
                    let mut first = strip.crop(0, 1).apply_style(&st.cursor);
                    if first.cell_len() == 0 {
                        first = Strip::from_str(" ", st.cursor.clone());
                    }
                    first.spans.extend(strip.crop(1, width).spans);
                    first
                } else {
                    strip
                }
            })
            .collect();
        Content::Lines(out)
    }

    #[allow(clippy::too_many_arguments)]
    fn render_line(
        &self,
        wrapped: &WrappedDocument,
        st: &Resolved,
        line_index: usize,
        section: usize,
        scroll_x: i32,
        width: i32,
        draw_cursor: bool,
    ) -> Strip {
        let line = self.document.get_line(line_index);
        let chars: Vec<char> = line.chars().collect();
        let cells = line_cells(line, self.indent_width);
        let n = chars.len();
        let selection = self.selection;
        let (cursor_row, cursor_col) = selection.end;
        let has_cursor = self.has_cursor();
        let on_cursor_row = has_cursor && cursor_row == line_index;
        let highlight_line = self.highlight_cursor_line && on_cursor_row;
        let line_style = if highlight_line {
            st.cursor_line.clone()
        } else {
            Style::default()
        };
        let mut styles: Vec<Style> = vec![line_style.clone(); n + 1];
        let mut empty_selected_line = false;

        // Selection.
        let (top, bottom) = selection.sorted();
        if !selection.is_empty() && top.0 <= line_index && line_index <= bottom.0 {
            if n == 0 && line_index != cursor_row {
                empty_selected_line = true;
            } else {
                let (a, b) = if top.0 == bottom.0 {
                    (top.1, bottom.1)
                } else if line_index == top.0 {
                    (top.1, n)
                } else if line_index == bottom.0 {
                    (0, bottom.1)
                } else {
                    (0, n)
                };
                for s in &mut styles[a.min(n)..b.min(n)] {
                    *s = s.combine(&st.selection);
                }
            }
        }

        // Syntax highlighting.
        if let Some(h) = &self.highlighter {
            for hl in h.highlight_line(line_index, line) {
                if let Some(style) = self.theme.get_highlight(&hl.name) {
                    for s in &mut styles[hl.start.min(n)..hl.end.min(n)] {
                        *s = s.combine(style);
                    }
                }
            }
        }

        // Matching brackets and the cursor.
        let draw_brackets = has_cursor && self.match_cursor_bracket && selection.is_empty();
        if let (true, Some(m)) = (draw_brackets, self.matching_bracket) {
            if on_cursor_row && cursor_col < n {
                styles[cursor_col] = styles[cursor_col].combine(&st.bracket);
            }
            if m.0 == line_index && m.1 < n {
                styles[m.1] = styles[m.1].combine(&st.bracket);
            }
        }
        if on_cursor_row && draw_cursor && cursor_col <= n {
            styles[cursor_col] = styles[cursor_col].combine(&st.cursor);
        }

        // Build the visible section.
        let sections = wrapped.section_count(line_index);
        let (start, end) = wrapped.section_range(line_index, section, n);
        let mut spans: Vec<Span> = Vec::new();
        let mut push = |text: &str, style: &Style| {
            if let Some(last) = spans.last_mut() {
                if last.style == *style {
                    last.text.push_str(text);
                    return;
                }
            }
            spans.push(Span::new(text, style.clone()));
        };
        if empty_selected_line {
            let marker = Style {
                fg: st.selection.bg,
                ..Style::default()
            };
            push("▌", &line_style.combine(&marker));
        } else {
            let mut buf = String::new();
            for i in start..end.min(n) {
                buf.clear();
                display_char(chars[i], cells[i], &mut buf);
                push(&buf, &styles[i]);
            }
            if section + 1 >= sections {
                // A cell for the cursor to rest at the end of the line.
                push(" ", &styles[n]);
            }
        }
        let gutter = self.gutter_width();
        let text_width = (width - gutter).max(0);
        let mut text = Strip::new(spans);
        if !self.soft_wrap {
            text = text.crop(scroll_x, scroll_x + text_width);
        }
        let text = text.set_cell_len(text_width, &line_style);
        if gutter == 0 {
            return text;
        }
        let label = if section == 0 {
            format!(
                "{:>w$}  ",
                line_index + self.line_number_start,
                w = (gutter - 2) as usize
            )
        } else {
            " ".repeat(gutter as usize)
        };
        let gutter_style = if highlight_line {
            &st.cursor_gutter
        } else {
            &st.gutter
        };
        let mut strip = Strip::from_str(&label, gutter_style.clone());
        strip.spans.extend(text.spans);
        strip
    }

    /// Height of the document wrapped at `wrap_width` (without caching).
    fn height_at(&self, wrap_width: i32) -> usize {
        let w = WrappedDocument::new(&self.document, wrap_width, self.indent_width);
        w.height()
    }
}

impl Widget for TextArea {
    fn type_names(&self) -> &'static [&'static str] {
        &["TextArea", "ScrollView"]
    }

    fn default_css(&self) -> &'static str {
        DEFAULT_CSS
    }

    fn can_focus(&self) -> bool {
        true
    }

    fn on_mount(&mut self, ctx: &mut Ctx) {
        ctx.set_class(self.read_only, "-read-only");
        ctx.set_class(self.compact, "-textual-compact");
        self.apply_theme_base(ctx);
        self.matching_bracket = self.find_matching_bracket_at(self.selection.end);
    }

    fn on_unmount(&mut self, ctx: &mut Ctx) {
        if let Some(t) = self.blink_timer.take() {
            ctx.cancel_timer(t);
        }
    }

    fn bindings(&self) -> Vec<Binding> {
        let b = |keys: &str, action: &str, desc: &str| Binding::new(keys, action, desc).hidden();
        vec![
            b("up", "cursor_up", "Cursor up"),
            b("down", "cursor_down", "Cursor down"),
            b("left", "cursor_left", "Cursor left"),
            b("right", "cursor_right", "Cursor right"),
            b("ctrl+left", "cursor_word_left", "Cursor word left"),
            b("ctrl+right", "cursor_word_right", "Cursor word right"),
            b("home,ctrl+a", "cursor_line_start", "Cursor line start"),
            b("end,ctrl+e", "cursor_line_end", "Cursor line end"),
            b("pageup", "cursor_page_up", "Cursor page up"),
            b("pagedown", "cursor_page_down", "Cursor page down"),
            b(
                "ctrl+home",
                "cursor_document_start",
                "Cursor document start",
            ),
            b("ctrl+end", "cursor_document_end", "Cursor document end"),
            b(
                "ctrl+shift+left",
                "cursor_word_left(True)",
                "Cursor left word select",
            ),
            b(
                "ctrl+shift+right",
                "cursor_word_right(True)",
                "Cursor right word select",
            ),
            b(
                "shift+home",
                "cursor_line_start(True)",
                "Cursor line start select",
            ),
            b(
                "shift+end",
                "cursor_line_end(True)",
                "Cursor line end select",
            ),
            b("shift+up", "cursor_up(True)", "Cursor up select"),
            b("shift+down", "cursor_down(True)", "Cursor down select"),
            b("shift+left", "cursor_left(True)", "Cursor left select"),
            b("shift+right", "cursor_right(True)", "Cursor right select"),
            b(
                "ctrl+shift+home",
                "cursor_document_start(True)",
                "Select to document start",
            ),
            b(
                "ctrl+shift+end",
                "cursor_document_end(True)",
                "Select to document end",
            ),
            b("f6", "select_line", "Select line"),
            b("f7", "select_all", "Select all"),
            b("backspace", "delete_left", "Delete character left"),
            b(
                "ctrl+w,ctrl+backspace,alt+backspace",
                "delete_word_left",
                "Delete left to start of word",
            ),
            b("delete,ctrl+d", "delete_right", "Delete character right"),
            b(
                "alt+delete,ctrl+f",
                "delete_word_right",
                "Delete right to start of word",
            ),
            b("ctrl+x,super+x", "cut", "Cut"),
            b("ctrl+c,super+c", "copy", "Copy"),
            b("ctrl+v", "paste", "Paste"),
            b(
                "ctrl+u,super+backspace",
                "delete_to_start_of_line",
                "Delete to line start",
            ),
            b(
                "ctrl+k",
                "delete_to_end_of_line_or_delete_line",
                "Delete to line end",
            ),
            b("ctrl+shift+k", "delete_line", "Delete line"),
            b("ctrl+z,super+z", "undo", "Undo"),
            b("ctrl+y,super+y", "redo", "Redo"),
        ]
    }

    fn run_action(&mut self, action: &str, args: &[String], ctx: &mut Ctx) -> bool {
        let select = args
            .first()
            .map(|a| a.eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        self.sync(ctx);
        match action {
            "cursor_up" => self.action_cursor_up(select, ctx),
            "cursor_down" => self.action_cursor_down(select, ctx),
            "cursor_left" => self.action_cursor_left(select, ctx),
            "cursor_right" => self.action_cursor_right(select, ctx),
            "cursor_word_left" => self.action_cursor_word_left(select, ctx),
            "cursor_word_right" => self.action_cursor_word_right(select, ctx),
            "cursor_line_start" => self.action_cursor_line_start(select, ctx),
            "cursor_line_end" => self.action_cursor_line_end(select, ctx),
            "cursor_page_up" => self.action_cursor_page_up(ctx),
            "cursor_page_down" => self.action_cursor_page_down(ctx),
            "cursor_document_start" => self.action_cursor_document_start(select, ctx),
            "cursor_document_end" => self.action_cursor_document_end(select, ctx),
            "select_line" => self.action_select_line(ctx),
            "select_all" => self.select_all(ctx),
            "delete_left" => self.action_delete_left(ctx),
            "delete_word_left" => self.action_delete_word_left(ctx),
            "delete_right" => self.action_delete_right(ctx),
            "delete_word_right" => self.action_delete_word_right(ctx),
            "cut" => self.action_cut(ctx),
            // Like Textual's SkipAction: let the key fall through when nothing is selected.
            "copy" => {
                let copied = self.action_copy();
                if copied {
                    ctx.copy_to_clipboard(&self.selected_text());
                }
                return copied;
            }
            "paste" => self.action_paste(ctx),
            "delete_to_start_of_line" => self.action_delete_to_start_of_line(ctx),
            "delete_to_end_of_line" => self.action_delete_to_end_of_line(ctx),
            "delete_to_end_of_line_or_delete_line" => {
                self.action_delete_to_end_of_line_or_delete_line(ctx)
            }
            "delete_line" => self.action_delete_line(ctx),
            "undo" => self.undo(ctx),
            "redo" => self.redo(ctx),
            _ => return false,
        }
        true
    }

    fn on_event(&mut self, event: &Event, ctx: &mut Ctx) -> Flow {
        self.sync(ctx);
        match event {
            Event::Key(k) => {
                self.restart_blink(ctx);
                if self.read_only {
                    return Flow::Continue;
                }
                let indent = self.tab_behavior == TabBehavior::Indent;
                if indent && k.key == "escape" {
                    ctx.focus_next();
                    return Flow::Stop;
                }
                let insert = if k.key == "enter" {
                    Some("\n".to_string())
                } else if indent && k.key == "tab" {
                    Some(self.indent_text())
                } else {
                    k.character.filter(|c| !c.is_control()).map(String::from)
                };
                if let Some(text) = insert {
                    let Selection { start, end } = self.selection;
                    self.replace_via_keyboard(&text, start, end, ctx);
                    return Flow::Stop;
                }
                Flow::Continue
            }
            Event::Paste(text) => {
                if !self.read_only {
                    let Selection { start, end } = self.selection;
                    if let Some(r) = self.replace_via_keyboard(text, start, end, ctx) {
                        self.move_cursor(r.end_location, false, ctx);
                    }
                }
                Flow::Stop
            }
            Event::Focus => {
                self.restart_blink(ctx);
                self.history.checkpoint();
                Flow::Continue
            }
            Event::Blur => {
                self.pause_blink(false, ctx);
                self.selecting = false;
                Flow::Continue
            }
            Event::MouseDown(m) if m.button == Some(MouseButton::Left) => {
                let location = self.mouse_location(m, ctx);
                let now = Instant::now();
                let count = match self.last_press {
                    Some((t, loc, c))
                        if loc == location
                            && now.saturating_duration_since(t) <= MULTI_CLICK_INTERVAL =>
                    {
                        c % 3 + 1
                    }
                    _ => 1,
                };
                self.last_press = Some((now, location, count));
                match count {
                    2 => {
                        let (a, b) = self.with_nav(|n| n.word_range_at(location));
                        self.set_selection(Selection::new((location.0, a), (location.0, b)), ctx);
                    }
                    3 => self.select_line(location.0, ctx),
                    _ => {
                        self.update_selection(Selection::cursor(location), ctx);
                        self.selecting = true;
                        self.pause_blink(false, ctx);
                    }
                }
                self.history.checkpoint();
                Flow::Stop
            }
            Event::MouseMove(m) if self.selecting => {
                if m.button != Some(MouseButton::Left) {
                    // The button was released outside the widget.
                    self.end_mouse_selection(ctx);
                    return Flow::Continue;
                }
                let location = self.mouse_location(m, ctx);
                let start = self.selection.start;
                self.update_selection(Selection::new(start, location), ctx);
                Flow::Stop
            }
            Event::MouseUp(_) => {
                self.end_mouse_selection(ctx);
                Flow::Continue
            }
            _ => Flow::Continue,
        }
    }

    fn virtual_size(&self, ctx: &RenderCtx) -> Option<Size> {
        let container = ctx.size;
        let indent = self.indent_width;
        let mut wrapped = self.wrapped.borrow_mut();
        if !self.soft_wrap {
            wrapped.ensure(&self.document, 0, indent);
            return Some(Size::new(
                wrapped.max_width() + self.gutter_width() + 1,
                self.document.line_count() as i32,
            ));
        }
        // The wrap width depends on whether a vertical scrollbar is shown, which depends on
        // the wrapped height: decide like the layout will.
        let st = ctx.styles();
        let needs_v = match st.get_overflow_y() {
            Overflow::Scroll => true,
            Overflow::Hidden => false,
            Overflow::Auto => {
                self.document.line_count() as i32 > container.height
                    || self.height_at(self.wrap_width(container.width)) as i32 > container.height
            }
        };
        let width = if needs_v {
            container.width - st.get_scrollbar_size_vertical()
        } else {
            container.width
        };
        wrapped.ensure(&self.document, self.wrap_width(width), indent);
        Some(Size::new(0, wrapped.height() as i32))
    }

    fn content_width(&self, _ctx: &RenderCtx) -> Option<i32> {
        let mut wrapped = self.wrapped.borrow_mut();
        wrapped.ensure(&self.document, 0, self.indent_width);
        Some(wrapped.max_width() + self.gutter_width() + 1)
    }

    fn content_height(&self, _ctx: &RenderCtx, width: i32) -> Option<i32> {
        let mut wrapped = self.wrapped.borrow_mut();
        wrapped.ensure(&self.document, self.wrap_width(width), self.indent_width);
        Some(wrapped.height() as i32)
    }

    fn render(&self, ctx: &RenderCtx) -> Content {
        let Size { width, height } = ctx.size;
        if width <= 0 || height <= 0 {
            return Content::Empty;
        }
        let st = self.resolve_styles(ctx);
        let draw_cursor = self.draw_cursor(ctx.is_focused());
        if self.document.is_empty() && !self.placeholder.is_empty() {
            return self.render_placeholder(ctx, &st, draw_cursor);
        }
        let mut wrapped = self.wrapped.borrow_mut();
        wrapped.ensure(&self.document, self.wrap_width(width), self.indent_width);
        let scroll = ctx.scroll_offset;
        let mut out = Vec::with_capacity(height as usize);
        for vy in 0..height {
            let y = (scroll.y + vy).max(0) as usize;
            let Some((line_index, section)) = wrapped.line_info(y) else {
                break;
            };
            out.push(self.render_line(
                &wrapped,
                &st,
                line_index,
                section,
                scroll.x,
                width,
                draw_cursor,
            ));
        }
        Content::Lines(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matching_brackets() {
        let ta = TextArea::new("fn f(a: [u8]) {\n    (x)\n}");
        assert_eq!(ta.find_matching_bracket('(', (0, 4)), Some((0, 12)));
        assert_eq!(ta.find_matching_bracket('[', (0, 8)), Some((0, 11)));
        assert_eq!(ta.find_matching_bracket('{', (0, 14)), Some((2, 0)));
        assert_eq!(ta.find_matching_bracket('}', (2, 0)), Some((0, 14)));
        assert_eq!(ta.find_matching_bracket(')', (1, 6)), Some((1, 4)));
        assert_eq!(ta.find_matching_bracket('x', (1, 5)), None);
        let unbalanced = TextArea::new("((");
        assert_eq!(unbalanced.find_matching_bracket('(', (0, 0)), None);
    }

    #[test]
    fn gutter_width_tracks_line_count() {
        let ta = TextArea::new("a\nb").show_line_numbers(true);
        assert_eq!(ta.gutter_width(), 3);
        let ta = TextArea::new("x\n".repeat(10)).show_line_numbers(true);
        assert_eq!(ta.gutter_width(), 4);
        assert_eq!(TextArea::new("x").gutter_width(), 0);
    }

    #[test]
    fn code_editor_defaults() {
        let ta = TextArea::code_editor("x = 1", Some("python"));
        assert!(!ta.is_soft_wrap());
        assert!(ta.is_showing_line_numbers());
        assert_eq!(ta.get_tab_behavior(), TabBehavior::Indent);
        assert_eq!(ta.theme_name(), "monokai");
        assert_eq!(ta.get_language(), Some("python"));
    }
}

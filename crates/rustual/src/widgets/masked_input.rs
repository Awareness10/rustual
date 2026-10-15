//! `MaskedInput` (port of `textual/widgets/_masked_input.py`): an [`Input`] whose value must
//! follow a template mask.
//!
//! Template characters:
//!
//! | Char | Accepts | Required |
//! |---|---|---|
//! | `A` / `a` | letter | yes / no |
//! | `N` / `n` | letter or digit | yes / no |
//! | `X` / `x` | any non-space character | yes / no |
//! | `9` / `0` | digit | yes / no |
//! | `D` / `d` | digit 1-9 | yes / no |
//! | `#` | digit, `+` or `-` | no |
//! | `H` / `h` | hex digit | yes / no |
//! | `B` / `b` | `0` or `1` | yes / no |
//!
//! `>` upper-cases the following characters, `<` lower-cases them, `!` stops case conversion,
//! `\` escapes the next character, and `;c` at the end sets the blank character shown for
//! unfilled positions (default space). Any other character is a literal separator, which the
//! cursor skips and which is inserted automatically while typing.
//!
//! ```
//! # use rustual::prelude::*;
//! let card = MaskedInput::new("9999-9999-9999-9999;0").id("card"); // credit card, blanks shown as 0
//! let code = MaskedInput::new(">AAA-999"); // upper-cased letters
//! ```
//!
//! The inner `Input` posts `input::Changed` / `input::Submitted` / `input::Blurred`.
//! The widget gets the `-valid` / `-invalid` class after each change, on blur and on submit.
//!
//! Deviations from Textual: no `validators` argument (only the template is validated);
//! typing or deleting with a selection first collapses / blanks the selection.

use crate::binding::Binding;
use crate::context::Ctx;
use crate::events::Event;
use crate::message::Flow;
use crate::widget::{Content, RenderCtx, Widget};
use crate::widgets::input::{self, Input, Selection};
use rustual_render::cells::cell_len;
use rustual_render::{Span, Strip, Style};
use std::any::Any;
use std::cell::Cell;

/// What a template position accepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pattern {
    Alpha,
    Alnum,
    NotSpace,
    Digit,
    Digit19,
    DigitSign,
    Hex,
    Binary,
    Literal(char),
}

impl Pattern {
    fn matches(self, c: char) -> bool {
        match self {
            Pattern::Alpha => c.is_ascii_alphabetic(),
            Pattern::Alnum => c.is_ascii_alphanumeric(),
            Pattern::NotSpace => c != ' ',
            Pattern::Digit => c.is_ascii_digit(),
            Pattern::Digit19 => ('1'..='9').contains(&c),
            Pattern::DigitSign => c.is_ascii_digit() || c == '+' || c == '-',
            Pattern::Hex => c.is_ascii_hexdigit(),
            Pattern::Binary => c == '0' || c == '1',
            Pattern::Literal(l) => c == l,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum Case {
    #[default]
    None,
    Upper,
    Lower,
}

/// One position of the template (`_Template.CharDefinition`).
#[derive(Debug, Clone, PartialEq)]
struct CharDef {
    pattern: Pattern,
    required: bool,
    separator: bool,
    case: Case,
    /// Separator character, or the placeholder / blank shown for an unfilled position.
    ch: char,
}

/// Error for a template without any input position.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateError(pub String);

impl std::fmt::Display for TemplateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for TemplateError {}

/// The parsed template mask (`_Template`). Values are handled as `Vec<char>`.
#[derive(Debug, Clone, PartialEq)]
struct Template {
    defs: Vec<CharDef>,
    blank: char,
}

fn set_char(value: &mut Vec<char>, pos: usize, ch: char) {
    while value.len() < pos {
        value.push(' ');
    }
    if pos < value.len() {
        value[pos] = ch;
    } else {
        value.push(ch);
    }
}

impl Template {
    fn parse(template: &str, placeholder: &str) -> Result<Template, TemplateError> {
        let mut defs = Vec::new();
        let mut blank = ' ';
        let mut escaped = false;
        let mut case = Case::None;
        let mut chars = template.chars();
        while let Some(c) = chars.next() {
            let mut def = if escaped {
                escaped = false;
                CharDef {
                    pattern: Pattern::Literal(c),
                    required: false,
                    separator: true,
                    case: Case::None,
                    ch: c,
                }
            } else {
                match c {
                    '\\' => {
                        escaped = true;
                        continue;
                    }
                    ';' => {
                        if let Some(b) = chars.next() {
                            blank = b;
                        }
                        break;
                    }
                    '>' => {
                        case = Case::Upper;
                        continue;
                    }
                    '<' => {
                        case = Case::Lower;
                        continue;
                    }
                    '!' => {
                        case = Case::None;
                        continue;
                    }
                    _ => {}
                }
                let (pattern, required) = match c {
                    'A' => (Pattern::Alpha, true),
                    'a' => (Pattern::Alpha, false),
                    'N' => (Pattern::Alnum, true),
                    'n' => (Pattern::Alnum, false),
                    'X' => (Pattern::NotSpace, true),
                    'x' => (Pattern::NotSpace, false),
                    '9' => (Pattern::Digit, true),
                    '0' => (Pattern::Digit, false),
                    'D' => (Pattern::Digit19, true),
                    'd' => (Pattern::Digit19, false),
                    '#' => (Pattern::DigitSign, false),
                    'H' => (Pattern::Hex, true),
                    'h' => (Pattern::Hex, false),
                    'B' => (Pattern::Binary, true),
                    'b' => (Pattern::Binary, false),
                    _ => (Pattern::Literal(c), false),
                };
                let separator = matches!(pattern, Pattern::Literal(_));
                CharDef {
                    pattern,
                    required,
                    separator,
                    case: Case::None,
                    ch: c,
                }
            };
            def.case = case;
            defs.push(def);
        }
        if defs.iter().all(|d| d.separator) {
            return Err(TemplateError(
                "Template must contain at least one non-separator character".into(),
            ));
        }
        let mut t = Template { defs, blank };
        t.update_mask(placeholder);
        Ok(t)
    }

    fn len(&self) -> usize {
        self.defs.len()
    }

    fn is_separator(&self, pos: usize) -> bool {
        self.defs.get(pos).is_some_and(|d| d.separator)
    }

    fn update_mask(&mut self, placeholder: &str) {
        let placeholder: Vec<char> = placeholder.chars().collect();
        let blank = self.blank;
        for (i, d) in self.defs.iter_mut().enumerate() {
            if !d.separator {
                d.ch = placeholder.get(i).copied().unwrap_or(blank);
            }
        }
    }

    /// The placeholder mask (separators and blank / placeholder characters).
    fn mask(&self) -> Vec<char> {
        self.defs.iter().map(|d| d.ch).collect()
    }

    /// The mask with every non-separator replaced by a space.
    fn empty_mask(&self) -> Vec<char> {
        self.defs
            .iter()
            .map(|d| if d.separator { d.ch } else { ' ' })
            .collect()
    }

    /// Whether every required position present in `value` matches.
    fn check(&self, value: &[char], allow_space: bool) -> bool {
        value
            .iter()
            .zip(&self.defs)
            .all(|(c, d)| !d.required || d.pattern.matches(*c) || (*c == ' ' && allow_space))
    }

    /// Full validation: every required position must be filled.
    fn validate(&self, value: &[char]) -> bool {
        let mut padded = value.to_vec();
        padded.resize(padded.len().max(self.len()), '\0');
        self.check(&padded, false)
    }

    fn insert_separators(&self, mut value: Vec<char>, mut cursor: usize) -> (Vec<char>, usize) {
        while cursor < self.len() && self.defs[cursor].separator {
            set_char(&mut value, cursor, self.defs[cursor].ch);
            cursor += 1;
        }
        (value, cursor)
    }

    fn prev_separator_position(&self, position: usize) -> Option<usize> {
        // Like Textual's `range(position - 1, 0, -1)`: position 0 is never returned.
        (1..position).rev().find(|i| self.is_separator(*i))
    }

    fn next_separator_position(&self, position: usize) -> Option<usize> {
        (position + 1..self.len()).find(|i| self.is_separator(*i))
    }

    fn next_separator(&self, position: usize) -> Option<char> {
        self.next_separator_position(position)
            .map(|p| self.defs[p].ch)
    }

    /// Insert `text` at `cursor`; `None` if a character doesn't match the template.
    fn insert_text(&self, value: &[char], cursor: usize, text: &str) -> Option<(Vec<char>, usize)> {
        let mut value = value.to_vec();
        let mut cursor = cursor;
        for c in text.chars() {
            let is_sep_char = self.defs.iter().any(|d| d.separator && d.ch == c);
            if is_sep_char {
                if Some(c) == self.next_separator(cursor) {
                    let prev = self.prev_separator_position(cursor);
                    if cursor > 0 && prev != Some(cursor - 1) {
                        if let Some(next) = self.next_separator_position(cursor) {
                            while cursor < next + 1 {
                                let ch = if self.defs[cursor].separator {
                                    self.defs[cursor].ch
                                } else {
                                    ' '
                                };
                                set_char(&mut value, cursor, ch);
                                cursor += 1;
                            }
                        }
                    }
                }
                continue;
            }
            if cursor >= self.len() {
                break;
            }
            let def = &self.defs[cursor];
            if !def.pattern.matches(c) {
                return None;
            }
            let c = match def.case {
                Case::Lower => c.to_ascii_lowercase(),
                Case::Upper => c.to_ascii_uppercase(),
                Case::None => c,
            };
            set_char(&mut value, cursor, c);
            cursor += 1;
            (value, cursor) = self.insert_separators(value, cursor);
        }
        Some((value, cursor))
    }

    /// Cursor position after moving by `delta`, skipping separators.
    fn move_cursor(&self, cursor: usize, delta: i64) -> usize {
        if delta < 0 && (0..cursor.min(self.len())).all(|i| self.defs[i].separator) {
            return cursor;
        }
        let mut pos = cursor as i64 + delta;
        while pos >= 0 && (pos as usize) < self.len() && self.defs[pos as usize].separator {
            pos += delta;
        }
        pos.max(0) as usize
    }

    /// Blank the character at `position` and trim trailing blanks / separators.
    fn delete_at(&self, value: &[char], position: usize) -> (Vec<char>, usize) {
        let mut value = value.to_vec();
        let mut cursor = position;
        if cursor < self.len() && cursor < value.len() {
            if cursor + 1 == value.len() {
                value.truncate(cursor);
            } else {
                value[cursor] = ' ';
            }
        }
        let mut pos = value.len();
        while pos > 0 {
            if !self.defs[pos - 1].separator && value[pos - 1] != ' ' {
                break;
            }
            pos -= 1;
        }
        value.truncate(pos);
        cursor = cursor.min(value.len());
        self.insert_separators(value, cursor)
    }

    /// `value` with blanks replaced by the mask characters.
    fn display(&self, value: &[char]) -> Vec<char> {
        value
            .iter()
            .zip(&self.defs)
            .map(|(c, d)| if *c == ' ' { d.ch } else { *c })
            .collect()
    }
}

/// A text input that follows a template mask.
#[derive(Debug, Clone)]
pub struct MaskedInput {
    input: Input,
    template_str: String,
    template: Template,
    valid_empty: bool,
    valid: bool,
    scroll_x: Cell<i32>,
}

fn masked_access(w: &mut dyn Any) -> Option<&mut Input> {
    w.downcast_mut::<MaskedInput>().map(|m| &mut m.input)
}

impl MaskedInput {
    /// A masked input for `template`. Panics if the template has no input positions
    /// (use [`MaskedInput::try_new`] to handle that).
    pub fn new(template: &str) -> Self {
        Self::try_new(template).expect("invalid MaskedInput template")
    }

    pub fn try_new(template: &str) -> Result<Self, TemplateError> {
        let t = Template::parse(template, "")?;
        let (value, cursor) = t.insert_separators(Vec::new(), 0);
        let mut input = Input::new().value(value.iter().collect::<String>());
        input.set_cursor_position(cursor);
        input.access = masked_access;
        Ok(MaskedInput {
            input,
            template_str: template.to_string(),
            template: t,
            valid_empty: false,
            valid: true,
            scroll_x: Cell::new(0),
        })
    }

    /// Initial value. Separators at the start are inserted; a value that doesn't match the
    /// template is ignored (Textual raises an error).
    pub fn value(mut self, value: &str) -> Self {
        let (v, _) = self.template.insert_separators(value.chars().collect(), 0);
        if self.template.check(&v, true) {
            let v: String = v.into_iter().take(self.template.len()).collect();
            self.input = self.input.value(v);
        }
        self
    }

    /// Placeholder characters shown in unfilled positions (position by position; the
    /// template's blank character is used past its end).
    pub fn placeholder(mut self, placeholder: &str) -> Self {
        self.template.update_mask(placeholder);
        self.input = self.input.placeholder(placeholder);
        self
    }

    /// Treat an empty value as valid.
    pub fn valid_empty(mut self, valid_empty: bool) -> Self {
        self.valid_empty = valid_empty;
        self
    }

    /// Select all text when focused (default true).
    pub fn select_on_focus(mut self, select: bool) -> Self {
        self.input = self.input.select_on_focus(select);
        self
    }

    /// Whether the cursor blinks while focused (default true).
    pub fn cursor_blink(mut self, blink: bool) -> Self {
        self.input = self.input.cursor_blink(blink);
        self
    }

    /// Compact style (no border, height 1).
    pub fn compact(mut self, compact: bool) -> Self {
        self.input = self.input.compact(compact);
        self
    }

    // ------------------------------------------------------------ accessors

    /// The inner input (value, cursor, selection).
    pub fn input(&self) -> &Input {
        &self.input
    }

    pub fn get_value(&self) -> &str {
        self.input.get_value()
    }

    pub fn template(&self) -> &str {
        &self.template_str
    }

    pub fn cursor_position(&self) -> usize {
        self.input.cursor_position()
    }

    /// The placeholder mask, e.g. `"    -    "` for `"9999-9999"`.
    pub fn mask(&self) -> String {
        self.template.mask().into_iter().collect()
    }

    /// Whether the value matched the template at the last validation.
    pub fn is_valid(&self) -> bool {
        self.valid
    }

    /// Whether `value` fully matches the template (all required positions filled).
    pub fn matches_template(&self, value: &str) -> bool {
        let v: Vec<char> = value.chars().collect();
        v.len() <= self.template.len() && self.template.validate(&v)
    }

    // ------------------------------------------------------------ mutation

    fn chars(&self) -> Vec<char> {
        self.input.get_value().chars().collect()
    }

    fn set(&mut self, value: Vec<char>, cursor: usize, ctx: &mut Ctx) {
        let value: String = value.into_iter().collect();
        self.input.set_value(value, ctx);
        self.input.set_cursor_position(cursor);
        self.validate(ctx);
    }

    fn set_cursor(&mut self, cursor: usize) {
        self.input.set_cursor_position(cursor);
    }

    /// Set the value (posts `input::Changed`). Returns false (and changes nothing) if it doesn't
    /// match the template.
    pub fn set_value(&mut self, value: &str, ctx: &mut Ctx) -> bool {
        let v: Vec<char> = value.chars().collect();
        if !self.template.check(&v, true) {
            return false;
        }
        let v: Vec<char> = v.into_iter().take(self.template.len()).collect();
        let cursor = v.len();
        self.set(v, cursor, ctx);
        true
    }

    /// Change the template (re-validates the value).
    pub fn set_template(&mut self, template: &str, ctx: &mut Ctx) -> Result<(), TemplateError> {
        self.template = Template::parse(template, self.input.get_placeholder())?;
        self.template_str = template.to_string();
        self.validate(ctx);
        Ok(())
    }

    pub fn set_placeholder(&mut self, placeholder: &str) {
        self.template.update_mask(placeholder);
        self.input.set_placeholder(placeholder);
    }

    /// Clear the value (separators at the start are kept).
    pub fn clear(&mut self, ctx: &mut Ctx) {
        let (v, c) = self.template.insert_separators(Vec::new(), 0);
        self.set(v, c, ctx);
    }

    /// Validate against the template, setting the `-valid` / `-invalid` classes.
    pub fn validate(&mut self, ctx: &mut Ctx) -> bool {
        let v = self.chars();
        self.valid = (self.valid_empty && v.is_empty()) || self.template.validate(&v);
        ctx.set_class(!self.valid, "-invalid");
        ctx.set_class(self.valid, "-valid");
        self.valid
    }

    /// Collapse a selection to its start (returns the cursor).
    fn collapse_selection(&mut self) -> usize {
        let (a, _) = self.input.selection().sorted();
        if !self.input.selection().is_empty() {
            self.set_cursor(a);
        }
        self.input.cursor_position()
    }

    /// Insert text at the cursor following the template (rings the bell if it doesn't match).
    pub fn insert_text_at_cursor(&mut self, text: &str, ctx: &mut Ctx) {
        let cursor = self.collapse_selection();
        match self.template.insert_text(&self.chars(), cursor, text) {
            Some((v, c)) => self.set(v, c, ctx),
            None => ctx.bell(),
        }
    }

    fn delete_at_cursor(&mut self, ctx: &mut Ctx) {
        let cursor = self.input.cursor_position();
        let (v, c) = self.template.delete_at(&self.chars(), cursor);
        self.set(v, c, ctx);
    }

    /// Blank every non-separator position in `start..end`; the cursor ends at `start`.
    fn delete_range(&mut self, start: usize, end: usize, ctx: &mut Ctx) {
        for index in start..end {
            if index >= self.chars().len() {
                break;
            }
            self.set_cursor(index);
            if !self.template.is_separator(index) {
                self.delete_at_cursor(ctx);
            }
        }
        self.set_cursor(start);
    }

    /// If there is a selection, blank it and return true.
    fn delete_selection(&mut self, ctx: &mut Ctx) -> bool {
        let sel = self.input.selection();
        if sel.is_empty() {
            return false;
        }
        let (a, b) = sel.sorted();
        self.delete_range(a, b, ctx);
        true
    }

    // ------------------------------------------------------------ actions

    pub fn action_cursor_left(&mut self) {
        let c = self.collapse_selection();
        let c = self.template.move_cursor(c, -1);
        self.set_cursor(c);
    }

    pub fn action_cursor_right(&mut self) {
        let c = self.input.selection().sorted().1;
        let c = self.template.move_cursor(c, 1);
        self.set_cursor(c);
    }

    pub fn action_home(&mut self) {
        let c = self.input.cursor_position();
        let c = self.template.move_cursor(c, -(self.template.len() as i64));
        self.set_cursor(c);
    }

    pub fn action_cursor_left_word(&mut self) {
        let c = self.collapse_selection();
        let position = if c > 0 && self.template.is_separator(c - 1) {
            self.template.prev_separator_position(c - 1)
        } else {
            self.template.prev_separator_position(c)
        };
        self.set_cursor(position.map(|p| p + 1).unwrap_or(0));
    }

    pub fn action_cursor_right_word(&mut self) {
        let c = self.input.selection().sorted().1;
        match self.template.next_separator_position(c) {
            None => self.set_cursor(self.template.len()),
            Some(p) => self.set_cursor(p + 1),
        }
    }

    pub fn action_delete_right(&mut self, ctx: &mut Ctx) {
        if !self.delete_selection(ctx) {
            self.delete_at_cursor(ctx);
        }
    }

    pub fn action_delete_right_word(&mut self, ctx: &mut Ctx) {
        if self.delete_selection(ctx) {
            return;
        }
        let c = self.input.cursor_position();
        let end = match self.template.next_separator_position(c) {
            Some(p) => p + 1,
            None => self.chars().len(),
        };
        self.delete_range(c, end, ctx);
        self.set_cursor(c.min(self.chars().len()));
    }

    pub fn action_delete_left(&mut self, ctx: &mut Ctx) {
        if self.delete_selection(ctx) {
            return;
        }
        let c = self.input.cursor_position();
        if c == 0 {
            return;
        }
        let c = self.template.move_cursor(c, -1);
        self.set_cursor(c);
        self.delete_at_cursor(ctx);
    }

    pub fn action_delete_left_word(&mut self, ctx: &mut Ctx) {
        if self.delete_selection(ctx) {
            return;
        }
        let c = self.input.cursor_position();
        if c == 0 {
            return;
        }
        let position = if self.template.is_separator(c - 1) {
            self.template.prev_separator_position(c - 1)
        } else {
            self.template.prev_separator_position(c)
        };
        let start = position.map(|p| p + 1).unwrap_or(0);
        self.delete_range(start, c, ctx);
        self.set_cursor(start.min(self.chars().len()));
    }

    pub fn action_delete_left_all(&mut self, ctx: &mut Ctx) {
        if self.delete_selection(ctx) {
            return;
        }
        let c = self.input.cursor_position();
        if c == 0 {
            return;
        }
        let value = self.chars();
        let v = if c >= value.len() {
            Vec::new()
        } else {
            let mut v = self.template.empty_mask()[..c].to_vec();
            v.extend_from_slice(&value[c..]);
            v
        };
        self.set(v, 0, ctx);
    }

    pub fn action_cut(&mut self, ctx: &mut Ctx) {
        let text = self.input.selected_text().to_string();
        Input::set_clipboard(&text);
        self.delete_selection(ctx);
    }

    pub fn action_paste(&mut self, ctx: &mut Ctx) {
        let text = Input::clipboard();
        if !text.is_empty() {
            self.delete_selection(ctx);
            self.insert_text_at_cursor(&text, ctx);
        }
    }

    /// After a mouse press, don't leave the cursor on a separator.
    fn skip_separator(&mut self) {
        let c = self.input.cursor_position();
        if self.input.selection().is_empty() && self.template.is_separator(c) {
            let c = self.template.move_cursor(c, 1);
            self.set_cursor(c);
        }
    }

    /// Cell index → character index for a click (one cell per char for ASCII masks).
    fn cell_to_index(&self, x: i32) -> usize {
        let x = x + self.scroll_x.get();
        let display = self.display_chars();
        let mut cell = 0;
        for (i, ch) in display.iter().enumerate() {
            let w = cell_len(&ch.to_string());
            if x < cell + w {
                return i;
            }
            cell += w;
        }
        display.len()
    }

    /// Value (blanks shown as mask characters) followed by the rest of the mask.
    fn display_chars(&self) -> Vec<char> {
        let value = self.chars();
        let mut out = self.template.display(&value);
        let mask = self.template.mask();
        if value.len() < mask.len() {
            out.extend_from_slice(&mask[value.len()..]);
        }
        out
    }
}

impl Widget for MaskedInput {
    fn type_names(&self) -> &'static [&'static str] {
        &["MaskedInput", "Input"]
    }

    fn default_css(&self) -> &'static str {
        Widget::default_css(&self.input)
    }

    fn can_focus(&self) -> bool {
        true
    }

    fn on_mount(&mut self, ctx: &mut Ctx) {
        self.input.on_mount(ctx);
    }

    fn on_unmount(&mut self, ctx: &mut Ctx) {
        self.input.on_unmount(ctx);
    }

    fn bindings(&self) -> Vec<Binding> {
        self.input.bindings()
    }

    fn run_action(&mut self, action: &str, args: &[String], ctx: &mut Ctx) -> bool {
        match action {
            "cursor_left" => self.action_cursor_left(),
            "cursor_right" => self.action_cursor_right(),
            "home" => self.action_home(),
            "cursor_left_word" => self.action_cursor_left_word(),
            "cursor_right_word" => self.action_cursor_right_word(),
            "delete_right" => self.action_delete_right(ctx),
            "delete_right_word" => self.action_delete_right_word(ctx),
            "delete_left" => self.action_delete_left(ctx),
            "delete_left_word" => self.action_delete_left_word(ctx),
            "delete_left_all" => self.action_delete_left_all(ctx),
            "cut" => self.action_cut(ctx),
            "paste" => self.action_paste(ctx),
            "end" => {
                let len = self.chars().len();
                self.set_cursor(len);
            }
            "submit" => {
                self.input.action_submit(ctx);
                self.validate(ctx);
            }
            "delete_right_all" => {
                self.input.run_action(action, args, ctx);
                self.validate(ctx);
            }
            _ => return self.input.run_action(action, args, ctx),
        }
        true
    }

    fn on_event(&mut self, event: &Event, ctx: &mut Ctx) -> Flow {
        match event {
            Event::Key(k) => {
                if let Some(c) = k.character.filter(|c| !c.is_control()) {
                    self.input.restart_blink(ctx);
                    self.insert_text_at_cursor(&c.to_string(), ctx);
                    return Flow::Stop;
                }
                self.input.on_event(event, ctx)
            }
            Event::Paste(text) => {
                if let Some(line) = text.lines().next().filter(|l| !l.is_empty()) {
                    self.delete_selection(ctx);
                    self.insert_text_at_cursor(line, ctx);
                }
                Flow::Stop
            }
            Event::MouseDown(m) | Event::MouseMove(m) => {
                let flow = self.input.on_event(event, ctx);
                // The inner input maps cells to its own (unmasked) value; remap to the mask.
                let x = m.screen_x - ctx.content_region().x;
                let idx = self.cell_to_index(x).min(self.chars().len());
                let sel = self.input.selection();
                if matches!(event, Event::MouseDown(_)) {
                    self.set_cursor(idx);
                    self.skip_separator();
                } else if flow == Flow::Stop {
                    self.input.set_selection(Selection::new(sel.start, idx));
                }
                flow
            }
            Event::Blur => {
                let flow = self.input.on_event(event, ctx);
                self.validate(ctx);
                flow
            }
            _ => self.input.on_event(event, ctx),
        }
    }

    fn content_width(&self, _ctx: &RenderCtx) -> Option<i32> {
        let cells: i32 = self
            .display_chars()
            .iter()
            .map(|c| cell_len(&c.to_string()))
            .sum();
        Some(cells + 1)
    }

    fn content_height(&self, _ctx: &RenderCtx, _width: i32) -> Option<i32> {
        Some(1)
    }

    fn render(&self, ctx: &RenderCtx) -> Content {
        let width = ctx.size.width.max(0);
        let focused = ctx.is_focused();
        let show_cursor = focused && self.input.cursor_visible();
        let cursor_style = ctx.component_style("input--cursor");
        let placeholder_style = ctx.component_style("input--placeholder");
        let selection_style = ctx.component_style("input--selection");
        let value = self.chars();
        let display = self.display_chars();
        let cursor = self.input.cursor_position();
        let (sel_a, sel_b) = self.input.selection().sorted();

        let mut strip = Strip::default();
        let mut cell = 0;
        let mut cursor_cell = 0;
        for (i, ch) in display.iter().enumerate() {
            let blank = value.get(i).is_none_or(|c| *c == ' ');
            let mut style = if blank {
                placeholder_style.clone()
            } else {
                Style::default()
            };
            if focused && i >= sel_a && i < sel_b {
                style = style.combine(&selection_style);
            }
            if i == cursor {
                cursor_cell = cell;
                if show_cursor {
                    style = style.combine(&cursor_style);
                }
            }
            cell += cell_len(&ch.to_string());
            strip.push(Span::new(ch.to_string(), style));
        }
        if cursor >= display.len() {
            cursor_cell = cell;
            if show_cursor {
                strip.push(Span::new(" ", cursor_style));
            }
        }
        let total = cell + 1;
        let mut scroll = self.scroll_x.get();
        if cursor_cell < scroll {
            scroll = cursor_cell;
        } else if width > 0 && cursor_cell >= scroll + width {
            scroll = cursor_cell - width + 1;
        }
        scroll = scroll.clamp(0, (total - width).max(0));
        self.scroll_x.set(scroll);
        let strip = strip.simplify().crop(scroll, scroll + width);
        Content::Lines(vec![strip.set_cell_len(width, &Style::default())])
    }
}

/// Re-exported message types posted by the inner input.
pub use input::{Blurred, Changed, Submitted};

#[cfg(test)]
mod tests {
    use super::*;

    fn t(s: &str) -> Template {
        Template::parse(s, "").unwrap()
    }

    fn chars(s: &str) -> Vec<char> {
        s.chars().collect()
    }

    fn s(v: &[char]) -> String {
        v.iter().collect()
    }

    #[test]
    fn parse_and_mask() {
        let tpl = t("9999-9999;0");
        assert_eq!(s(&tpl.mask()), "0000-0000");
        assert_eq!(s(&tpl.empty_mask()), "    -    ");
        assert!(Template::parse("--", "").is_err());
        let esc = t(r"\A99");
        assert!(esc.is_separator(0));
        assert_eq!(s(&esc.mask()), "A  ");
    }

    #[test]
    fn insert_skips_separators_and_cases() {
        let tpl = t(">AA-99");
        let (v, c) = tpl.insert_text(&[], 0, "ab").unwrap();
        assert_eq!((s(&v).as_str(), c), ("AB-", 3));
        let (v, c) = tpl.insert_text(&v, c, "12").unwrap();
        assert_eq!((s(&v).as_str(), c), ("AB-12", 5));
        assert!(tpl.insert_text(&[], 0, "1").is_none());
        // Typing a separator early jumps to the next field.
        let tpl = t("99-99");
        let (v, c) = tpl.insert_text(&[], 0, "1-").unwrap();
        assert_eq!((s(&v).as_str(), c), ("1 -", 3));
    }

    #[test]
    fn delete_and_validate() {
        let tpl = t("99-99");
        let (v, c) = tpl.delete_at(&chars("12-34"), 4);
        assert_eq!((s(&v).as_str(), c), ("12-3", 4));
        let (v, c) = tpl.delete_at(&chars("12-34"), 0);
        assert_eq!((s(&v).as_str(), c), (" 2-34", 0));
        assert!(tpl.validate(&chars("12-34")));
        assert!(!tpl.validate(&chars("12-3")));
        assert!(!tpl.validate(&chars(" 2-34")));
        assert!(tpl.check(&chars(" 2-34"), true));
        assert_eq!(tpl.move_cursor(1, 1), 3);
        assert_eq!(tpl.move_cursor(3, -1), 1);
    }
}

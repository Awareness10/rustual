//! `Input`: a single-line text input (port of `textual/widgets/_input.py`).
//!
//! Supports a placeholder, password masking, a (blinking) cursor, selection (shift+arrows,
//! mouse drag, select-on-focus), word movement / deletion, paste, `max_length`, character
//! restriction, the `integer` / `number` input types and validation. The value scrolls
//! horizontally to keep the cursor visible.
//!
//! Validators ([`crate::validation`]) run when the value changes, on blur and on submit (see
//! [`Input::validate_on`]); the input gets the `-valid` / `-invalid` class and the
//! [`ValidationResult`] travels with [`Changed`], [`Blurred`] and [`Submitted`]:
//!
//! ```
//! # use rustual::prelude::*;
//! use rustual::validation::{Length, Number};
//!
//! let age = Input::new()
//!     .placeholder("Age")
//!     .validator(Number::new().minimum(0).maximum(150))
//!     .id("age");
//! let name = Input::new()
//!     .validator(Length::new().minimum(1).failure_description("A name is required."))
//!     .validate_on([ValidateOn::Submitted]);
//! ```
//!
//! Without validators, the `integer` / `number` types validate with [`Integer`] / [`Number`]
//! (Textual adds them when no validators are given).

use crate::app::TimerId;
use crate::binding::Binding;
use crate::context::Ctx;
use crate::events::Event;
use crate::message::Flow;
use crate::validation::{Integer, Number, ValidationResult, Validator};
use crate::widget::{Content, RenderCtx, Widget};
use rustual_render::cells::cell_len;
use rustual_render::{Span, Strip, Style};
use std::any::Any;
use std::cell::Cell;
use std::fmt;
use std::rc::Rc;
use std::sync::Mutex;
use std::time::Duration;

const DEFAULT_CSS: &str = r#"
Input {
    background: $surface;
    color: $foreground;
    padding: 0 2;
    border: tall $border-blurred;
    width: 100%;
    height: 3;
    scrollbar-size-horizontal: 0;
    pointer: text;

    &.-textual-compact {
        border: none !important;
        height: 1;
        padding: 0;
        &.-invalid {
            background-tint: $error 20%;
        }
    }

    &:focus {
        border: tall $border;
        background-tint: $foreground 5%;
    }
    &>.input--cursor {
        background: $input-cursor-background;
        color: $input-cursor-foreground;
        text-style: $input-cursor-text-style;
    }
    &>.input--selection {
        background: $input-selection-background;
        color: $input-selection-foreground;
    }
    &>.input--placeholder, &>.input--suggestion {
        color: $text-disabled;
    }
    &.-invalid {
        border: tall $error 60%;
    }
    &.-invalid:focus {
        border: tall $error;
    }
}

/* ANSI themes (Textual `:ansi`). */
Input {
    &:ansi {
        background: ansi_default;
        color: ansi_default;
        &>.input--cursor {
            background: ansi_white;
            color: ansi_black;
        }
        &>.input--placeholder, &>.input--suggestion {
            text-style: dim;
            color: ansi_default;
        }
        &.-invalid {
            border: tall ansi_red;
        }
        &.-invalid:focus {
            border: tall ansi_red;
        }
    }
}
"#;

/// Cursor blink interval (Textual: 0.5s).
const BLINK_INTERVAL: Duration = Duration::from_millis(500);

/// Shared local clipboard for cut / copy / paste (Textual uses `app.clipboard`).
/// Shared with `TextArea`.
static CLIPBOARD: Mutex<String> = Mutex::new(String::new());

/// The local clipboard's contents (shared by `Input` and `TextArea`).
pub(crate) fn clipboard_text() -> String {
    CLIPBOARD.lock().map(|c| c.clone()).unwrap_or_default()
}

/// Replace the local clipboard's contents (shared by `Input` and `TextArea`).
pub(crate) fn set_clipboard_text(text: &str) {
    if let Ok(mut c) = CLIPBOARD.lock() {
        *c = text.to_string();
    }
}

/// Posted when the value changes (Textual: `Input.Changed`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Changed {
    /// The new value.
    pub value: String,
    /// The CSS id of the input, if any.
    pub input_id: Option<String>,
    /// The combined result of the validators, or `None` if validation didn't run (no
    /// validators, an empty value with `valid_empty`, or `changed` not in `validate_on`).
    pub validation_result: Option<ValidationResult>,
}

/// Posted when enter is pressed (Textual: `Input.Submitted`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Submitted {
    pub value: String,
    pub input_id: Option<String>,
    /// The validation result on submission (see [`Changed::validation_result`]).
    pub validation_result: Option<ValidationResult>,
}

/// Posted when the input loses focus (Textual: `Input.Blurred`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Blurred {
    pub value: String,
    pub input_id: Option<String>,
    /// The validation result on blur (see [`Changed::validation_result`]).
    pub validation_result: Option<ValidationResult>,
}

/// When an [`Input`] validates its value (Textual: `InputValidationOn`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ValidateOn {
    /// When the input loses focus.
    Blur,
    /// When the value changes.
    Changed,
    /// When the value is submitted (enter).
    Submitted,
}

/// The kind of value accepted (Textual's `type` argument).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InputType {
    #[default]
    Text,
    /// Restricts edits to (partial) integers; validates with [`Integer`] when there are no
    /// validators.
    Integer,
    /// Restricts edits to (partial) numbers; validates with [`Number`] when there are no
    /// validators.
    Number,
}

/// A range of selected text (character indices). `end` is the cursor position.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Selection {
    pub start: usize,
    pub end: usize,
}

impl Selection {
    pub fn cursor(pos: usize) -> Self {
        Selection {
            start: pos,
            end: pos,
        }
    }

    pub fn new(start: usize, end: usize) -> Self {
        Selection { start, end }
    }

    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }

    /// `(min, max)`.
    pub fn sorted(&self) -> (usize, usize) {
        (self.start.min(self.end), self.start.max(self.end))
    }
}

type Restrict = Rc<dyn Fn(char) -> bool>;
type SharedValidator = Rc<dyn Validator>;
type RestrictValue = Rc<dyn Fn(&str) -> bool>;

/// A single-line text input.
#[derive(Clone)]
pub struct Input {
    value: String,
    placeholder: String,
    password: bool,
    max_length: usize,
    restrict: Option<Restrict>,
    restrict_value: Option<RestrictValue>,
    input_type: InputType,
    selection: Selection,
    select_on_focus: bool,
    compact: bool,
    cursor_blink: bool,
    cursor_visible: bool,
    blink_timer: Option<TimerId>,
    validators: Vec<SharedValidator>,
    /// Validated on blur / changed / submitted.
    validate_on: [bool; 3],
    valid_empty: bool,
    /// Always runs first, even without validators (`MaskedInput`'s template).
    pub(crate) template_validator: Option<SharedValidator>,
    valid: bool,
    /// Mouse is pressed inside the input (drag-select).
    selecting: bool,
    /// The cursor was just placed by a mouse press; don't select-all on the following focus.
    mouse_placed_cursor: bool,
    /// Horizontal scroll (cells), updated while rendering to keep the cursor visible.
    scroll_x: Cell<i32>,
    /// Finds this `Input` inside the widget that owns it (for the blink timer), so widgets
    /// that wrap an `Input` (`MaskedInput`) keep a blinking cursor.
    pub(crate) access: InputAccess,
}

/// Extracts the `Input` from the widget running a timer callback.
pub(crate) type InputAccess = fn(&mut dyn Any) -> Option<&mut Input>;

fn self_access(w: &mut dyn Any) -> Option<&mut Input> {
    w.downcast_mut::<Input>()
}

impl fmt::Debug for Input {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Input")
            .field("value", &self.value)
            .field("placeholder", &self.placeholder)
            .field("password", &self.password)
            .field("max_length", &self.max_length)
            .field("input_type", &self.input_type)
            .field("selection", &self.selection)
            .finish_non_exhaustive()
    }
}

impl Default for Input {
    fn default() -> Self {
        Self::new()
    }
}

fn char_len(s: &str) -> usize {
    s.chars().count()
}

fn byte_index(s: &str, char_idx: usize) -> usize {
    s.char_indices()
        .nth(char_idx)
        .map(|(b, _)| b)
        .unwrap_or(s.len())
}

fn slice_chars(s: &str, start: usize, end: usize) -> &str {
    &s[byte_index(s, start)..byte_index(s, end)]
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Indices matching Textual's `(?<=\W)\w` (a word char preceded by a non-word char) in `chars`.
fn word_starts(chars: &[char]) -> impl Iterator<Item = usize> + '_ {
    (1..chars.len()).filter(|&i| is_word_char(chars[i]) && !is_word_char(chars[i - 1]))
}

/// `(?:\d*|\d+_)*`: digits and underscores, every underscore directly after a digit.
fn is_digit_run(s: &str) -> bool {
    let b = s.as_bytes();
    b.iter()
        .enumerate()
        .all(|(i, c)| c.is_ascii_digit() || (*c == b'_' && i > 0 && b[i - 1].is_ascii_digit()))
}

fn strip_sign(s: &str) -> &str {
    s.strip_prefix(['-', '+']).unwrap_or(s)
}

/// Full match of Textual's `integer` restrict pattern.
fn integer_partial_ok(s: &str) -> bool {
    s.is_ascii() && is_digit_run(strip_sign(s))
}

/// Full match of Textual's `number` restrict pattern:
/// `[-+]?(?:\d*|\d+_)*\.?(?:\d*|\d+_)*(?:\d[eE]?[-+]?(?:\d*|\d+_)*)?`.
fn number_partial_ok(s: &str) -> bool {
    if !s.is_ascii() {
        return false;
    }
    let t = strip_sign(s);
    let exp_ok = |c: &str| -> bool {
        let b = c.as_bytes();
        if b.is_empty() {
            return true;
        }
        if !b[0].is_ascii_digit() {
            return false;
        }
        let mut rest = &c[1..];
        rest = rest.strip_prefix(['e', 'E']).unwrap_or(rest);
        rest = strip_sign(rest);
        is_digit_run(rest)
    };
    for i in 0..=t.len() {
        if !is_digit_run(&t[..i]) {
            break;
        }
        let rest = &t[i..];
        let mut options = vec![rest];
        if let Some(r) = rest.strip_prefix('.') {
            options.push(r);
        }
        for r in options {
            for j in 0..=r.len() {
                if !is_digit_run(&r[..j]) {
                    break;
                }
                if exp_ok(&r[j..]) {
                    return true;
                }
            }
        }
    }
    false
}

impl Input {
    pub fn new() -> Self {
        Input {
            value: String::new(),
            placeholder: String::new(),
            password: false,
            max_length: 0,
            restrict: None,
            restrict_value: None,
            input_type: InputType::Text,
            selection: Selection::default(),
            select_on_focus: true,
            compact: false,
            cursor_blink: true,
            cursor_visible: true,
            blink_timer: None,
            validators: Vec::new(),
            validate_on: [true; 3],
            valid_empty: false,
            template_validator: None,
            valid: true,
            selecting: false,
            mouse_placed_cursor: false,
            scroll_x: Cell::new(0),
            access: self_access,
        }
    }

    /// Initial value (the cursor is placed at the end).
    pub fn value(mut self, value: impl Into<String>) -> Self {
        self.value = value.into();
        self.selection = Selection::cursor(char_len(&self.value));
        self
    }

    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    /// Obfuscate the value (rendered as `•`).
    pub fn password(mut self, password: bool) -> Self {
        self.password = password;
        self
    }

    /// Maximum length in characters (0 = unlimited).
    pub fn max_length(mut self, max_length: usize) -> Self {
        self.max_length = max_length;
        self
    }

    /// Only allow values whose every character satisfies `allowed`
    /// (Textual's `restrict` takes a regex; a character predicate is used here).
    pub fn restrict(mut self, allowed: impl Fn(char) -> bool + 'static) -> Self {
        self.restrict = Some(Rc::new(allowed));
        self
    }

    /// Only allow values for which `allowed` returns true (checked on every edit against the
    /// whole new value, like Textual's `restrict` regex full match).
    pub fn restrict_value(mut self, allowed: impl Fn(&str) -> bool + 'static) -> Self {
        self.restrict_value = Some(Rc::new(allowed));
        self
    }

    /// Only allow characters contained in `allowed`.
    pub fn restrict_chars(self, allowed: &str) -> Self {
        let set: Vec<char> = allowed.chars().collect();
        self.restrict(move |c| set.contains(&c))
    }

    pub fn input_type(mut self, t: InputType) -> Self {
        self.input_type = t;
        self
    }

    /// Whether the cursor blinks while focused (default true).
    pub fn cursor_blink(mut self, blink: bool) -> Self {
        self.cursor_blink = blink;
        self
    }

    /// Select all text when focused (default true).
    pub fn select_on_focus(mut self, select: bool) -> Self {
        self.select_on_focus = select;
        self
    }

    /// Compact style (no border, height 1).
    pub fn compact(mut self, compact: bool) -> Self {
        self.compact = compact;
        self
    }

    /// Add a validator (Textual: `validators=`). All validators run, in the order added.
    pub fn validator(mut self, validator: impl Validator + 'static) -> Self {
        self.validators.push(Rc::new(validator));
        self
    }

    /// Add validators (Textual: `validators=`).
    pub fn validators(mut self, validators: impl IntoIterator<Item = Box<dyn Validator>>) -> Self {
        self.validators
            .extend(validators.into_iter().map(SharedValidator::from));
        self
    }

    /// When to validate (Textual: `validate_on`; default: all three). An empty list never
    /// validates automatically; [`validate`](Input::validate) still does.
    pub fn validate_on(mut self, on: impl IntoIterator<Item = ValidateOn>) -> Self {
        self.set_validate_on(on);
        self
    }

    /// An empty value passes validation (Textual: `valid_empty`).
    pub fn valid_empty(mut self, valid_empty: bool) -> Self {
        self.valid_empty = valid_empty;
        self
    }

    // ------------------------------------------------------------------ accessors

    pub fn get_value(&self) -> &str {
        &self.value
    }

    pub fn get_placeholder(&self) -> &str {
        &self.placeholder
    }

    pub fn is_password(&self) -> bool {
        self.password
    }

    pub fn get_input_type(&self) -> InputType {
        self.input_type
    }

    /// Cursor position (character index): the end of the selection.
    pub fn cursor_position(&self) -> usize {
        self.selection.end
    }

    pub fn selection(&self) -> Selection {
        self.selection
    }

    pub fn selected_text(&self) -> &str {
        let (a, b) = self.selection.sorted();
        slice_chars(&self.value, a, b)
    }

    pub fn cursor_at_start(&self) -> bool {
        self.cursor_position() == 0
    }

    pub fn cursor_at_end(&self) -> bool {
        self.cursor_position() == char_len(&self.value)
    }

    /// Whether the value passed the last validation (true until the first one).
    pub fn is_valid(&self) -> bool {
        self.valid
    }

    /// The validators (without the implicit one of the `integer` / `number` types).
    pub fn get_validators(&self) -> &[Rc<dyn Validator>] {
        &self.validators
    }

    /// Whether the value is validated on `on`.
    pub fn validates_on(&self, on: ValidateOn) -> bool {
        self.validate_on[on as usize]
    }

    pub fn is_valid_empty(&self) -> bool {
        self.valid_empty
    }

    /// Whether the cursor is currently shown (blink phase).
    pub fn cursor_visible(&self) -> bool {
        self.cursor_visible
    }

    // ------------------------------------------------------------------ mutation

    pub fn set_placeholder(&mut self, placeholder: impl Into<String>) {
        self.placeholder = placeholder.into();
    }

    pub fn set_password(&mut self, password: bool) {
        self.password = password;
    }

    pub fn set_max_length(&mut self, max_length: usize) {
        self.max_length = max_length;
    }

    pub fn set_cursor_position(&mut self, pos: usize) {
        self.set_selection(Selection::cursor(pos));
    }

    /// Set the selection (clamped to the value).
    pub fn set_selection(&mut self, sel: Selection) {
        let len = char_len(&self.value);
        self.selection = Selection::new(sel.start.min(len), sel.end.min(len));
    }

    pub fn select_all(&mut self) {
        self.selection = Selection::new(0, char_len(&self.value));
    }

    /// Set the value, posting [`Changed`] if it differs. The selection is clamped.
    pub fn set_value(&mut self, value: impl Into<String>, ctx: &mut Ctx) {
        let value = value.into();
        if value == self.value {
            return;
        }
        self.value = value;
        let sel = self.selection;
        self.set_selection(sel);
        self.post_changed(ctx);
    }

    /// Validate (if `changed` is in `validate_on`) and post [`Changed`] (Textual: `_watch_value`).
    pub(crate) fn post_changed(&mut self, ctx: &mut Ctx) {
        let validation_result = self.validate_for(ValidateOn::Changed, ctx);
        let input_id = ctx.css_id(ctx.id()).map(String::from);
        ctx.post_message(Changed {
            value: self.value.clone(),
            input_id,
            validation_result,
        });
    }

    /// Replace the validators (re-validates nothing until the next trigger).
    pub fn set_validators(&mut self, validators: impl IntoIterator<Item = Box<dyn Validator>>) {
        self.validators = validators.into_iter().map(SharedValidator::from).collect();
    }

    /// Change `valid_empty`; like Textual, this re-validates and posts [`Changed`].
    pub fn set_valid_empty(&mut self, valid_empty: bool, ctx: &mut Ctx) {
        if self.valid_empty != valid_empty {
            self.valid_empty = valid_empty;
            self.post_changed(ctx);
        }
    }

    /// Change when the value is validated.
    pub fn set_validate_on(&mut self, on: impl IntoIterator<Item = ValidateOn>) {
        self.validate_on = [false; 3];
        for v in on {
            self.validate_on[v as usize] = true;
        }
    }

    pub fn clear(&mut self, ctx: &mut Ctx) {
        self.set_value("", ctx);
    }

    /// Replace characters `start..end` with `text` if the result is allowed (else ring the bell).
    pub fn replace(&mut self, text: &str, start: usize, end: usize, ctx: &mut Ctx) {
        let len = char_len(&self.value);
        let (a, b) = (start.min(len), end.min(len));
        let (start, end) = (a.min(b), a.max(b));
        let new_value = format!(
            "{}{}{}",
            slice_chars(&self.value, 0, start),
            text,
            slice_chars(&self.value, end, len)
        );
        if self.check_allowed(&new_value) {
            self.set_value(new_value, ctx);
            self.set_cursor_position(start + char_len(text));
        } else {
            ctx.bell();
        }
    }

    pub fn insert(&mut self, text: &str, index: usize, ctx: &mut Ctx) {
        self.replace(text, index, index, ctx);
    }

    pub fn insert_text_at_cursor(&mut self, text: &str, ctx: &mut Ctx) {
        let pos = self.cursor_position();
        self.insert(text, pos, ctx);
    }

    pub fn delete(&mut self, start: usize, end: usize, ctx: &mut Ctx) {
        self.replace("", start, end, ctx);
    }

    pub fn delete_selection(&mut self, ctx: &mut Ctx) {
        let Selection { start, end } = self.selection;
        self.delete(start, end, ctx);
    }

    fn check_allowed(&self, value: &str) -> bool {
        if self.max_length > 0 && char_len(value) > self.max_length {
            return false;
        }
        if let Some(r) = &self.restrict {
            if !value.chars().all(|c| r(c)) {
                return false;
            }
        }
        if let Some(r) = &self.restrict_value {
            if !r(value) {
                return false;
            }
        }
        match self.input_type {
            InputType::Text => true,
            InputType::Integer => integer_partial_ok(value),
            InputType::Number => number_partial_ok(value),
        }
    }

    /// Validate the current value (see [`validate_value`](Input::validate_value)).
    pub fn validate(&mut self, ctx: &mut Ctx) -> Option<ValidationResult> {
        let value = self.value.clone();
        self.validate_value(&value, ctx)
    }

    /// Run every validator on `value`, combine the results and set the `-valid` / `-invalid`
    /// classes (Textual: `Input.validate`). `None` (and valid) when there are no validators, or
    /// when `value` is empty and `valid_empty` is set.
    pub fn validate_value(&mut self, value: &str, ctx: &mut Ctx) -> Option<ValidationResult> {
        // Textual gives typed inputs a validator when none is given.
        let implicit: Option<SharedValidator> = match self.input_type {
            _ if !self.validators.is_empty() => None,
            InputType::Text => None,
            InputType::Integer => Some(Rc::new(Integer::new())),
            InputType::Number => Some(Rc::new(Number::new())),
        };
        let validators = match &implicit {
            Some(v) => std::slice::from_ref(v),
            None => &self.validators,
        };
        let result = if validators.is_empty() || (self.valid_empty && value.is_empty()) {
            None
        } else {
            Some(ValidationResult::merge(
                validators.iter().map(|v| v.validate(value)),
            ))
        };
        let result = match &self.template_validator {
            Some(template) => Some(ValidationResult::merge(
                std::iter::once(template.validate(value)).chain(result),
            )),
            None => result,
        };
        self.valid = result.as_ref().is_none_or(ValidationResult::is_valid);
        ctx.set_class(!self.valid, "-invalid");
        ctx.set_class(self.valid, "-valid");
        result
    }

    /// Validate if `on` is in `validate_on`.
    fn validate_for(&mut self, on: ValidateOn, ctx: &mut Ctx) -> Option<ValidationResult> {
        if self.validates_on(on) {
            self.validate(ctx)
        } else {
            None
        }
    }

    // ------------------------------------------------------------------ blink

    pub(crate) fn restart_blink(&mut self, ctx: &mut Ctx) {
        self.cursor_visible = true;
        if let Some(t) = self.blink_timer.take() {
            ctx.cancel_timer(t);
        }
        if self.cursor_blink && ctx.has_focus() {
            let access = self.access;
            let node = ctx.id();
            self.blink_timer = Some(ctx.app.add_timer(
                node,
                BLINK_INTERVAL,
                true,
                Box::new(move |w, _ctx| {
                    let w: &mut dyn crate::Widget = if access(w as &mut dyn Any).is_some() {
                        w
                    } else {
                        match w.delegate_mut() {
                            Some(d) => d,
                            None => return,
                        }
                    };
                    if let Some(input) = access(w as &mut dyn Any) {
                        input.cursor_visible = !input.cursor_visible;
                    }
                }),
            ));
        }
    }

    fn pause_blink(&mut self, visible: bool, ctx: &mut Ctx) {
        self.cursor_visible = visible;
        if let Some(t) = self.blink_timer.take() {
            ctx.cancel_timer(t);
        }
    }

    // ------------------------------------------------------------------ actions

    fn word_left_target(&self) -> usize {
        let chars: Vec<char> = self.value.chars().take(self.cursor_position()).collect();
        word_starts(&chars).last().unwrap_or(0)
    }

    /// Offset (from the cursor) of the next word start to the right, if any.
    fn word_right_offset(&self) -> Option<usize> {
        let chars: Vec<char> = self.value.chars().skip(self.cursor_position()).collect();
        let first = word_starts(&chars).next();
        first
    }

    pub fn action_cursor_left(&mut self, select: bool) {
        let Selection { start, end } = self.selection;
        if select {
            self.set_selection(Selection::new(start, end.saturating_sub(1)));
        } else if self.selection.is_empty() {
            self.set_cursor_position(end.saturating_sub(1));
        } else {
            self.set_cursor_position(start.min(end));
        }
    }

    pub fn action_cursor_right(&mut self, select: bool) {
        let Selection { start, end } = self.selection;
        if select {
            self.set_selection(Selection::new(start, end + 1));
        } else if self.selection.is_empty() {
            self.set_cursor_position(end + 1);
        } else {
            self.set_cursor_position(start.max(end));
        }
    }

    pub fn action_home(&mut self, select: bool) {
        if select {
            self.set_selection(Selection::new(self.cursor_position(), 0));
        } else {
            self.set_cursor_position(0);
        }
    }

    pub fn action_end(&mut self, select: bool) {
        let len = char_len(&self.value);
        if select {
            self.set_selection(Selection::new(self.cursor_position(), len));
        } else {
            self.set_cursor_position(len);
        }
    }

    pub fn action_cursor_left_word(&mut self, select: bool) {
        if self.password {
            // No hints about word boundaries in password fields.
            return self.action_home(select);
        }
        let target = self.word_left_target();
        if select {
            self.set_selection(Selection::new(self.selection.start, target));
        } else {
            self.set_cursor_position(target);
        }
    }

    pub fn action_cursor_right_word(&mut self, select: bool) {
        if self.password {
            return self.action_end(select);
        }
        let target = match self.word_right_offset() {
            Some(off) => self.selection.end + off,
            None => char_len(&self.value),
        };
        if select {
            self.set_selection(Selection::new(self.selection.start, target));
        } else {
            self.set_cursor_position(target);
        }
    }

    pub fn action_delete_left(&mut self, ctx: &mut Ctx) {
        if self.selection.is_empty() {
            let c = self.cursor_position();
            if c > 0 {
                self.delete(c - 1, c, ctx);
            }
        } else {
            self.delete_selection(ctx);
        }
    }

    pub fn action_delete_right(&mut self, ctx: &mut Ctx) {
        if self.selection.is_empty() {
            let c = self.cursor_position();
            self.delete(c, c + 1, ctx);
        } else {
            self.delete_selection(ctx);
        }
    }

    pub fn action_delete_left_word(&mut self, ctx: &mut Ctx) {
        if !self.selection.is_empty() {
            return self.delete_selection(ctx);
        }
        if self.password {
            return self.action_delete_left_all(ctx);
        }
        let target = self.word_left_target();
        let c = self.cursor_position();
        self.delete(target, c, ctx);
    }

    pub fn action_delete_left_all(&mut self, ctx: &mut Ctx) {
        if self.selection.is_empty() {
            let c = self.cursor_position();
            self.delete(0, c, ctx);
        } else {
            self.delete_selection(ctx);
        }
    }

    pub fn action_delete_right_word(&mut self, ctx: &mut Ctx) {
        if !self.selection.is_empty() {
            return self.delete_selection(ctx);
        }
        if self.password {
            return self.action_delete_right_all(ctx);
        }
        match self.word_right_offset() {
            None => self.action_delete_right_all(ctx),
            Some(off) => {
                let c = self.cursor_position();
                self.delete(c, c + off, ctx);
            }
        }
    }

    pub fn action_delete_right_all(&mut self, ctx: &mut Ctx) {
        if self.selection.is_empty() {
            let c = self.cursor_position();
            let len = char_len(&self.value);
            self.delete(c, len, ctx);
        } else {
            self.delete_selection(ctx);
        }
    }

    pub fn action_submit(&mut self, ctx: &mut Ctx) {
        let validation_result = self.validate_for(ValidateOn::Submitted, ctx);
        let input_id = ctx.css_id(ctx.id()).map(String::from);
        ctx.post_message(Submitted {
            value: self.value.clone(),
            input_id,
            validation_result,
        });
    }

    pub(crate) fn set_clipboard(text: &str) {
        set_clipboard_text(text);
    }

    /// The local clipboard's content.
    pub(crate) fn clipboard() -> String {
        CLIPBOARD.lock().map(|c| c.clone()).unwrap_or_default()
    }

    pub fn action_cut(&mut self, ctx: &mut Ctx) {
        // Local clipboard + OSC 52 system clipboard (Textual: `app.copy_to_clipboard`).
        ctx.copy_to_clipboard(self.selected_text());
        self.delete_selection(ctx);
    }

    pub fn action_copy(&mut self) -> bool {
        let text = self.selected_text().to_string();
        if text.is_empty() {
            return false;
        }
        Self::set_clipboard(&text);
        true
    }

    pub fn action_paste(&mut self, ctx: &mut Ctx) {
        let text = crate::terminal::paste_text(ctx.app);
        let Selection { start, end } = self.selection;
        self.replace(&text, start, end, ctx);
    }

    // ------------------------------------------------------------------ rendering helpers

    /// The value as displayed (masked for passwords).
    fn display_chars(&self) -> Vec<String> {
        if self.password {
            self.value.chars().map(|_| "•".to_string()).collect()
        } else {
            self.value.chars().map(|c| c.to_string()).collect()
        }
    }

    /// Character index for a cell offset within the content region (scroll included).
    fn cell_offset_to_index(&self, offset: i32) -> usize {
        let offset = offset + self.scroll_x.get();
        let mut cell = 0;
        let chars = self.display_chars();
        for (i, ch) in chars.iter().enumerate() {
            let w = cell_len(ch);
            if cell <= offset && offset < cell + w {
                return i;
            }
            cell += w;
        }
        offset.clamp(0, chars.len() as i32) as usize
    }
}

impl Widget for Input {
    fn check_consume_key(&self, _key: &str, character: Option<char>) -> bool {
        character.is_some_and(crate::binding::is_printable)
    }

    fn type_names(&self) -> &'static [&'static str] {
        &["Input"]
    }

    fn default_css(&self) -> &'static str {
        DEFAULT_CSS
    }

    fn can_focus(&self) -> bool {
        true
    }

    fn on_mount(&mut self, ctx: &mut Ctx) {
        ctx.set_class(self.compact, "-textual-compact");
        // Textual's value watcher runs for an initial value: validate it and post `Changed`.
        if !self.value.is_empty() {
            self.post_changed(ctx);
        }
    }

    fn on_unmount(&mut self, ctx: &mut Ctx) {
        if let Some(t) = self.blink_timer.take() {
            ctx.cancel_timer(t);
        }
    }

    fn help(&self) -> Option<&str> {
        Some(
            "\
## Input

A single-line text input.

- **shift+arrows** select text, **ctrl+shift+a** selects all.
- **enter** submits the value.
- **ctrl+x** / **ctrl+c** / **ctrl+v** cut, copy and paste.",
        )
    }

    fn bindings(&self) -> Vec<Binding> {
        vec![
            Binding::new("left", "cursor_left", "Move cursor left").hidden(),
            Binding::new(
                "shift+left",
                "cursor_left(True)",
                "Move cursor left and select",
            )
            .hidden(),
            Binding::new("ctrl+left", "cursor_left_word", "Move cursor left a word").hidden(),
            Binding::new(
                "ctrl+shift+left",
                "cursor_left_word(True)",
                "Move cursor left a word and select",
            )
            .hidden(),
            Binding::new(
                "right",
                "cursor_right",
                "Move cursor right or accept the completion suggestion",
            )
            .hidden(),
            Binding::new(
                "shift+right",
                "cursor_right(True)",
                "Move cursor right and select",
            )
            .hidden(),
            Binding::new(
                "ctrl+right",
                "cursor_right_word",
                "Move cursor right a word",
            )
            .hidden(),
            Binding::new(
                "ctrl+shift+right",
                "cursor_right_word(True)",
                "Move cursor right a word and select",
            )
            .hidden(),
            Binding::new("backspace", "delete_left", "Delete character left").hidden(),
            Binding::new("ctrl+shift+a", "select_all", "Select all").hidden(),
            Binding::new("home,ctrl+a", "home", "Go to start").hidden(),
            Binding::new("end,ctrl+e", "end", "Go to end").hidden(),
            Binding::new("shift+home", "home(True)", "Select line start").hidden(),
            Binding::new("shift+end", "end(True)", "Select line end").hidden(),
            Binding::new("delete,ctrl+d", "delete_right", "Delete character right").hidden(),
            Binding::new("enter", "submit", "Submit").hidden(),
            Binding::new("ctrl+w", "delete_left_word", "Delete left to start of word").hidden(),
            Binding::new("ctrl+u", "delete_left_all", "Delete all to the left").hidden(),
            Binding::new(
                "ctrl+backspace,alt+backspace",
                "delete_right_word",
                "Delete right to start of word",
            )
            .hidden(),
            Binding::new("ctrl+k", "delete_right_all", "Delete all to the right").hidden(),
            Binding::new("ctrl+x", "cut", "Cut selected text").hidden(),
            Binding::new("ctrl+c", "copy", "Copy selected text").hidden(),
            Binding::new("ctrl+v", "paste", "Paste text from the clipboard").hidden(),
        ]
    }

    fn run_action(&mut self, action: &str, args: &[String], ctx: &mut Ctx) -> bool {
        let select = args
            .first()
            .map(|a| a.eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        match action {
            "cursor_left" => self.action_cursor_left(select),
            "cursor_right" => self.action_cursor_right(select),
            "cursor_left_word" => self.action_cursor_left_word(select),
            "cursor_right_word" => self.action_cursor_right_word(select),
            "home" => self.action_home(select),
            "end" => self.action_end(select),
            "select_all" => self.select_all(),
            "delete_left" => self.action_delete_left(ctx),
            "delete_right" => self.action_delete_right(ctx),
            "delete_left_word" => self.action_delete_left_word(ctx),
            "delete_left_all" => self.action_delete_left_all(ctx),
            "delete_right_word" => self.action_delete_right_word(ctx),
            "delete_right_all" => self.action_delete_right_all(ctx),
            "submit" => self.action_submit(ctx),
            "cut" => self.action_cut(ctx),
            // Like Textual's SkipAction: let the key fall through when nothing is selected.
            "copy" => {
                let copied = self.action_copy();
                if copied {
                    ctx.copy_to_clipboard(self.selected_text());
                }
                return copied;
            }
            "paste" => self.action_paste(ctx),
            _ => return false,
        }
        true
    }

    fn on_event(&mut self, event: &Event, ctx: &mut Ctx) -> Flow {
        match event {
            Event::Key(k) => {
                self.restart_blink(ctx);
                if let Some(c) = k.character.filter(|c| !c.is_control()) {
                    let Selection { start, end } = self.selection;
                    if self.selection.is_empty() {
                        self.insert_text_at_cursor(&c.to_string(), ctx);
                    } else {
                        self.replace(&c.to_string(), start, end, ctx);
                    }
                    return Flow::Stop;
                }
                Flow::Continue
            }
            Event::Paste(text) => {
                if let Some(line) = text.lines().next().filter(|l| !l.is_empty()) {
                    let Selection { start, end } = self.selection;
                    self.replace(line, start, end, ctx);
                }
                Flow::Stop
            }
            Event::Focus => {
                if self.select_on_focus && !self.mouse_placed_cursor {
                    self.select_all();
                }
                self.mouse_placed_cursor = false;
                self.restart_blink(ctx);
                Flow::Continue
            }
            Event::Blur => {
                self.pause_blink(false, ctx);
                self.selecting = false;
                let validation_result = self.validate_for(ValidateOn::Blur, ctx);
                let input_id = ctx.css_id(ctx.id()).map(String::from);
                ctx.post_message(Blurred {
                    value: self.value.clone(),
                    input_id,
                    validation_result,
                });
                Flow::Continue
            }
            Event::MouseDown(m) => {
                self.pause_blink(true, ctx);
                let x = m.screen_x - ctx.content_region().x;
                let idx = self.cell_offset_to_index(x);
                self.set_cursor_position(idx);
                self.selecting = true;
                self.mouse_placed_cursor = true;
                Flow::Stop
            }
            Event::MouseMove(m) if self.selecting => {
                let x = m.screen_x - ctx.content_region().x;
                let idx = self.cell_offset_to_index(x);
                self.set_selection(Selection::new(self.selection.start, idx));
                Flow::Stop
            }
            Event::MouseUp(_) | Event::Click(_) => {
                self.mouse_placed_cursor = false;
                if self.selecting {
                    self.selecting = false;
                    self.restart_blink(ctx);
                }
                Flow::Stop
            }
            Event::Leave => {
                self.selecting = false;
                Flow::Continue
            }
            _ => Flow::Continue,
        }
    }

    fn content_width(&self, _ctx: &RenderCtx) -> Option<i32> {
        if !self.placeholder.is_empty() && self.value.is_empty() {
            return Some(cell_len(&self.placeholder));
        }
        let cells: i32 = self.display_chars().iter().map(|c| cell_len(c)).sum();
        Some(cells + 1)
    }

    fn content_height(&self, _ctx: &RenderCtx, _width: i32) -> Option<i32> {
        Some(1)
    }

    fn render(&self, ctx: &RenderCtx) -> Content {
        let width = ctx.size.width.max(0);
        let focused = ctx.is_focused();
        let show_cursor = focused && self.cursor_visible;
        let cursor_style = ctx.component_style("input--cursor");
        let mut strip = Strip::default();

        if self.value.is_empty() {
            self.scroll_x.set(0);
            let placeholder_style = ctx.component_style("input--placeholder");
            let mut chars = self.placeholder.chars();
            if show_cursor {
                match chars.next() {
                    Some(c) => strip.push(Span::new(
                        c.to_string(),
                        placeholder_style.combine(&cursor_style),
                    )),
                    None => strip.push(Span::new(" ", cursor_style)),
                }
            }
            let rest: String = chars.collect();
            if !rest.is_empty() {
                strip.push(Span::new(rest, placeholder_style));
            }
            return Content::Lines(vec![strip.set_cell_len(width, &Style::default())]);
        }

        let chars = self.display_chars();
        let cursor = self.cursor_position();
        let (sel_a, sel_b) = self.selection.sorted();
        let selection_style = ctx.component_style("input--selection");
        let mut cursor_cell = 0;
        let mut cell = 0;
        for (i, ch) in chars.iter().enumerate() {
            let mut style = Style::default();
            if focused && i >= sel_a && i < sel_b {
                style = selection_style.clone();
            }
            if i == cursor {
                cursor_cell = cell;
                if show_cursor {
                    style = style.combine(&cursor_style);
                }
            }
            cell += cell_len(ch);
            strip.push(Span::new(ch.clone(), style));
        }
        if cursor >= chars.len() {
            cursor_cell = cell;
            if show_cursor {
                strip.push(Span::new(" ", cursor_style));
            }
        }
        let total = cell + 1;

        // Keep the cursor cell visible (Textual: scroll_to_region on selection change).
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integer_restrict() {
        for ok in ["", "-", "+", "12", "-1_000", "1_2_3"] {
            assert!(integer_partial_ok(ok), "{ok}");
        }
        for bad in ["a", "1.5", "_1", "1__", "--1", "1-"] {
            assert!(!integer_partial_ok(bad), "{bad}");
        }
    }

    #[test]
    fn number_restrict() {
        for ok in [
            "", "-", "1.", ".5", "1.5", "1e5", "1.5e-3", "-1_0.2_5", "1e",
        ] {
            assert!(number_partial_ok(ok), "{ok}");
        }
        for bad in ["a", "1..2", "e5", "1.5.", "1e5e"] {
            assert!(!number_partial_ok(bad), "{bad}");
        }
    }

    #[test]
    fn type_validators_are_textuals() {
        use crate::validation::{Integer, Number, Validator};
        for (v, ok) in [
            ("-12", true),
            ("1_000", true),
            ("", false),
            ("-", false),
            ("1_", false),
        ] {
            assert_eq!(Integer::new().validate(v).is_valid(), ok, "{v:?}");
        }
        for (v, ok) in [
            ("1.5e3", true),
            (".5", true),
            ("1e", false),
            ("inf", false),
            ("", false),
        ] {
            assert_eq!(Number::new().validate(v).is_valid(), ok, "{v:?}");
        }
    }

    #[test]
    fn word_boundaries() {
        let chars: Vec<char> = "foo bar-baz".chars().collect();
        assert_eq!(word_starts(&chars).collect::<Vec<_>>(), vec![4, 8]);
    }
}

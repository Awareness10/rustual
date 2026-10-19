//! Validating string values (port of `textual/validation.py`).
//!
//! A [`Validator`] checks a value and returns a [`ValidationResult`]: success, or one or more
//! [`Failure`]s, each with a user-facing description. [`Input`](crate::widgets::Input) and
//! [`MaskedInput`](crate::widgets::MaskedInput) run their validators as the value changes, on
//! blur and on submit, set the `-valid` / `-invalid` classes, and attach the result to their
//! `Changed` / `Blurred` / `Submitted` messages. Validators work on any string, too:
//!
//! ```
//! use rustual::validation::{Function, Integer, Length, Validator, ValidationResult};
//!
//! let age = Integer::new().minimum(0).maximum(150);
//! assert!(age.validate("42").is_valid());
//! assert_eq!(age.validate("200").failure_descriptions(), ["Must be between 0 and 150."]);
//! assert_eq!(age.validate("4.2").failure_descriptions(), ["Must be a valid integer."]);
//!
//! let name = Length::new().minimum(1).failure_description("A name is required.");
//! let no_spaces = Function::new(|v| !v.contains(' ')).failure_description("No spaces.");
//! let result = ValidationResult::merge([name.validate(""), no_spaces.validate("")]);
//! assert_eq!(result.failure_descriptions(), ["A name is required."]);
//! ```
//!
//! Custom validators implement the trait (Textual's `Validator` subclass):
//!
//! ```
//! use rustual::validation::{ValidationResult, Validator};
//!
//! struct Palindrome;
//!
//! impl Validator for Palindrome {
//!     fn validate(&self, value: &str) -> ValidationResult {
//!         if value.chars().eq(value.chars().rev()) {
//!             self.success()
//!         } else {
//!             self.failure(Some("That's not a palindrome :/"), Some(value))
//!         }
//!     }
//! }
//!
//! assert!(Palindrome.validate("racecar").is_valid());
//! assert_eq!(Palindrome.validate("rust").failures[0].value.as_deref(), Some("rust"));
//! ```
//!
//! Values are read like Python reads them, so validators accept exactly what Textual's do:
//! [`Number`] and [`Integer`] follow `float()` / `int()` (digits of any script, surrounding
//! whitespace, `_` between digits), [`Url`] follows `urllib.parse.urlparse`, and failure
//! descriptions print bounds and patterns as Python would (`10`, `2.5`, `1e+20`, `'\\d+'`).
//!
//! Differences from Textual: [`Regex`] uses the [`regex`] crate (linear time, no look-around or
//! backreferences; Python-only syntax such as `\Z`, `(?#...)` and octal escapes is translated)
//! and its constructor returns an error for an invalid pattern instead of failing on use;
//! failure subclasses are a [`FailureKind`]; Textual's `URL` is named [`Url`].

mod pyre;
mod python;
mod tables;

pub(crate) use python::{float_repr, py_float, py_int_ok};

use std::fmt;
use std::rc::Rc;

// ---------------------------------------------------------------------------------- results

/// The result of [`Validator::validate`] (Textual: `ValidationResult`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ValidationResult {
    /// Why the value is invalid. Empty if it is valid.
    pub failures: Vec<Failure>,
}

impl ValidationResult {
    /// A successful result.
    pub fn success() -> Self {
        ValidationResult::default()
    }

    /// A failed result with these failures.
    pub fn failure(failures: Vec<Failure>) -> Self {
        ValidationResult { failures }
    }

    /// Combine results: valid if all are valid, else a failure listing every failure in order.
    pub fn merge(results: impl IntoIterator<Item = ValidationResult>) -> Self {
        let failures: Vec<Failure> = results.into_iter().flat_map(|r| r.failures).collect();
        ValidationResult::failure(failures)
    }

    /// Whether validation succeeded (there are no failures).
    pub fn is_valid(&self) -> bool {
        self.failures.is_empty()
    }

    /// The descriptions of the failures (those that have one), in order.
    pub fn failure_descriptions(&self) -> Vec<String> {
        self.failures
            .iter()
            .filter_map(|f| f.description.clone())
            .collect()
    }
}

/// Why a [`Failure`] happened (Textual's `Failure` subclasses).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum FailureKind {
    /// A plain failure (Textual: `Failure`), e.g. from [`Validator::failure`].
    Failure,
    /// The value doesn't match the [`Regex`] (Textual: `Regex.NoResults`).
    NoResults,
    /// The value isn't a finite number (Textual: `Number.NotANumber`).
    NotANumber,
    /// The number is outside the range (Textual: `Number.NotInRange`).
    NotInRange,
    /// The number isn't an integer (Textual: `Integer.NotAnInteger`).
    NotAnInteger,
    /// The length is outside the range (Textual: `Length.Incorrect`).
    Incorrect,
    /// The [`Function`] returned `false` (Textual: `Function.ReturnedFalse`).
    ReturnedFalse,
    /// The value isn't a URL (Textual: `URL.InvalidURL`).
    InvalidUrl,
    /// A failure defined by a custom validator.
    Custom(String),
}

/// Information about a validation failure (Textual: `Failure`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failure {
    /// What failed.
    pub kind: FailureKind,
    /// The name of the validator that produced the failure (its type name, e.g. `"Number"`).
    pub validator: String,
    /// The value that failed validation.
    pub value: Option<String>,
    /// A user-facing description of the failure.
    pub description: Option<String>,
}

impl Failure {
    /// A failure from `validator`. Without a `description`, it comes from the validator: its
    /// [`failure description`](Validator::get_failure_description) if set, else
    /// [`Validator::describe_failure`] (Textual: `Failure.__post_init__`).
    pub fn new(
        validator: &dyn Validator,
        kind: FailureKind,
        value: Option<&str>,
        description: Option<&str>,
    ) -> Self {
        let mut failure = Failure {
            kind,
            validator: validator.name().to_string(),
            value: value.map(String::from),
            description: description.map(String::from),
        };
        if failure.description.is_none() {
            failure.description = match validator.get_failure_description() {
                Some(d) => Some(d.to_string()),
                None => validator.describe_failure(&failure),
            };
        }
        failure
    }
}

// ---------------------------------------------------------------------------------- Validator

/// Validates string values (Textual: `Validator`).
///
/// Implement [`validate`](Validator::validate), returning [`success`](Validator::success) or
/// [`failure`](Validator::failure). Override [`describe_failure`](Validator::describe_failure)
/// to describe failures that have no description of their own.
pub trait Validator {
    /// Validate `value`.
    fn validate(&self, value: &str) -> ValidationResult;

    /// The description given to every failure of this validator, overriding
    /// [`describe_failure`](Validator::describe_failure) (Textual: `failure_description`).
    fn get_failure_description(&self) -> Option<&str> {
        None
    }

    /// Describe a failure that has no description (called only when neither the failure nor
    /// the validator has one).
    fn describe_failure(&self, _failure: &Failure) -> Option<String> {
        self.get_failure_description().map(String::from)
    }

    /// The validator's name, recorded in its failures. Defaults to the type name.
    fn name(&self) -> &str {
        let name = std::any::type_name::<Self>();
        let name = name.split('<').next().unwrap_or(name);
        name.rsplit("::").next().unwrap_or(name)
    }

    /// A successful result.
    fn success(&self) -> ValidationResult
    where
        Self: Sized,
    {
        ValidationResult::success()
    }

    /// A failed result with one [`FailureKind::Failure`], described by `description` (or by the
    /// validator, see [`Failure::new`]).
    fn failure(&self, description: Option<&str>, value: Option<&str>) -> ValidationResult
    where
        Self: Sized,
    {
        ValidationResult::failure(vec![Failure::new(
            self,
            FailureKind::Failure,
            value,
            description,
        )])
    }

    /// A failed result with these failures (a plain one if `failures` is empty).
    fn failures(&self, failures: Vec<Failure>) -> ValidationResult
    where
        Self: Sized,
    {
        if failures.is_empty() {
            self.failure(None, None)
        } else {
            ValidationResult::failure(failures)
        }
    }
}

impl<V: Validator + ?Sized> Validator for Box<V> {
    fn validate(&self, value: &str) -> ValidationResult {
        (**self).validate(value)
    }
    fn get_failure_description(&self) -> Option<&str> {
        (**self).get_failure_description()
    }
    fn describe_failure(&self, failure: &Failure) -> Option<String> {
        (**self).describe_failure(failure)
    }
    fn name(&self) -> &str {
        (**self).name()
    }
}

impl<V: Validator + ?Sized> Validator for Rc<V> {
    fn validate(&self, value: &str) -> ValidationResult {
        (**self).validate(value)
    }
    fn get_failure_description(&self) -> Option<&str> {
        (**self).get_failure_description()
    }
    fn describe_failure(&self, failure: &Failure) -> Option<String> {
        (**self).describe_failure(failure)
    }
    fn name(&self) -> &str {
        (**self).name()
    }
}

/// A result with one failure of `kind`, described by the validator.
fn fail(validator: &dyn Validator, kind: FailureKind, value: &str) -> ValidationResult {
    ValidationResult::failure(vec![Failure::new(validator, kind, Some(value), None)])
}

// ---------------------------------------------------------------------------------- bounds

/// An inclusive bound of a [`Number`] or [`Integer`] range. Keeps whether it was given as an
/// integer or a float, so descriptions print `10` or `10.0` as Python would.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Bound {
    Int(i64),
    Float(f64),
}

impl Bound {
    /// The bound as a float.
    pub fn as_f64(self) -> f64 {
        match self {
            Bound::Int(i) => i as f64,
            Bound::Float(f) => f,
        }
    }

    /// `value < self`, compared exactly (as Python compares a float with an int).
    fn above(self, value: f64) -> bool {
        match self {
            Bound::Float(f) => value < f,
            Bound::Int(i) => cmp_f64_i64(value, i) == Some(std::cmp::Ordering::Less),
        }
    }

    /// `value > self`, compared exactly.
    fn below(self, value: f64) -> bool {
        match self {
            Bound::Float(f) => value > f,
            Bound::Int(i) => cmp_f64_i64(value, i) == Some(std::cmp::Ordering::Greater),
        }
    }
}

/// Exact comparison of a float with an integer (no rounding of large integers).
fn cmp_f64_i64(f: f64, i: i64) -> Option<std::cmp::Ordering> {
    use std::cmp::Ordering;
    if f.is_nan() {
        return None;
    }
    // Every integer with |i| < 2^53 converts exactly; beyond that, compare the integer part.
    const TWO_63: f64 = 9_223_372_036_854_775_808.0;
    if f >= TWO_63 {
        return Some(Ordering::Greater);
    }
    if f < -TWO_63 {
        return Some(Ordering::Less);
    }
    let t = f.trunc();
    match (t as i64).cmp(&i) {
        Ordering::Equal => (f - t).partial_cmp(&0.0),
        other => Some(other),
    }
}

impl fmt::Display for Bound {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Bound::Int(i) => write!(f, "{i}"),
            Bound::Float(x) => f.write_str(&float_repr(*x)),
        }
    }
}

macro_rules! bound_from_int {
    ($($t:ty),*) => {$(
        impl From<$t> for Bound {
            fn from(v: $t) -> Self {
                Bound::Int(v as i64)
            }
        }
    )*};
}
bound_from_int!(i8, i16, i32, i64, u8, u16, u32);

impl From<f64> for Bound {
    fn from(v: f64) -> Self {
        Bound::Float(v)
    }
}

impl From<f32> for Bound {
    fn from(v: f32) -> Self {
        Bound::Float(v as f64)
    }
}

/// Describe a range failure ("less than or equal to", "greater than or equal to", "between").
fn describe_range(minimum: Option<Bound>, maximum: Option<Bound>) -> String {
    match (minimum, maximum) {
        (None, Some(max)) => format!("Must be less than or equal to {max}."),
        (Some(min), None) => format!("Must be greater than or equal to {min}."),
        (min, max) => format!("Must be between {} and {}.", opt(min), opt(max)),
    }
}

/// Python's `f"{x}"` for an optional value (`None` when absent).
fn opt<T: fmt::Display>(v: Option<T>) -> String {
    v.map_or_else(|| "None".to_string(), |v| v.to_string())
}

// ---------------------------------------------------------------------------------- Number

/// A number (decimal, integer or scientific notation; not NaN or infinite), optionally within an
/// inclusive range (Textual: `Number`).
#[derive(Debug, Clone, Default)]
pub struct Number {
    /// The inclusive minimum, if any.
    pub minimum: Option<Bound>,
    /// The inclusive maximum, if any.
    pub maximum: Option<Bound>,
    /// Overrides every failure description.
    pub failure_description: Option<String>,
}

impl Number {
    pub fn new() -> Self {
        Number::default()
    }

    /// The inclusive minimum (`1` or `0.5`).
    pub fn minimum(mut self, minimum: impl Into<Bound>) -> Self {
        self.minimum = Some(minimum.into());
        self
    }

    /// The inclusive maximum.
    pub fn maximum(mut self, maximum: impl Into<Bound>) -> Self {
        self.maximum = Some(maximum.into());
        self
    }

    /// The description of every failure.
    pub fn failure_description(mut self, description: impl Into<String>) -> Self {
        self.failure_description = Some(description.into());
        self
    }

    fn in_range(&self, value: f64) -> bool {
        !self.minimum.is_some_and(|m| m.above(value))
            && !self.maximum.is_some_and(|m| m.below(value))
    }

    /// The number check shared with [`Integer`]: `Err` with the failure kind.
    fn check(&self, value: &str) -> Result<(), FailureKind> {
        match py_float(value) {
            Some(f) if f.is_finite() => {
                if self.in_range(f) {
                    Ok(())
                } else {
                    Err(FailureKind::NotInRange)
                }
            }
            _ => Err(FailureKind::NotANumber),
        }
    }
}

impl Validator for Number {
    fn validate(&self, value: &str) -> ValidationResult {
        match self.check(value) {
            Ok(()) => self.success(),
            Err(kind) => fail(self, kind, value),
        }
    }

    fn get_failure_description(&self) -> Option<&str> {
        self.failure_description.as_deref()
    }

    fn describe_failure(&self, failure: &Failure) -> Option<String> {
        match failure.kind {
            FailureKind::NotANumber => Some("Must be a valid number.".into()),
            FailureKind::NotInRange => Some(describe_range(self.minimum, self.maximum)),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------------- Integer

/// An integer, optionally within an inclusive range (Textual: `Integer`). The value must be a
/// number (in range) and parse as an integer: `"1e3"` and `"1.0"` are not integers.
#[derive(Debug, Clone, Default)]
pub struct Integer {
    /// The inclusive minimum, if any.
    pub minimum: Option<Bound>,
    /// The inclusive maximum, if any.
    pub maximum: Option<Bound>,
    /// Overrides every failure description.
    pub failure_description: Option<String>,
}

impl Integer {
    pub fn new() -> Self {
        Integer::default()
    }

    /// The inclusive minimum.
    pub fn minimum(mut self, minimum: impl Into<Bound>) -> Self {
        self.minimum = Some(minimum.into());
        self
    }

    /// The inclusive maximum.
    pub fn maximum(mut self, maximum: impl Into<Bound>) -> Self {
        self.maximum = Some(maximum.into());
        self
    }

    /// The description of every failure.
    pub fn failure_description(mut self, description: impl Into<String>) -> Self {
        self.failure_description = Some(description.into());
        self
    }
}

impl Validator for Integer {
    fn validate(&self, value: &str) -> ValidationResult {
        let number = Number {
            minimum: self.minimum,
            maximum: self.maximum,
            failure_description: None,
        };
        match number.check(value) {
            Err(kind) => fail(self, kind, value),
            Ok(()) if !py_int_ok(value) => fail(self, FailureKind::NotAnInteger, value),
            Ok(()) => self.success(),
        }
    }

    fn get_failure_description(&self) -> Option<&str> {
        self.failure_description.as_deref()
    }

    fn describe_failure(&self, failure: &Failure) -> Option<String> {
        match failure.kind {
            FailureKind::NotANumber | FailureKind::NotAnInteger => {
                Some("Must be a valid integer.".into())
            }
            FailureKind::NotInRange => Some(describe_range(self.minimum, self.maximum)),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------------- Length

/// A length (in characters) within an inclusive range (Textual: `Length`).
#[derive(Debug, Clone, Default)]
pub struct Length {
    /// The inclusive minimum length, if any.
    pub minimum: Option<usize>,
    /// The inclusive maximum length, if any.
    pub maximum: Option<usize>,
    /// Overrides every failure description.
    pub failure_description: Option<String>,
}

impl Length {
    pub fn new() -> Self {
        Length::default()
    }

    /// The inclusive minimum length.
    pub fn minimum(mut self, minimum: usize) -> Self {
        self.minimum = Some(minimum);
        self
    }

    /// The inclusive maximum length.
    pub fn maximum(mut self, maximum: usize) -> Self {
        self.maximum = Some(maximum);
        self
    }

    /// The description of every failure.
    pub fn failure_description(mut self, description: impl Into<String>) -> Self {
        self.failure_description = Some(description.into());
        self
    }
}

impl Validator for Length {
    fn validate(&self, value: &str) -> ValidationResult {
        let len = value.chars().count();
        let too_short = self.minimum.is_some_and(|m| len < m);
        let too_long = self.maximum.is_some_and(|m| len > m);
        if too_short || too_long {
            fail(self, FailureKind::Incorrect, value)
        } else {
            self.success()
        }
    }

    fn get_failure_description(&self) -> Option<&str> {
        self.failure_description.as_deref()
    }

    fn describe_failure(&self, failure: &Failure) -> Option<String> {
        if failure.kind != FailureKind::Incorrect {
            return None;
        }
        // Textual's wording ("shorter than" / "longer than" for inclusive bounds).
        Some(match (self.minimum, self.maximum) {
            (None, Some(max)) => format!("Must be shorter than {max} characters."),
            (Some(min), None) => format!("Must be longer than {min} characters."),
            (min, max) => format!("Must be between {} and {} characters.", opt(min), opt(max)),
        })
    }
}

// ---------------------------------------------------------------------------------- Function

/// Valid when a function returns `true` (Textual: `Function`). Its failures are described only
/// by the [`failure_description`](Function::failure_description).
#[derive(Clone)]
pub struct Function {
    /// Returns `true` for valid values.
    pub function: Rc<dyn Fn(&str) -> bool>,
    /// The description of failures.
    pub failure_description: Option<String>,
}

impl Function {
    pub fn new(function: impl Fn(&str) -> bool + 'static) -> Self {
        Function {
            function: Rc::new(function),
            failure_description: None,
        }
    }

    /// The description of failures.
    pub fn failure_description(mut self, description: impl Into<String>) -> Self {
        self.failure_description = Some(description.into());
        self
    }
}

impl fmt::Debug for Function {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Function")
            .field("failure_description", &self.failure_description)
            .finish_non_exhaustive()
    }
}

impl Validator for Function {
    fn validate(&self, value: &str) -> ValidationResult {
        if (self.function)(value) {
            self.success()
        } else {
            fail(self, FailureKind::ReturnedFalse, value)
        }
    }

    fn get_failure_description(&self) -> Option<&str> {
        self.failure_description.as_deref()
    }
}

// ---------------------------------------------------------------------------------- Url

/// A URL with a scheme and a network location, as `urllib.parse.urlparse` splits it
/// (Textual: `URL`): `https://example.com` is valid, `example.com` and `mailto:a@b.c` are not.
#[derive(Debug, Clone, Default)]
pub struct Url {
    /// Overrides every failure description.
    pub failure_description: Option<String>,
}

impl Url {
    pub fn new() -> Self {
        Url::default()
    }

    /// The description of every failure.
    pub fn failure_description(mut self, description: impl Into<String>) -> Self {
        self.failure_description = Some(description.into());
        self
    }
}

impl Validator for Url {
    fn validate(&self, value: &str) -> ValidationResult {
        if python::url_has_scheme_and_netloc(value) {
            self.success()
        } else {
            fail(self, FailureKind::InvalidUrl, value)
        }
    }

    fn get_failure_description(&self) -> Option<&str> {
        self.failure_description.as_deref()
    }

    fn describe_failure(&self, _failure: &Failure) -> Option<String> {
        Some("Must be a valid URL.".into())
    }
}

// ---------------------------------------------------------------------------------- Regex

/// Flags for [`Regex`], with the values of Python's `re` flags.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct RegexFlags(u32);

impl RegexFlags {
    /// No flags (Textual's default `flags=0`).
    pub const NONE: RegexFlags = RegexFlags(0);
    /// Case-insensitive matching (`re.IGNORECASE`).
    pub const IGNORECASE: RegexFlags = RegexFlags(2);
    /// `^` / `$` also match at line boundaries (`re.MULTILINE`).
    pub const MULTILINE: RegexFlags = RegexFlags(8);
    /// `.` also matches a newline (`re.DOTALL`).
    pub const DOTALL: RegexFlags = RegexFlags(16);
    /// Unicode matching, the default for strings (`re.UNICODE`).
    pub const UNICODE: RegexFlags = RegexFlags(32);
    /// Whitespace and `#` comments in the pattern are ignored (`re.VERBOSE`).
    pub const VERBOSE: RegexFlags = RegexFlags(64);
    /// `\d`, `\w`, `\s` and `\b` match ASCII only (`re.ASCII`).
    pub const ASCII: RegexFlags = RegexFlags(256);

    const NAMES: [(RegexFlags, &'static str); 6] = [
        (Self::IGNORECASE, "IGNORECASE"),
        (Self::MULTILINE, "MULTILINE"),
        (Self::DOTALL, "DOTALL"),
        (Self::UNICODE, "UNICODE"),
        (Self::VERBOSE, "VERBOSE"),
        (Self::ASCII, "ASCII"),
    ];

    /// The flags as Python's integer value.
    pub fn bits(self) -> u32 {
        self.0
    }

    /// Whether all of `other`'s flags are set.
    pub fn contains(self, other: RegexFlags) -> bool {
        self.0 & other.0 == other.0
    }
}

impl std::ops::BitOr for RegexFlags {
    type Output = RegexFlags;
    fn bitor(self, rhs: RegexFlags) -> RegexFlags {
        RegexFlags(self.0 | rhs.0)
    }
}

impl std::ops::BitOrAssign for RegexFlags {
    fn bitor_assign(&mut self, rhs: RegexFlags) {
        self.0 |= rhs.0;
    }
}

/// As Python formats them: `0` for no flags, else `re.IGNORECASE|re.MULTILINE`.
impl fmt::Display for RegexFlags {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0 == 0 {
            return f.write_str("0");
        }
        let names: Vec<String> = Self::NAMES
            .iter()
            .filter(|(flag, _)| self.contains(*flag))
            .map(|(_, name)| format!("re.{name}"))
            .collect();
        f.write_str(&names.join("|"))
    }
}

/// The whole value matches a regular expression (Textual: `Regex`, which uses `re.fullmatch`).
#[derive(Debug, Clone)]
pub struct Regex {
    pattern: String,
    flags: RegexFlags,
    regex: regex::Regex,
    /// Overrides every failure description.
    pub failure_description: Option<String>,
}

impl Regex {
    /// A validator for `pattern` (see the [module docs](self) for the syntax).
    pub fn new(pattern: &str) -> Result<Regex, regex::Error> {
        Regex::with_flags(pattern, RegexFlags::NONE)
    }

    /// A validator for `pattern` with Python-style `flags`.
    pub fn with_flags(pattern: &str, flags: RegexFlags) -> Result<Regex, regex::Error> {
        Ok(Regex {
            pattern: pattern.to_string(),
            flags,
            regex: pyre::compile_fullmatch(pattern, flags)?,
            failure_description: None,
        })
    }

    /// The description of every failure.
    pub fn failure_description(mut self, description: impl Into<String>) -> Self {
        self.failure_description = Some(description.into());
        self
    }

    /// The pattern, as given.
    pub fn pattern(&self) -> &str {
        &self.pattern
    }

    /// The flags.
    pub fn flags(&self) -> RegexFlags {
        self.flags
    }

    /// Whether the whole of `value` matches.
    pub fn is_match(&self, value: &str) -> bool {
        self.regex.is_match(value)
    }
}

impl Validator for Regex {
    fn validate(&self, value: &str) -> ValidationResult {
        if self.is_match(value) {
            self.success()
        } else {
            fail(self, FailureKind::NoResults, value)
        }
    }

    fn get_failure_description(&self) -> Option<&str> {
        self.failure_description.as_deref()
    }

    fn describe_failure(&self, _failure: &Failure) -> Option<String> {
        Some(format!(
            "Must match regular expression {} (flags={}).",
            python::str_repr(&self.pattern),
            self.flags
        ))
    }
}

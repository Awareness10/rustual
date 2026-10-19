//! `rustual::validation` compared with Textual's validators (goldens from
//! `scripts/validation_golden.py`: every value validated by real Textual), plus the API that
//! the goldens can't exercise (custom validators, description precedence, merging).

use rustual::validation::{
    Bound, Failure, FailureKind, Function, Integer, Length, Number, Regex, RegexFlags, Url,
    ValidationResult, Validator,
};
use std::cell::Cell;
use std::rc::Rc;

include!("golden/validation.rs");

/// Textual's name for a failure kind (`type(failure).__qualname__`).
fn kind_name(kind: &FailureKind) -> &str {
    match kind {
        FailureKind::Failure => "Failure",
        FailureKind::NoResults => "Regex.NoResults",
        FailureKind::NotANumber => "Number.NotANumber",
        FailureKind::NotInRange => "Number.NotInRange",
        FailureKind::NotAnInteger => "Integer.NotAnInteger",
        FailureKind::Incorrect => "Length.Incorrect",
        FailureKind::ReturnedFalse => "Function.ReturnedFalse",
        FailureKind::InvalidUrl => "URL.InvalidURL",
        FailureKind::Custom(name) => name,
    }
}

#[test]
fn matches_textual_on_every_golden_case() {
    let mut mismatches = Vec::new();
    let mut spec = "";
    let mut validator = golden_validator(GOLDEN[0].0);
    for &(case_spec, value, expected) in GOLDEN {
        if case_spec != spec {
            spec = case_spec;
            validator = golden_validator(spec);
        }
        let result = validator.validate(value);
        let got: Vec<(&str, Option<&str>, Option<&str>)> = result
            .failures
            .iter()
            .map(|f| {
                (
                    kind_name(&f.kind),
                    f.value.as_deref(),
                    f.description.as_deref(),
                )
            })
            .collect();
        if got != expected || result.is_valid() != expected.is_empty() {
            mismatches.push(format!(
                "{spec} {value:?}: expected {expected:?}, got {got:?}"
            ));
        }
    }
    assert!(
        mismatches.is_empty(),
        "{} of {} cases differ from Textual:\n{}",
        mismatches.len(),
        GOLDEN.len(),
        mismatches.join("\n")
    );
}

#[test]
fn goldens_cover_every_validator_and_outcome() {
    let kinds: std::collections::HashSet<&str> = GOLDEN
        .iter()
        .flat_map(|(_, _, f)| f.iter().map(|(k, _, _)| *k))
        .collect();
    for kind in [
        "Number.NotANumber",
        "Number.NotInRange",
        "Integer.NotAnInteger",
        "Length.Incorrect",
        "URL.InvalidURL",
        "Regex.NoResults",
    ] {
        assert!(kinds.contains(kind), "no golden case fails with {kind}");
    }
    assert!(GOLDEN.len() > 5000);
    assert!(GOLDEN.iter().filter(|(_, _, f)| f.is_empty()).count() > 500);
}

// ---------------------------------------------------------------------------------- results

#[test]
fn validation_result_success_failure_and_merge() {
    let ok = ValidationResult::success();
    assert!(ok.is_valid());
    assert!(ok.failure_descriptions().is_empty());
    assert_eq!(ok, ValidationResult::default());

    let len = Length::new().minimum(3);
    let num = Number::new();
    let merged = ValidationResult::merge([
        len.validate("ab"),
        ValidationResult::success(),
        num.validate("x"),
    ]);
    assert!(!merged.is_valid());
    assert_eq!(
        merged.failure_descriptions(),
        [
            "Must be longer than 3 characters.",
            "Must be a valid number."
        ]
    );
    assert_eq!(merged.failures[0].validator, "Length");
    assert_eq!(merged.failures[1].validator, "Number");

    assert!(ValidationResult::merge([]).is_valid());
    assert!(ValidationResult::merge([ok.clone(), ok]).is_valid());
}

#[test]
fn failure_descriptions_skip_failures_without_one() {
    let undescribed = Function::new(|_| false);
    let result = ValidationResult::merge([undescribed.validate("x"), Number::new().validate("x")]);
    assert_eq!(result.failures.len(), 2);
    assert_eq!(result.failures[0].description, None);
    assert_eq!(result.failure_descriptions(), ["Must be a valid number."]);
}

// ---------------------------------------------------------------------------------- Validator

struct Palindrome;

impl Validator for Palindrome {
    fn validate(&self, value: &str) -> ValidationResult {
        if value.chars().eq(value.chars().rev()) {
            self.success()
        } else {
            self.failure(Some("That's not a palindrome :/"), Some(value))
        }
    }
}

#[test]
fn custom_validator_with_explicit_description() {
    assert!(Palindrome.validate("racecar").is_valid());
    assert!(Palindrome.validate("").is_valid());
    let result = Palindrome.validate("rust");
    let f = &result.failures[0];
    assert_eq!(f.kind, FailureKind::Failure);
    assert_eq!(f.validator, "Palindrome");
    assert_eq!(f.value.as_deref(), Some("rust"));
    assert_eq!(f.description.as_deref(), Some("That's not a palindrome :/"));
}

/// A validator with fine-grained failures, described by `describe_failure`.
struct Password {
    failure_description: Option<String>,
}

impl Validator for Password {
    fn validate(&self, value: &str) -> ValidationResult {
        let mut failures = Vec::new();
        if value.chars().count() < 8 {
            failures.push(Failure::new(
                self,
                FailureKind::Custom("TooShort".into()),
                Some(value),
                None,
            ));
        }
        if !value.chars().any(|c| c.is_ascii_digit()) {
            failures.push(Failure::new(
                self,
                FailureKind::Custom("NoDigit".into()),
                Some(value),
                None,
            ));
        }
        if failures.is_empty() {
            self.success()
        } else {
            self.failures(failures)
        }
    }

    fn get_failure_description(&self) -> Option<&str> {
        self.failure_description.as_deref()
    }

    fn describe_failure(&self, failure: &Failure) -> Option<String> {
        match &failure.kind {
            FailureKind::Custom(k) if k == "TooShort" => Some("At least 8 characters.".into()),
            FailureKind::Custom(k) if k == "NoDigit" => Some("At least one digit.".into()),
            _ => None,
        }
    }
}

#[test]
fn description_precedence_follows_textual() {
    // 1. describe_failure when nothing else is given.
    let v = Password {
        failure_description: None,
    };
    assert_eq!(
        v.validate("abc").failure_descriptions(),
        ["At least 8 characters.", "At least one digit."]
    );
    assert_eq!(
        v.validate("abcdefgh").failure_descriptions(),
        ["At least one digit."]
    );
    assert!(v.validate("abcdefg1").is_valid());
    assert_eq!(
        v.validate("abc").failures[0].kind,
        FailureKind::Custom("TooShort".into())
    );

    // 2. The validator's failure description overrides describe_failure.
    let v = Password {
        failure_description: Some("Weak password".into()),
    };
    assert_eq!(
        v.validate("abc").failure_descriptions(),
        ["Weak password", "Weak password"]
    );

    // 3. A failure's own description overrides both.
    let f = Failure::new(&v, FailureKind::Failure, None, Some("Explicit"));
    assert_eq!(f.description.as_deref(), Some("Explicit"));
    assert_eq!(f.value, None);
}

#[test]
fn failure_helpers() {
    let v = Palindrome;
    // An empty failure list still fails, with one plain failure.
    let r = v.failures(vec![]);
    assert!(!r.is_valid());
    assert_eq!(r.failures.len(), 1);
    assert_eq!(r.failures[0].kind, FailureKind::Failure);
    assert_eq!(r.failures[0].description, None);
    // failure() without a description takes the validator's (none here).
    assert_eq!(v.failure(None, Some("v")).failures[0].description, None);
    assert_eq!(
        Number::new()
            .failure_description("custom")
            .failure(None, None)
            .failures[0]
            .description
            .as_deref(),
        Some("custom")
    );
}

#[test]
fn validators_work_as_trait_objects() {
    let validators: Vec<Box<dyn Validator>> = vec![
        Box::new(Number::new()),
        Box::new(Length::new().maximum(2)),
        Box::new(Palindrome),
    ];
    let result = ValidationResult::merge(validators.iter().map(|v| v.validate("123")));
    assert_eq!(
        result.failure_descriptions(),
        [
            "Must be shorter than 2 characters.",
            "That's not a palindrome :/"
        ]
    );
    let names: Vec<&str> = validators.iter().map(|v| v.name()).collect();
    assert_eq!(names, ["Number", "Length", "Palindrome"]);

    let shared: Rc<dyn Validator> = Rc::new(Integer::new());
    assert!(shared.validate("7").is_valid());
    assert_eq!(shared.name(), "Integer");
    let boxed: Box<dyn Validator> = Box::new(Url::new());
    assert_eq!(boxed.validate("x").failures[0].validator, "Url");
}

// ---------------------------------------------------------------------------------- validators

#[test]
fn function_validator() {
    let calls = Rc::new(Cell::new(0));
    let c = calls.clone();
    let even = Function::new(move |v| {
        c.set(c.get() + 1);
        v.parse::<i32>().is_ok_and(|n| n % 2 == 0)
    })
    .failure_description("Must be even.");
    assert!(even.validate("4").is_valid());
    let r = even.validate("3");
    assert_eq!(r.failures[0].kind, FailureKind::ReturnedFalse);
    assert_eq!(r.failures[0].value.as_deref(), Some("3"));
    assert_eq!(r.failure_descriptions(), ["Must be even."]);
    assert_eq!(calls.get(), 2);
    // Without a description, Textual's Function gives none.
    assert_eq!(
        Function::new(|_| false).validate("x").failures[0].description,
        None
    );
    assert!(format!("{even:?}").contains("Must be even."));
}

#[test]
fn number_and_integer_bounds_print_like_python() {
    let cases: [(Number, &str); 6] = [
        (
            Number::new().minimum(5),
            "Must be greater than or equal to 5.",
        ),
        (
            Number::new().minimum(5.0),
            "Must be greater than or equal to 5.0.",
        ),
        (
            Number::new().maximum(1e16),
            "Must be less than or equal to 1e+16.",
        ),
        (
            Number::new().maximum(0.1f32),
            "Must be less than or equal to 0.10000000149011612.",
        ),
        (
            Number::new().minimum(-0.0).maximum(-0.0),
            "Must be between -0.0 and -0.0.",
        ),
        (
            Number::new().minimum(1u8).maximum(2.5e-7),
            "Must be between 1 and 2.5e-07.",
        ),
    ];
    for (v, description) in cases {
        let result = ValidationResult::merge([v.validate("-1e300"), v.validate("1e300")]);
        assert_eq!(result.failure_descriptions()[0], description);
    }
    assert_eq!(Bound::from(3).to_string(), "3");
    assert_eq!(Bound::from(3.0).to_string(), "3.0");
    assert_eq!(Bound::from(3).as_f64(), 3.0);
    assert_eq!(
        Integer::new()
            .maximum(-5)
            .validate("0")
            .failure_descriptions(),
        ["Must be less than or equal to -5."]
    );
}

#[test]
fn integer_bounds_compare_exactly() {
    // 2^53 + 1 isn't a float: the comparison must not round it.
    let v = Integer::new().maximum(9_007_199_254_740_993i64);
    assert!(v.validate("9007199254740992").is_valid());
    let v = Number::new().minimum(9_007_199_254_740_993i64);
    assert!(!v.validate("9007199254740992").is_valid());
    let v = Number::new().minimum(i64::MIN).maximum(i64::MAX);
    assert!(v.validate("9.2e18").is_valid());
    assert!(!v.validate("9.3e18").is_valid());
    assert!(!v.validate("-9.3e18").is_valid());
}

#[test]
fn integer_reports_number_failures_first() {
    let v = Integer::new().minimum(0).maximum(10);
    assert_eq!(v.validate("x").failures[0].kind, FailureKind::NotANumber);
    assert_eq!(v.validate("11").failures[0].kind, FailureKind::NotInRange);
    // In range but not an integer.
    assert_eq!(
        v.validate("5.5").failures[0].kind,
        FailureKind::NotAnInteger
    );
    assert_eq!(
        v.validate("5e0").failures[0].kind,
        FailureKind::NotAnInteger
    );
    // Out of range and not an integer: the range failure wins (as in Textual).
    assert_eq!(v.validate("10.5").failures[0].kind, FailureKind::NotInRange);
    assert_eq!(v.validate("x").failures[0].validator, "Integer");
}

#[test]
fn regex_api() {
    let v = Regex::new(r"\d{4}").unwrap();
    assert_eq!(v.pattern(), r"\d{4}");
    assert_eq!(v.flags(), RegexFlags::NONE);
    assert!(v.is_match("2024"));
    assert!(!v.is_match("20245"));
    assert_eq!(
        v.validate("x").failure_descriptions(),
        ["Must match regular expression '\\\\d{4}' (flags=0)."]
    );
    let v = Regex::with_flags("[a-z]+", RegexFlags::IGNORECASE | RegexFlags::MULTILINE).unwrap();
    assert_eq!(
        v.validate("1").failure_descriptions(),
        ["Must match regular expression '[a-z]+' (flags=re.IGNORECASE|re.MULTILINE)."]
    );
    let v = Regex::new("x").unwrap().failure_description("Only x");
    assert_eq!(v.validate("y").failure_descriptions(), ["Only x"]);
    assert_eq!(v.validate("y").failures[0].kind, FailureKind::NoResults);

    for bad in ["(", "[a", r"(a)\1", "(?=a)", "a++", "\\"] {
        assert!(Regex::new(bad).is_err(), "{bad:?} should not compile");
    }
    assert!(Regex::with_flags("a", RegexFlags::ASCII | RegexFlags::UNICODE).is_err());
}

#[test]
fn regex_flags() {
    let mut f = RegexFlags::IGNORECASE;
    f |= RegexFlags::VERBOSE;
    assert!(f.contains(RegexFlags::IGNORECASE) && f.contains(RegexFlags::VERBOSE));
    assert!(!f.contains(RegexFlags::DOTALL));
    assert_eq!(f.bits(), 2 | 64);
    assert_eq!(f.to_string(), "re.IGNORECASE|re.VERBOSE");
    assert_eq!(RegexFlags::NONE.to_string(), "0");
    assert_eq!(RegexFlags::default(), RegexFlags::NONE);
    assert_eq!(
        (RegexFlags::ASCII | RegexFlags::DOTALL | RegexFlags::MULTILINE).to_string(),
        "re.MULTILINE|re.DOTALL|re.ASCII"
    );
}

#[test]
fn length_counts_characters() {
    let v = Length::new().minimum(2).maximum(3);
    assert!(v.validate("日本").is_valid()); // 2 characters, 6 bytes
    assert!(v.validate("😀😀😀").is_valid());
    assert!(!v.validate("e\u{301}e\u{301}").is_valid()); // 4 code points, as Python counts
}

#[test]
fn url_validator() {
    for ok in [
        "https://example.com",
        "ftp://x/y",
        "http://[::1]:8080/",
        "git+ssh://host",
    ] {
        assert!(Url::new().validate(ok).is_valid(), "{ok}");
    }
    for bad in [
        "example.com",
        "mailto:a@b.c",
        "http://",
        "/path",
        "http://[::1",
    ] {
        let r = Url::new().validate(bad);
        assert_eq!(r.failures[0].kind, FailureKind::InvalidUrl, "{bad}");
        assert_eq!(r.failure_descriptions(), ["Must be a valid URL."]);
    }
}

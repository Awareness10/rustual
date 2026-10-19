"""``rustual.validation``: the goldens Textual produced (``scripts/validation_golden.py``), and the
API around them (results, failures, description precedence, custom validators)."""

from __future__ import annotations

import json
import platform
import re
import warnings
from pathlib import Path

import pytest

from rustual import validation
from rustual.validation import (
    URL,
    Failure,
    Function,
    Integer,
    Length,
    Number,
    Regex,
    ValidationResult,
    Validator,
)

GOLDEN = json.loads((Path(__file__).parent / "data" / "validation_golden.json").read_text())


def test_matches_textual_on_every_golden_case() -> None:
    namespace = {name: getattr(validation, name) for name in validation.__all__} | {"re": re}
    validators = {spec: eval(ctor, namespace) for spec, ctor in GOLDEN["specs"].items()}
    # Like Textual's, `URL` uses this interpreter's urllib.parse, whose answers for edge cases
    # change between Python releases: compare URLs only on the Python the goldens came from.
    same_python = platform.python_version() == GOLDEN["python"]
    mismatches = []
    compared = 0
    for spec, value, expected in GOLDEN["cases"]:
        if spec.startswith("url") and not same_python:
            continue
        compared += 1
        with warnings.catch_warnings():
            warnings.simplefilter("ignore", FutureWarning)  # "Possible nested set" in a pattern
            result = validators[spec].validate(value)
        got = [[type(f).__qualname__, f.value, f.description] for f in result.failures]
        if got != expected or result.is_valid != (not expected):
            mismatches.append(f"{spec} {value!r}: expected {expected}, got {got}")
    assert not mismatches, f"{len(mismatches)} cases differ from Textual:\n" + "\n".join(mismatches[:50])
    assert compared > 3000


# -- ValidationResult ------------------------------------------------------------------------


def test_success_and_failure_results() -> None:
    assert ValidationResult.success().is_valid
    assert ValidationResult.success() == ValidationResult()
    assert ValidationResult.success().failure_descriptions == []
    failure = Failure(Number(), "x", "bad")
    result = ValidationResult.failure([failure])
    assert not result.is_valid
    assert result.failures == [failure]
    assert result.failure_descriptions == ["bad"]


def test_merge() -> None:
    a = Length(minimum=3).validate("ab")
    b = Number().validate("x")
    merged = ValidationResult.merge([a, ValidationResult.success(), b])
    assert not merged.is_valid
    assert merged.failure_descriptions == ["Must be longer than 3 characters.", "Must be a valid number."]
    assert merged.failures == [*a.failures, *b.failures]
    assert ValidationResult.merge([]).is_valid
    assert ValidationResult.merge([ValidationResult.success()] * 3).is_valid


def test_failure_descriptions_skip_missing() -> None:
    result = ValidationResult.merge([Function(lambda v: False).validate("x"), Number().validate("x")])
    assert len(result.failures) == 2
    assert result.failures[0].description is None
    assert result.failure_descriptions == ["Must be a valid number."]


# -- Validator ---------------------------------------------------------------------------------


class Palindrome(Validator):
    def validate(self, value: str) -> ValidationResult:
        if value == value[::-1]:
            return self.success()
        return self.failure("That's not a palindrome :/", value)


def test_custom_validator() -> None:
    assert Palindrome().validate("racecar").is_valid
    result = Palindrome().validate("rust")
    [failure] = result.failures
    assert type(failure) is Failure
    assert failure.validator.__class__ is Palindrome
    assert failure.value == "rust"
    assert failure.description == "That's not a palindrome :/"


def test_validator_is_abstract() -> None:
    with pytest.raises(TypeError):
        Validator()  # type: ignore[abstract]


class Password(Validator):
    class TooShort(Failure):
        pass

    class NoDigit(Failure):
        pass

    def validate(self, value: str) -> ValidationResult:
        failures: list[Failure] = []
        if len(value) < 8:
            failures.append(Password.TooShort(self, value))
        if not any(c.isdigit() for c in value):
            failures.append(Password.NoDigit(self, value))
        return self.failure(failures=failures) if failures else self.success()

    def describe_failure(self, failure: Failure) -> str | None:
        if isinstance(failure, Password.TooShort):
            return "At least 8 characters."
        if isinstance(failure, Password.NoDigit):
            return "At least one digit."
        return None


def test_description_precedence() -> None:
    # describe_failure when nothing else is given...
    assert Password().validate("abc").failure_descriptions == ["At least 8 characters.", "At least one digit."]
    assert Password().validate("abcdefg1").is_valid
    # ...the validator's failure_description overrides it...
    assert Password("Weak password").validate("abc").failure_descriptions == ["Weak password", "Weak password"]
    # ...and a failure's own description overrides both.
    assert Failure(Password("Weak password"), None, "Explicit").description == "Explicit"
    # The base describe_failure returns the failure_description.
    assert Palindrome("fallback").describe_failure(Failure(Palindrome(), None, "x")) == "fallback"


def test_failure_helper() -> None:
    v = Palindrome()
    # No failures given: a generic Failure.
    for failures in (None, []):
        result = v.failure(failures=failures)
        [f] = result.failures
        assert type(f) is Failure and f.value is None and f.description is None
    # A single Failure is wrapped in a list.
    single = Failure(v, "x", "one")
    assert v.failure(failures=single).failures == [single]
    # Without a description, the validator's failure_description is used.
    assert Number(failure_description="custom").failure().failure_descriptions == ["custom"]


# -- validators ------------------------------------------------------------------------------


def test_number_and_integer_failure_kinds() -> None:
    v = Integer(minimum=0, maximum=10)
    assert isinstance(v.validate("x").failures[0], Number.NotANumber)
    assert isinstance(v.validate("11").failures[0], Number.NotInRange)
    assert isinstance(v.validate("5.5").failures[0], Integer.NotAnInteger)
    assert isinstance(v.validate("10.5").failures[0], Number.NotInRange)
    assert Integer.NotANumber is Number.NotANumber
    assert v.validate("x").failure_descriptions == ["Must be a valid integer."]
    assert v.validate("x").failures[0].validator is v


def test_number_bounds_print_like_python() -> None:
    assert Number(minimum=5).validate("1").failure_descriptions == ["Must be greater than or equal to 5."]
    assert Number(minimum=5.0).validate("1").failure_descriptions == ["Must be greater than or equal to 5.0."]
    assert Number(maximum=1e16).validate("1e17").failure_descriptions == ["Must be less than or equal to 1e+16."]


def test_length() -> None:
    assert Length(minimum=2, maximum=3).validate("日本").is_valid
    assert not Length(minimum=2, maximum=3).validate("éé").is_valid  # 4 code points
    assert isinstance(Length(maximum=0).validate("x").failures[0], Length.Incorrect)
    assert Length().validate("anything").is_valid


def test_function() -> None:
    calls = []

    def even(value: str) -> bool:
        calls.append(value)
        return value.isdigit() and int(value) % 2 == 0

    v = Function(even, "Must be even.")
    assert v.validate("4").is_valid
    result = v.validate("3")
    assert isinstance(result.failures[0], Function.ReturnedFalse)
    assert result.failure_descriptions == ["Must be even."]
    assert calls == ["4", "3"]
    assert Function(lambda v: False).validate("x").failures[0].description is None


def test_regex() -> None:
    v = Regex(r"\d{4}")
    assert v.validate("2024").is_valid
    assert not v.validate("20245").is_valid
    assert isinstance(v.validate("x").failures[0], Regex.NoResults)
    assert v.validate("x").failure_descriptions == ["Must match regular expression '\\\\d{4}' (flags=0)."]
    # A compiled pattern (Textual passes the flags too, so only flags=0 works with one).
    compiled = Regex(re.compile("[a-z]+", re.IGNORECASE))
    assert compiled.validate("ABC").is_valid
    assert compiled.validate("1").failure_descriptions == [
        "Must match regular expression re.compile('[a-z]+', re.IGNORECASE) (flags=0)."
    ]
    flags = Regex("[a-z]+", flags=re.IGNORECASE | re.MULTILINE)
    assert flags.validate("1").failure_descriptions == [
        "Must match regular expression '[a-z]+' (flags=re.IGNORECASE|re.MULTILINE)."
    ]
    # Python's re: look-around and backreferences work here.
    assert Regex(r"(a)\1(?=b)b").validate("aab").is_valid


def test_url() -> None:
    for ok in ["https://example.com", "ftp://x/y", "http://[::1]:8080/"]:
        assert URL().validate(ok).is_valid
    for bad in ["example.com", "mailto:a@b.c", "http://", "http://[::1"]:
        result = URL().validate(bad)
        assert isinstance(result.failures[0], URL.InvalidURL)
        assert result.failure_descriptions == ["Must be a valid URL."]


def test_exports() -> None:
    assert set(validation.__all__) == {
        "URL",
        "Failure",
        "Function",
        "Integer",
        "Length",
        "Number",
        "Regex",
        "ValidationResult",
        "Validator",
    }

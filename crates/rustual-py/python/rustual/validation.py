"""Input validation (Textual's ``textual.validation``)."""

from __future__ import annotations

import math
import re
from abc import ABC, abstractmethod
from dataclasses import dataclass, field
from typing import Any, Callable, Pattern, Sequence
from urllib.parse import urlparse


@dataclass
class ValidationResult:
    failures: Sequence[Failure] = field(default_factory=list)

    @staticmethod
    def merge(results: Sequence[ValidationResult]) -> ValidationResult:
        failures = [f for r in results for f in r.failures]
        return ValidationResult.failure(failures) if failures else ValidationResult.success()

    @staticmethod
    def success() -> ValidationResult:
        return ValidationResult()

    @staticmethod
    def failure(failures: Sequence[Failure]) -> ValidationResult:
        return ValidationResult(failures)

    @property
    def failure_descriptions(self) -> list[str]:
        return [f.description for f in self.failures if f.description is not None]

    @property
    def is_valid(self) -> bool:
        return not self.failures


@dataclass
class Failure:
    validator: Validator
    value: str | None = None
    description: str | None = None

    def __post_init__(self) -> None:
        if self.description is None:
            if self.validator.failure_description is not None:
                self.description = self.validator.failure_description
            else:
                self.description = self.validator.describe_failure(self)


class Validator(ABC):
    def __init__(self, failure_description: str | None = None) -> None:
        self.failure_description = failure_description

    @abstractmethod
    def validate(self, value: str) -> ValidationResult: ...

    def describe_failure(self, failure: Failure) -> str | None:
        return None

    def success(self) -> ValidationResult:
        return ValidationResult.success()

    def failure(self, description: str | None = None, value: str | None = None, failures: Any = None) -> ValidationResult:
        if failures is None:
            return ValidationResult.failure([Failure(self, value, description)])
        if isinstance(failures, Failure):
            failures = [failures]
        return ValidationResult.failure(list(failures))


class Regex(Validator):
    def __init__(self, regex: str | Pattern[str], flags: int | re.RegexFlag = 0, failure_description: str | None = None) -> None:
        super().__init__(failure_description)
        self.regex = regex
        self.flags = flags

    def validate(self, value: str) -> ValidationResult:
        regex = self.regex
        if (re.fullmatch(regex, value, flags=self.flags) if isinstance(regex, str) else regex.fullmatch(value)) is None:
            return ValidationResult.failure([Regex._NoResults(self, value)])
        return self.success()

    class _NoResults(Failure):
        pass

    def describe_failure(self, failure: Failure) -> str | None:
        return f"Must match regular expression {self.regex!r} (flags={self.flags})."


class Number(Validator):
    def __init__(self, minimum: float | None = None, maximum: float | None = None, failure_description: str | None = None) -> None:
        super().__init__(failure_description)
        self.minimum = minimum
        self.maximum = maximum

    class NotANumber(Failure):
        pass

    class NotInRange(Failure):
        pass

    def validate(self, value: str) -> ValidationResult:
        try:
            float_value = float(value)
        except ValueError:
            return ValidationResult.failure([Number.NotANumber(self, value)])
        if math.isnan(float_value) or math.isinf(float_value):
            return ValidationResult.failure([Number.NotANumber(self, value)])
        if not self._validate_range(float_value):
            return ValidationResult.failure([Number.NotInRange(self, value)])
        return self.success()

    def _validate_range(self, value: float) -> bool:
        if self.minimum is not None and value < self.minimum:
            return False
        if self.maximum is not None and value > self.maximum:
            return False
        return True

    def describe_failure(self, failure: Failure) -> str | None:
        if isinstance(failure, Number.NotANumber):
            return "Must be a valid number."
        if isinstance(failure, Number.NotInRange):
            if self.minimum is None and self.maximum is not None:
                return f"Must be less than or equal to {self.maximum}."
            if self.minimum is not None and self.maximum is None:
                return f"Must be greater than or equal to {self.minimum}."
            return f"Must be between {self.minimum} and {self.maximum}."
        return None


class Integer(Number):
    class NotAnInteger(Failure):
        pass

    def validate(self, value: str) -> ValidationResult:
        result = super().validate(value)
        if not result.is_valid:
            return result
        try:
            int(value)
        except ValueError:
            return ValidationResult.failure([Integer.NotAnInteger(self, value)])
        return self.success()

    def describe_failure(self, failure: Failure) -> str | None:
        if isinstance(failure, (Integer.NotAnInteger, Number.NotANumber)):
            return "Must be a valid integer."
        return super().describe_failure(failure)


class Length(Validator):
    def __init__(self, minimum: int | None = None, maximum: int | None = None, failure_description: str | None = None) -> None:
        super().__init__(failure_description)
        self.minimum = minimum
        self.maximum = maximum

    class Incorrect(Failure):
        pass

    def validate(self, value: str) -> ValidationResult:
        too_short = self.minimum is not None and len(value) < self.minimum
        too_long = self.maximum is not None and len(value) > self.maximum
        if too_short or too_long:
            return ValidationResult.failure([Length.Incorrect(self, value)])
        return self.success()

    def describe_failure(self, failure: Failure) -> str | None:
        if self.minimum is None and self.maximum is not None:
            return f"Must be shorter than {self.maximum} characters."
        if self.minimum is not None and self.maximum is None:
            return f"Must be longer than {self.minimum} characters."
        return f"Must be between {self.minimum} and {self.maximum} characters."


class Function(Validator):
    def __init__(self, function: Callable[[str], bool], failure_description: str | None = None) -> None:
        super().__init__(failure_description)
        self.function = function

    class ReturnedFalse(Failure):
        pass

    def validate(self, value: str) -> ValidationResult:
        if self.function(value):
            return self.success()
        return ValidationResult.failure([Function.ReturnedFalse(self, value)])

    def describe_failure(self, failure: Failure) -> str | None:
        return self.failure_description


class URL(Validator):
    class InvalidURL(Failure):
        pass

    def validate(self, value: str) -> ValidationResult:
        try:
            parsed = urlparse(value)
            if not all([parsed.scheme, parsed.netloc]):
                return ValidationResult.failure([URL.InvalidURL(self, value)])
        except ValueError:
            return ValidationResult.failure([URL.InvalidURL(self, value)])
        return self.success()

    def describe_failure(self, failure: Failure) -> str | None:
        return "Must be a valid URL."

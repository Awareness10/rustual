from __future__ import annotations

import re
from typing import Any, ClassVar, Literal

from .._bridge import _native
from ..message import Message
from ..validation import ValidationResult, Validator
from ..widget import _NATIVE_CLASSES, Widget

InputType = Literal["integer", "number", "text"]
InputValidationOn = Literal["blur", "changed", "submitted"]
"""Possible messages that trigger input validation."""
_POSSIBLE_VALIDATE_ON_VALUES = {"blur", "changed", "submitted"}
"""Set literal with the legal values for the type `InputValidationOn`."""
_INPUT_TYPES = ("integer", "number", "text")


class Input(Widget):
    """A single-line text input."""

    _rs_kind: ClassVar[str] = "Input"
    _rs_framework: ClassVar[bool] = True

    class _InputMessage(Message):
        def __init__(
            self, input: Input | None = None, value: str = "", validation_result: ValidationResult | None = None
        ) -> None:
            super().__init__()
            self._input = input
            self.value = value
            self.validation_result = validation_result

        @property
        def input(self) -> Input:
            return self._input if self._input is not None else self.sender  # type: ignore[return-value]

        @property
        def control(self) -> Input:
            return self.input

    class Changed(_InputMessage):
        """Posted when the value changes (``on_input_changed``)."""

    class Submitted(_InputMessage):
        """Posted when enter is pressed (``on_input_submitted``)."""

    class Blurred(_InputMessage):
        """Posted when the input loses focus (``on_input_blurred``)."""

    def __init__(
        self,
        value: str | None = None,
        placeholder: str = "",
        highlighter: Any = None,
        password: bool = False,
        *,
        restrict: str | None = None,
        type: InputType = "text",
        max_length: int = 0,
        suggester: Any = None,
        validators: Any = None,
        validate_on: Any = None,
        valid_empty: bool = False,
        select_on_focus: bool = True,
        name: str | None = None,
        id: str | None = None,
        classes: str | None = None,
        disabled: bool = False,
        tooltip: str | None = None,
        compact: bool = False,
    ) -> None:
        super().__init__(name=name, id=id, classes=classes, disabled=disabled)
        if type not in _INPUT_TYPES:
            raise ValueError(f"Input type must be one of 'integer', 'number', or 'text'; not {type!r}")
        # Ensure we always end up with a list of validators (Textual's `Input.__init__`).
        if isinstance(validators, Validator):
            self.validators: list[Validator] = [validators]
        elif validators is None:
            self.validators = []
        else:
            self.validators = list(validators)
        self.validate_on: set[str] = (
            (_POSSIBLE_VALIDATE_ON_VALUES & set(validate_on))
            if validate_on is not None
            else _POSSIBLE_VALIDATE_ON_VALUES.copy()
        )
        """Set with event names to do input validation on."""
        self._valid_empty = valid_empty
        self._valid = True
        self.restrict = restrict
        self.type = type
        if not self.validators:
            from ..validation import Integer, Number

            if type == "integer":
                self.validators.append(Integer())
            elif type == "number":
                self.validators.append(Number())
        # Validation runs here, in Python (validators are Python objects): the native input
        # never validates by itself, it only posts the messages.
        self._rs_kw = {
            "value": value or "",
            "restrict": re.compile(restrict).fullmatch if restrict else None,
            "placeholder": placeholder,
            "password": password,
            "type": type,
            "max_length": max_length,
            "select_on_focus": select_on_focus,
            "compact": compact,
            "validate_on": [],
            "valid_empty": valid_empty,
        }
        if tooltip is not None:
            self.tooltip = tooltip

    @property
    def value(self) -> str:
        return self._native_get("value", "")

    @value.setter
    def value(self, value: str) -> None:
        self._native_set("value", str(value))

    @property
    def placeholder(self) -> str:
        return self._native_get("placeholder", "")

    @placeholder.setter
    def placeholder(self, value: str) -> None:
        self._native_set("placeholder", value)

    @property
    def password(self) -> bool:
        return self._native_get("password", False)

    @password.setter
    def password(self, value: bool) -> None:
        self._native_set("password", bool(value))

    @property
    def cursor_position(self) -> int:
        return self._native_get("cursor_position", len(self.value))

    @cursor_position.setter
    def cursor_position(self, value: int) -> None:
        self._native_set("cursor_position", int(value))

    @property
    def selection(self) -> tuple[int, int]:
        return tuple(self._native_get("selection", (0, 0)))  # type: ignore[return-value]

    @selection.setter
    def selection(self, value: tuple[int, int]) -> None:
        self._native_set("selection", tuple(value))

    @property
    def selected_text(self) -> str:
        return self._native_get("selected_text", "")

    @property
    def is_valid(self) -> bool:
        """Check if the value has passed validation."""
        return self._valid

    @property
    def valid_empty(self) -> bool:
        """Empty values should pass validation."""
        return self._valid_empty

    @valid_empty.setter
    def valid_empty(self, valid_empty: bool) -> None:
        # Like Textual's `_watch_valid_empty`: a change re-validates and posts `Changed`.
        valid_empty = bool(valid_empty)
        if valid_empty != self._valid_empty:
            self._valid_empty = valid_empty
            self._native_set("valid_empty", valid_empty)

    def clear(self) -> None:
        if self.is_mounted:
            self._native_call("clear")
        else:
            self._rs_kw["value"] = ""

    def insert_text_at_cursor(self, text: str) -> None:
        self._native_call("insert_text_at_cursor", text)

    def insert(self, text: str, index: int) -> None:
        self._native_call("insert", text, index)

    def delete(self, start: int, end: int) -> None:
        self._native_call("delete", start, end)

    def replace(self, text: str, start: int, end: int) -> None:
        self._native_call("replace", text, start, end)

    def select_all(self) -> None:
        self._native_call("select_all")

    def validate(self, value: str | None = None) -> ValidationResult | None:
        """Run all the validators associated with this Input on the supplied value.

        Runs all validators, combines the result into one. If any of the validators
        failed, the combined result will be a failure. If no validators are present,
        None will be returned. This also sets the `-invalid` CSS class on the Input
        if the validation fails, and sets the `-valid` CSS class on the Input if
        the validation succeeds. The value defaults to the current value.

        Returns:
            A ValidationResult indicating whether *all* validators succeeded or not.
                That is, if *any* validator fails, the result will be an unsuccessful
                validation.
        """
        if value is None:
            value = self.value

        def set_classes() -> None:
            """Set classes for valid flag."""
            valid = self._valid
            self.set_class(not valid, "-invalid")
            self.set_class(valid, "-valid")

        # If no validators are supplied, and therefore no validation occurs, we return None.
        if not self.validators:
            self._valid = True
            set_classes()
            return None

        if self.valid_empty and not value:
            self._valid = True
            set_classes()
            return None

        validation_results: list[ValidationResult] = [validator.validate(value) for validator in self.validators]
        combined_result = ValidationResult.merge(validation_results)
        self._valid = combined_result.is_valid
        set_classes()

        return combined_result

    def _validate_message(self, event: Input._InputMessage, trigger: str) -> None:
        """Validate (if `trigger` is in `validate_on`) before the message reaches other handlers."""
        if event.__dict__.get("_rs_validated"):
            return
        event._rs_validated = True  # type: ignore[attr-defined]
        if trigger in self.validate_on:
            event.validation_result = self.validate(event.value)

    def _on_input_changed(self, event: Input.Changed) -> None:
        self._validate_message(event, "changed")

    def _on_input_submitted(self, event: Input.Submitted) -> None:
        self._validate_message(event, "submitted")

    def _on_input_blurred(self, event: Input.Blurred) -> None:
        self._validate_message(event, "blur")

    async def action_submit(self) -> None:
        self._native_call("action_submit")


_native.register_class("Input.Changed", lambda **kw: Input.Changed(None, **kw))
_native.register_class("Input.Submitted", lambda **kw: Input.Submitted(None, **kw))
_native.register_class("Input.Blurred", lambda **kw: Input.Blurred(None, **kw))
_NATIVE_CLASSES["Input"] = Input

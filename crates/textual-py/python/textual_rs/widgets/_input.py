from __future__ import annotations

import re
from typing import Any, ClassVar, Iterable, Literal

from .._bridge import _native
from ..message import Message
from ..validation import ValidationResult, Validator
from ..widget import Widget, _NATIVE_CLASSES

InputType = Literal["integer", "number", "text"]


class Input(Widget):
    """A single-line text input."""

    _rs_kind: ClassVar[str] = "Input"
    _rs_framework: ClassVar[bool] = True

    class _InputMessage(Message):
        def __init__(self, input: Input | None = None, value: str = "", validation_result: ValidationResult | None = None) -> None:
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
        if isinstance(validators, Validator):
            validators = [validators]
        self.validators: list[Validator] = list(validators or [])
        self.validate_on = set(validate_on) if validate_on is not None else {"blur", "changed", "submitted"}
        self.valid_empty = valid_empty
        self.restrict = restrict
        self._rs_kw = {
            "value": value or "",
            "restrict": re.compile(restrict).fullmatch if restrict else None,
            "placeholder": placeholder,
            "password": password,
            "type": type,
            "max_length": max_length,
            "select_on_focus": select_on_focus,
            "compact": compact,
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
        return self._native_get("is_valid", True)

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
        """Run the validators (or the built-in type validation) and set ``-valid``/``-invalid``."""
        if value is None:
            value = self.value
        if not self.validators:
            if self.is_mounted:
                ok = self._native_call("validate")
                return ValidationResult.success() if ok else ValidationResult.failure([])
            return None
        if self.valid_empty and value == "":
            result = ValidationResult.success()
        else:
            result = ValidationResult.merge([v.validate(value) for v in self.validators])
        self.set_class(not result.is_valid, "-invalid")
        self.set_class(result.is_valid, "-valid")
        return result

    def _validate_message(self, event: Input._InputMessage, trigger: str) -> None:
        if self.validators and trigger in self.validate_on and event.validation_result is None:
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

from __future__ import annotations

from collections.abc import Iterable
from typing import Any, ClassVar

from ..validation import ValidationResult, Validator
from ..widget import _NATIVE_CLASSES
from ._input import Input, InputValidationOn


class _Template(Validator):
    """The template mask as a validator (Textual's ``_Template.validate``; the template itself
    is parsed and checked natively)."""

    def __init__(self, input: MaskedInput) -> None:
        super().__init__()
        self.input = input

    def validate(self, value: str) -> ValidationResult:
        """Checks if `value` matches this template, always returning a ValidationResult."""
        if self.check(value):
            return self.success()
        else:
            return self.failure("Value does not match template!", value)

    def check(self, value: str) -> bool:
        """Whether every required position of the template is filled (characters past the
        template are ignored). Before the input is mounted the template can't be checked."""
        if not self.input.is_mounted:
            return True
        return bool(self.input._native_call("matches_template", value))


class MaskedInput(Input):
    """A text input that follows a template mask (e.g. ``"9999-9999-9999-9999;0"``).

    Posts the ``Input`` messages (``Input.Changed``, ``Input.Submitted``, ``Input.Blurred``).
    """

    _rs_kind: ClassVar[str] = "MaskedInput"
    _rs_framework: ClassVar[bool] = True

    def __init__(
        self,
        template: str,
        value: str | None = None,
        placeholder: str = "",
        *,
        validators: Any = None,
        validate_on: Iterable[InputValidationOn] | None = None,
        valid_empty: bool = False,
        select_on_focus: bool = True,
        name: str | None = None,
        id: str | None = None,
        classes: str | None = None,
        disabled: bool = False,
        tooltip: str | None = None,
        compact: bool = False,
    ) -> None:
        super().__init__(
            placeholder=placeholder,
            validators=validators,
            validate_on=validate_on,
            valid_empty=valid_empty,
            select_on_focus=select_on_focus,
            name=name,
            id=id,
            classes=classes,
            disabled=disabled,
            tooltip=tooltip,
            compact=compact,
        )
        self._template = _Template(self)
        self._rs_kw = {
            "template": template,
            "value": value or "",
            "placeholder": placeholder,
            "valid_empty": valid_empty,
            "select_on_focus": select_on_focus,
            "compact": compact,
            "validate_on": [],
        }

    @property
    def template(self) -> str:
        """The template mask currently in use."""
        return self._native_get("template", "")

    @template.setter
    def template(self, template: str) -> None:
        self._native_set("template", template)

    @property
    def value(self) -> str:
        return self._native_get("value", "")

    @value.setter
    def value(self, value: str) -> None:
        """Set the value; raises ``ValueError`` if it doesn't match the template."""
        self._native_set("value", str(value))

    def clear(self) -> None:
        """Clear the input (separators at the start are kept)."""
        if self.is_mounted:
            self._native_call("clear")
        else:
            self._rs_kw["value"] = ""

    def validate(self, value: str | None = None) -> ValidationResult | None:
        """Run all the validators associated with this MaskedInput on the supplied value.

        Same as `Input.validate()` but also validates against template which acts as an
        additional implicit validator.

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

        result = super().validate(value)
        validation_results: list[ValidationResult] = [self._template.validate(value)]
        if result is not None:
            validation_results.append(result)
        combined_result = ValidationResult.merge(validation_results)
        self._valid = combined_result.is_valid
        set_classes()

        return combined_result

    def action_cursor_left(self) -> None:
        self._native_call("run_action", "cursor_left")

    def action_cursor_right(self) -> None:
        self._native_call("run_action", "cursor_right")

    def action_home(self) -> None:
        self._native_call("run_action", "home")

    def action_end(self) -> None:
        self._native_call("run_action", "end")

    def action_delete_left(self) -> None:
        self._native_call("run_action", "delete_left")

    def action_delete_right(self) -> None:
        self._native_call("run_action", "delete_right")

    async def action_submit(self) -> None:
        self._native_call("run_action", "submit")


_NATIVE_CLASSES["MaskedInput"] = MaskedInput

from __future__ import annotations

from typing import Any, ClassVar, Iterable, Literal

from ..validation import ValidationResult, Validator
from ..widget import _NATIVE_CLASSES
from ._input import Input

InputValidationOn = Literal["blur", "changed", "submitted"]


class _Template(Validator):
    """The template check as a validator (the check itself runs natively)."""

    def validate(self, value: str) -> ValidationResult:
        return self.success()


_TEMPLATE = _Template()


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
        self._rs_kw = {
            "template": template,
            "value": value or "",
            "placeholder": placeholder,
            "valid_empty": valid_empty,
            "select_on_focus": select_on_focus,
            "compact": compact,
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

    @property
    def is_valid(self) -> bool:
        return self._native_get("is_valid", True)

    def clear(self) -> None:
        """Clear the input (separators at the start are kept)."""
        if self.is_mounted:
            self._native_call("clear")
        else:
            self._rs_kw["value"] = ""

    def validate(self, value: str | None = None) -> ValidationResult | None:
        """Validate against the template (an implicit validator) and any ``validators``."""
        if value is None or value == self.value:
            ok = bool(self._native_call("validate")) if self.is_mounted else True
        else:
            ok = bool(self._native_call("matches_template", value)) or (self.valid_empty and value == "")
        results = [ValidationResult.success() if ok else _TEMPLATE.failure("Value does not match template!", value)]
        if self.validators:
            v = self.value if value is None else value
            if not (self.valid_empty and v == ""):
                results.extend(validator.validate(v) for validator in self.validators)
        result = ValidationResult.merge(results)
        self.set_class(not result.is_valid, "-invalid")
        self.set_class(result.is_valid, "-valid")
        return result

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

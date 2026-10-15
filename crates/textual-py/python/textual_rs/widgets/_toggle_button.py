from __future__ import annotations

from typing import Any, ClassVar

from .._bridge import _native
from ..message import Message
from ..widget import _NATIVE_CLASSES
from ._helpers import compact_property, prompt_markup
from ._static import Static


class ToggleButton(Static):
    """Base class for a toggle-style button (``Checkbox``, ``RadioButton``)."""

    _rs_kind: ClassVar[str] = "ToggleButton"
    _rs_framework: ClassVar[bool] = True

    BUTTON_LEFT: str = "▐"
    BUTTON_INNER: str = "X"
    BUTTON_RIGHT: str = "▌"

    class Changed(Message):
        """Posted when the value of the toggle button changes."""

        def __init__(self, toggle_button: ToggleButton | None = None, value: bool = False) -> None:
            super().__init__()
            self._toggle_button = toggle_button
            self.value = value

        @property
        def toggle_button(self) -> ToggleButton:
            return self._toggle_button if self._toggle_button is not None else self.sender  # type: ignore[return-value]

        @property
        def control(self) -> ToggleButton:
            return self.toggle_button

        def __repr__(self) -> str:
            return f"{type(self).__qualname__}(value={self.value!r})"

    def __init__(
        self,
        label: Any = "",
        value: bool = False,
        button_first: bool = True,
        *,
        name: str | None = None,
        id: str | None = None,
        classes: str | None = None,
        disabled: bool = False,
        tooltip: str | None = None,
        compact: bool = False,
    ) -> None:
        Static.__init__(self, name=name, id=id, classes=classes, disabled=disabled)
        self._button_first = button_first
        self._rs_kw = {
            "label": prompt_markup(label),
            "value": bool(value),
            "button_first": button_first,
            "compact": compact,
        }
        self.compact = compact
        if tooltip is not None:
            self.tooltip = tooltip

    compact = compact_property()

    @property
    def value(self) -> bool:
        """The value of the button: ``True`` for on, ``False`` for off."""
        return bool(self._native_get("value", False))

    @value.setter
    def value(self, value: bool) -> None:
        self._native_set("value", bool(value))

    @property
    def label(self) -> str:
        """The label (markup) of the button."""
        return self._native_get("label", "")

    @label.setter
    def label(self, label: Any) -> None:
        self._native_set("label", prompt_markup(label))

    @property
    def content(self) -> Any:
        return self.label

    def toggle(self) -> ToggleButton:
        """Toggle the value."""
        self.value = not self.value
        return self

    def action_toggle_button(self) -> None:
        self.toggle()


_native.register_class("ToggleButton.Changed", lambda **kw: ToggleButton.Changed(None, **kw))
_NATIVE_CLASSES["ToggleButton"] = ToggleButton

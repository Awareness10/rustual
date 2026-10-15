from __future__ import annotations

from typing import Any, ClassVar, Literal

from .._bridge import _native
from ..content import to_markup
from ..message import Message
from ..widget import Widget, _NATIVE_CLASSES

ButtonVariant = Literal["default", "primary", "success", "warning", "error"]


class Button(Widget):
    """A clickable button; posts :class:`Button.Pressed` when pressed."""

    _rs_kind: ClassVar[str] = "Button"
    _rs_framework: ClassVar[bool] = True

    class Pressed(Message):
        """Posted when a button is pressed (``on_button_pressed``)."""

        def __init__(self, button: Button | None = None) -> None:
            super().__init__()
            self._button = button

        @property
        def button(self) -> Button:
            return self._button if self._button is not None else self.sender  # type: ignore[return-value]

        @property
        def control(self) -> Button:
            return self.button

    def __init__(
        self,
        label: Any = None,
        variant: ButtonVariant = "default",
        *,
        name: str | None = None,
        id: str | None = None,
        classes: str | None = None,
        disabled: bool = False,
        tooltip: str | None = None,
        action: str | None = None,
        compact: bool = False,
        flat: bool = False,
    ) -> None:
        super().__init__(name=name, id=id, classes=classes, disabled=disabled)
        text, _ = to_markup(label if label is not None else "Button")
        self._rs_kw = {"label": text, "variant": variant, "action": action, "flat": flat}
        if compact:
            self.add_class("-textual-compact")
        if tooltip is not None:
            self.tooltip = tooltip

    @classmethod
    def success(cls, label: Any = None, **kwargs: Any) -> Button:
        return cls(label, "success", **kwargs)

    @classmethod
    def warning(cls, label: Any = None, **kwargs: Any) -> Button:
        return cls(label, "warning", **kwargs)

    @classmethod
    def error(cls, label: Any = None, **kwargs: Any) -> Button:
        return cls(label, "error", **kwargs)

    @property
    def label(self) -> str:
        return self._native_get("label", "")

    @label.setter
    def label(self, value: Any) -> None:
        self._native_set("label", to_markup(value)[0])

    @property
    def variant(self) -> str:
        return self._native_get("variant", "default")

    @variant.setter
    def variant(self, value: str) -> None:
        self._native_set("variant", value)

    def press(self) -> Button:
        """Press the button (posts :class:`Button.Pressed`)."""
        if self.is_mounted:
            self._native_call("press")
        return self

    def action_press(self) -> None:
        self.press()


_native.register_class("Button.Pressed", lambda **kw: Button.Pressed())
_NATIVE_CLASSES["Button"] = Button

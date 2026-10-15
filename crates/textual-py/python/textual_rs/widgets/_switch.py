from __future__ import annotations

from typing import ClassVar

from .._bridge import _native
from ..message import Message
from ..widget import Widget, _NATIVE_CLASSES


class Switch(Widget):
    """A switch widget that represents a boolean value; toggle with enter, space or a click."""

    _rs_kind: ClassVar[str] = "Switch"
    _rs_framework: ClassVar[bool] = True

    class Changed(Message):
        """Posted when the status of the switch changes (``on_switch_changed``)."""

        def __init__(self, switch: Switch | None = None, value: bool = False) -> None:
            super().__init__()
            self._switch = switch
            self.value: bool = value

        @property
        def switch(self) -> Switch:
            return self._switch if self._switch is not None else self.sender  # type: ignore[return-value]

        @property
        def control(self) -> Switch:
            return self.switch

        def __repr__(self) -> str:
            return f"Switch.Changed(value={self.value!r})"

    def __init__(
        self,
        value: bool = False,
        *,
        animate: bool = True,
        name: str | None = None,
        id: str | None = None,
        classes: str | None = None,
        disabled: bool = False,
        tooltip: str | None = None,
    ) -> None:
        super().__init__(name=name, id=id, classes=classes, disabled=disabled)
        self._rs_kw = {"value": bool(value), "animate": animate}
        if tooltip is not None:
            self.tooltip = tooltip

    @property
    def value(self) -> bool:
        """The value of the switch; ``True`` for on and ``False`` for off."""
        return bool(self._native_get("value", False))

    @value.setter
    def value(self, value: bool) -> None:
        self._native_set("value", bool(value))

    @property
    def _slider_position(self) -> float:
        return float(self._native_get("slider_pos", 1.0 if self.value else 0.0))

    def toggle(self) -> Switch:
        """Toggle the switch value."""
        self.value = not self.value
        return self

    def action_toggle_switch(self) -> None:
        self.toggle()


_native.register_class("Switch.Changed", lambda **kw: Switch.Changed(None, **kw))
_NATIVE_CLASSES["Switch"] = Switch

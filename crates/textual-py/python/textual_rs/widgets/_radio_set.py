from __future__ import annotations

from typing import Any, ClassVar

from .._bridge import _native
from ..containers import VerticalScroll
from ..message import Message
from ..widget import _NATIVE_CLASSES, _widget_for
from ._helpers import compact_property
from ._radio_button import RadioButton


class RadioSet(VerticalScroll):  # type: ignore[misc, valid-type]
    """A container of radio buttons where at most one can be pressed.

    The set takes focus itself; arrow keys move a highlight between the buttons and
    enter / space press the highlighted one.
    """

    _rs_kind: ClassVar[str] = "RadioSet"
    _rs_framework: ClassVar[bool] = True
    can_focus: bool = True
    can_focus_children: bool = False

    class Changed(Message):
        """Posted when the pressed button in the set changes (``on_radio_set_changed``)."""

        ALLOW_SELECTOR_MATCH = {"pressed"}

        def __init__(
            self,
            radio_set: RadioSet | None = None,
            pressed: RadioButton | int | None = None,
            index: int = -1,
        ) -> None:
            super().__init__()
            self._radio_set = radio_set
            self._pressed = pressed
            self.index: int = index

        @property
        def radio_set(self) -> RadioSet:
            return self._radio_set if self._radio_set is not None else self.sender  # type: ignore[return-value]

        @property
        def pressed(self) -> RadioButton:
            """The radio button that was pressed to make the change."""
            if isinstance(self._pressed, int):
                self._pressed = _widget_for(self._pressed)
            return self._pressed  # type: ignore[return-value]

        @property
        def control(self) -> RadioSet:
            return self.radio_set

        def __repr__(self) -> str:
            return f"RadioSet.Changed(index={self.index!r})"

    def __init__(
        self,
        *buttons: str | RadioButton,
        name: str | None = None,
        id: str | None = None,
        classes: str | None = None,
        disabled: bool = False,
        tooltip: str | None = None,
        compact: bool = False,
    ) -> None:
        super().__init__(
            *[button if isinstance(button, RadioButton) else RadioButton(button) for button in buttons],
            name=name,
            id=id,
            classes=classes,
            disabled=disabled,
        )
        self._rs_kw = {"compact": compact}
        self.compact = compact
        if tooltip is not None:
            self.tooltip = tooltip

    compact = compact_property()

    @property
    def pressed_button(self) -> RadioButton | None:
        """The currently pressed radio button, or ``None`` if none is pressed."""
        if not self.is_mounted:
            return next((b for b in self._pending_children if isinstance(b, RadioButton) and b.value), None)
        node = self._native_get("pressed_button")
        return None if node is None else _widget_for(node)

    @property
    def pressed_index(self) -> int:
        """The index of the currently pressed radio button, or -1 if none is pressed."""
        if not self.is_mounted:
            buttons = [b for b in self._pending_children if isinstance(b, RadioButton)]
            return next((i for i, b in enumerate(buttons) if b.value), -1)
        index = self._native_get("pressed_index")
        return -1 if index is None else index

    @property
    def _selected(self) -> int | None:
        """The index of the highlighted (keyboard-selected) radio button."""
        return self._native_get("selected") if self.is_mounted else None

    def action_previous_button(self) -> None:
        """Navigate to the previous button in the set (wraps around)."""
        self._native_call("run_action", "previous_button")

    def action_next_button(self) -> None:
        """Navigate to the next button in the set (wraps around)."""
        self._native_call("run_action", "next_button")

    def action_toggle_button(self) -> None:
        """Toggle the state of the currently selected button."""
        self._native_call("run_action", "toggle_button")


def _changed(index: int = -1, pressed: Any = None) -> RadioSet.Changed:
    return RadioSet.Changed(None, pressed, index)


_native.register_class("RadioSet.Changed", _changed)
_NATIVE_CLASSES["RadioSet"] = RadioSet

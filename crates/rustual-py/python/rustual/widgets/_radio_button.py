from __future__ import annotations

from typing import ClassVar

from .._bridge import _native
from ..widget import _NATIVE_CLASSES
from ._toggle_button import ToggleButton


class RadioButton(ToggleButton):
    """A radio button widget that represents a boolean value (best used in a ``RadioSet``)."""

    _rs_kind: ClassVar[str] = "RadioButton"
    _rs_framework: ClassVar[bool] = True

    BUTTON_INNER = "●"

    class Changed(ToggleButton.Changed):
        """Posted when the value of the radio button changes (``on_radio_button_changed``).

        Inside a ``RadioSet`` these are consumed by the set, which posts ``RadioSet.Changed``.
        """

        @property
        def radio_button(self) -> RadioButton:
            return self.toggle_button  # type: ignore[return-value]

        @property
        def control(self) -> RadioButton:
            return self.radio_button


_native.register_class("RadioButton.Changed", lambda **kw: RadioButton.Changed(None, **kw))
_NATIVE_CLASSES["RadioButton"] = RadioButton

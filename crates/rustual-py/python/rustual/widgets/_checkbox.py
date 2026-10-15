from __future__ import annotations

from typing import ClassVar

from .._bridge import _native
from ..widget import _NATIVE_CLASSES
from ._toggle_button import ToggleButton


class Checkbox(ToggleButton):
    """A check box widget that represents a boolean value."""

    _rs_kind: ClassVar[str] = "Checkbox"
    _rs_framework: ClassVar[bool] = True

    class Changed(ToggleButton.Changed):
        """Posted when the value of the checkbox changes (``on_checkbox_changed``)."""

        @property
        def checkbox(self) -> Checkbox:
            return self.toggle_button  # type: ignore[return-value]

        @property
        def control(self) -> Checkbox:
            return self.checkbox


_native.register_class("Checkbox.Changed", lambda **kw: Checkbox.Changed(None, **kw))
_NATIVE_CLASSES["Checkbox"] = Checkbox

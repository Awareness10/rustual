from __future__ import annotations

from typing import ClassVar

from ..widget import _NATIVE_CLASSES, Widget


class KeyPanel(Widget):
    """Shows the key bindings available in the current context."""

    _rs_kind: ClassVar[str] = "KeyPanel"
    _rs_framework: ClassVar[bool] = True

    def __init__(
        self,
        *,
        name: str | None = None,
        id: str | None = None,
        classes: str | None = None,
        disabled: bool = False,
    ) -> None:
        super().__init__(name=name, id=id, classes=classes, disabled=disabled)


_NATIVE_CLASSES["KeyPanel"] = KeyPanel

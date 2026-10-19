from __future__ import annotations

from typing import ClassVar

from ..widget import _NATIVE_CLASSES, Widget


class HelpPanel(Widget):
    """Shows context-sensitive help (``HELP``) for the focused widget and its key bindings."""

    _rs_kind: ClassVar[str] = "HelpPanel"
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


_NATIVE_CLASSES["HelpPanel"] = HelpPanel

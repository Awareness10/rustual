from __future__ import annotations

from typing import ClassVar

from ..widget import Widget, _NATIVE_CLASSES


class LoadingIndicator(Widget):
    """Display an animated loading indicator."""

    _rs_kind: ClassVar[str] = "LoadingIndicator"
    _rs_framework: ClassVar[bool] = True

    def __init__(
        self,
        name: str | None = None,
        id: str | None = None,
        classes: str | None = None,
        disabled: bool = False,
    ) -> None:
        super().__init__(name=name, id=id, classes=classes, disabled=disabled)


_NATIVE_CLASSES["LoadingIndicator"] = LoadingIndicator

from __future__ import annotations

from typing import ClassVar

from ..widget import Widget, _NATIVE_CLASSES


class Footer(Widget):
    """Shows the key bindings of the focused widget chain."""

    _rs_kind: ClassVar[str] = "Footer"
    _rs_framework: ClassVar[bool] = True

    def __init__(
        self,
        *,
        name: str | None = None,
        id: str | None = None,
        classes: str | None = None,
        disabled: bool = False,
        show_command_palette: bool = True,
        compact: bool = False,
    ) -> None:
        super().__init__(name=name, id=id, classes=classes, disabled=disabled)
        self._rs_kw = {"compact": compact}

    @property
    def compact(self) -> bool:
        return self._native_get("compact", False)

    @compact.setter
    def compact(self, value: bool) -> None:
        self._native_set("compact", bool(value))


_NATIVE_CLASSES["Footer"] = Footer

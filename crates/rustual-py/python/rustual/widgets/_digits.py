from __future__ import annotations

from typing import ClassVar

from ..widget import _NATIVE_CLASSES, Widget
from ._detached import DetachedNative


class Digits(DetachedNative, Widget):
    """Displays numbers with large 3x3 glyphs."""

    _rs_kind: ClassVar[str] = "Digits"
    _rs_framework: ClassVar[bool] = True

    def __init__(
        self,
        value: str = "",
        *,
        name: str | None = None,
        id: str | None = None,
        classes: str | None = None,
        disabled: bool = False,
    ) -> None:
        if not isinstance(value, str):
            raise TypeError("value must be a str")
        super().__init__(name=name, id=id, classes=classes, disabled=disabled)
        self._rs_kw = {"value": value}

    @property
    def value(self) -> str:
        return self._get("value")

    @value.setter
    def value(self, value: str) -> None:
        self.update(value)

    def update(self, value: str) -> None:
        if not isinstance(value, str):
            raise TypeError("value must be a str")
        self._call("update", value)


_NATIVE_CLASSES["Digits"] = Digits

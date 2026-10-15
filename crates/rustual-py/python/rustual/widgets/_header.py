from __future__ import annotations

from typing import ClassVar

from ..widget import Widget, _NATIVE_CLASSES


class Header(Widget):
    """The app header: icon, title / sub-title and an optional clock."""

    _rs_kind: ClassVar[str] = "Header"
    _rs_framework: ClassVar[bool] = True

    def __init__(
        self,
        show_clock: bool = False,
        *,
        name: str | None = None,
        id: str | None = None,
        classes: str | None = None,
        icon: str | None = None,
        time_format: str | None = None,
    ) -> None:
        super().__init__(name=name, id=id, classes=classes)
        self._rs_kw = {"show_clock": show_clock, "icon": icon}

    @property
    def tall(self) -> bool:
        return self._native_get("tall", False)

    @tall.setter
    def tall(self, value: bool) -> None:
        self._native_set("tall", bool(value))


_NATIVE_CLASSES["Header"] = Header

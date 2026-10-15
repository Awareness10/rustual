from __future__ import annotations

from typing import Any, ClassVar

from ..widget import _NATIVE_CLASSES
from ._static import Static


class Label(Static):
    """A simple text label (auto width)."""

    _rs_kind: ClassVar[str] = "Label"
    _rs_framework: ClassVar[bool] = True

    def __init__(
        self,
        content: Any = "",
        *,
        variant: str | None = None,
        expand: bool = False,
        shrink: bool = False,
        markup: bool = True,
        name: str | None = None,
        id: str | None = None,
        classes: str | None = None,
        disabled: bool = False,
    ) -> None:
        super().__init__(content, expand=expand, shrink=shrink, markup=markup, name=name, id=id, classes=classes, disabled=disabled)
        if variant:
            self.add_class(variant)


_NATIVE_CLASSES["Label"] = Label

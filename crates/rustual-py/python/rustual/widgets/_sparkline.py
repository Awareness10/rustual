from __future__ import annotations

from typing import Any, Callable, ClassVar, Sequence

from ..widget import Widget, _NATIVE_CLASSES
from ._detached import DetachedNative


class Sparkline(DetachedNative, Widget):
    """A sparkline bar chart of a sequence of numbers."""

    _rs_kind: ClassVar[str] = "Sparkline"
    _rs_framework: ClassVar[bool] = True

    def __init__(
        self,
        data: Sequence[float] | None = None,
        *,
        min_color: Any = None,
        max_color: Any = None,
        summary_function: Callable[[Sequence[float]], float] | None = None,
        name: str | None = None,
        id: str | None = None,
        classes: str | None = None,
        disabled: bool = False,
    ) -> None:
        super().__init__(name=name, id=id, classes=classes, disabled=disabled)
        self._rs_kw = {
            "data": [float(v) for v in data or ()],
            "min_color": None if min_color is None else str(min_color),
            "max_color": None if max_color is None else str(max_color),
            "summary_function": summary_function,
        }

    @property
    def data(self) -> list[float]:
        return self._get("data")

    @data.setter
    def data(self, data: Sequence[float] | None) -> None:
        self._call("set_data", [float(v) for v in data or ()])


_NATIVE_CLASSES["Sparkline"] = Sparkline

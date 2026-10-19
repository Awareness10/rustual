from __future__ import annotations

from typing import Any, ClassVar

from ..widget import _NATIVE_CLASSES, Widget
from ._detached import DetachedNative

_UNUSED: Any = object()


class ProgressBar(DetachedNative, Widget):
    """A progress bar with percentage and ETA (``total=None`` is indeterminate)."""

    _rs_kind: ClassVar[str] = "ProgressBar"
    _rs_framework: ClassVar[bool] = True

    def __init__(
        self,
        total: float | None = None,
        *,
        show_bar: bool = True,
        show_percentage: bool = True,
        show_eta: bool = True,
        name: str | None = None,
        id: str | None = None,
        classes: str | None = None,
        disabled: bool = False,
        clock: Any = None,
        gradient: Any = None,
    ) -> None:
        super().__init__(name=name, id=id, classes=classes, disabled=disabled)
        self._rs_kw = {
            "total": None if total is None else float(total),
            "show_bar": show_bar,
            "show_percentage": show_percentage,
            "show_eta": show_eta,
        }

    @property
    def total(self) -> float | None:
        return self._get("total")

    @total.setter
    def total(self, total: float | None) -> None:
        self._call("set_total", None if total is None else float(total))

    @property
    def progress(self) -> float:
        return self._get("progress")

    @progress.setter
    def progress(self, progress: float) -> None:
        self._call("set_progress", float(progress))

    @property
    def percentage(self) -> float | None:
        """Completed fraction in 0..1, or ``None`` while the total is unknown."""
        return self._get("percentage")

    @property
    def eta(self) -> int | None:
        return self._get("eta")

    def advance(self, advance: float = 1) -> None:
        self._call("advance", float(advance))

    def update(
        self,
        *,
        total: float | None = _UNUSED,
        progress: float = _UNUSED,
        advance: float = _UNUSED,
    ) -> None:
        """Update the total, the progress and / or advance (only the given arguments)."""
        if total is not _UNUSED:
            self.total = total
        if progress is not _UNUSED:
            self.progress = progress
        if advance is not _UNUSED:
            self.advance(advance)


_NATIVE_CLASSES["ProgressBar"] = ProgressBar

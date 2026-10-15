from __future__ import annotations

from typing import Any, ClassVar, Iterable

from .._rich import is_rich_renderable
from ..widget import Widget, _NATIVE_CLASSES
from ._detached import DetachedNative


class _LogOptions:
    def _option(name: str) -> property:  # type: ignore[misc]
        def fget(self: Any) -> Any:
            return self._get(name)

        def fset(self: Any, value: Any) -> None:
            self._call(f"set_{name}", value)

        return property(fget, fset)

    max_lines = _option("max_lines")
    auto_scroll = _option("auto_scroll")
    highlight = _option("highlight")
    del _option

    @property
    def lines(self) -> list[str]:
        return self._get("lines")  # type: ignore[attr-defined]

    @property
    def line_count(self) -> int:
        return self._get("line_count")  # type: ignore[attr-defined]


class Log(_LogOptions, DetachedNative, Widget):
    """A log of plain text lines (fast; no markup)."""

    _rs_kind: ClassVar[str] = "Log"
    _rs_framework: ClassVar[bool] = True
    can_focus = True

    def __init__(
        self,
        highlight: bool = False,
        max_lines: int | None = None,
        auto_scroll: bool = True,
        name: str | None = None,
        id: str | None = None,
        classes: str | None = None,
        disabled: bool = False,
    ) -> None:
        super().__init__(name=name, id=id, classes=classes, disabled=disabled)
        self._rs_kw = {"highlight": highlight, "max_lines": max_lines, "auto_scroll": auto_scroll}

    def write(self, data: Any, scroll_end: bool | None = None) -> Log:
        """Write text; newlines start new lines (a partial line is continued by the next write)."""
        self._call("write", data if isinstance(data, str) else str(data), scroll_end)
        return self

    def write_line(self, line: Any, scroll_end: bool | None = None) -> Log:
        return self.write_lines([line], scroll_end)

    def write_lines(self, lines: Iterable[Any], scroll_end: bool | None = None) -> Log:
        self._call("write_lines", [line if isinstance(line, str) else str(line) for line in lines], scroll_end)
        return self

    def clear(self) -> Log:
        self._call("clear")
        return self


class RichLog(_LogOptions, DetachedNative, Widget):
    """A log of styled text (markup, Rich renderables, highlighting)."""

    _rs_kind: ClassVar[str] = "RichLog"
    _rs_framework: ClassVar[bool] = True
    can_focus = True

    def __init__(
        self,
        *,
        max_lines: int | None = None,
        min_width: int = 78,
        wrap: bool = False,
        highlight: bool = False,
        markup: bool = False,
        auto_scroll: bool = True,
        name: str | None = None,
        id: str | None = None,
        classes: str | None = None,
        disabled: bool = False,
    ) -> None:
        super().__init__(name=name, id=id, classes=classes, disabled=disabled)
        self._rs_kw = {
            "max_lines": max_lines,
            "min_width": min_width,
            "wrap": wrap,
            "highlight": highlight,
            "markup": markup,
            "auto_scroll": auto_scroll,
        }

    def _option(name: str) -> property:  # type: ignore[misc]
        def fget(self: Any) -> Any:
            return self._get(name)

        def fset(self: Any, value: Any) -> None:
            self._call(f"set_{name}", value)

        return property(fget, fset)

    wrap = _option("wrap")
    markup = _option("markup")
    min_width = _option("min_width")
    del _option

    def write(
        self,
        content: Any,
        width: int | None = None,
        expand: bool = False,
        shrink: bool = True,
        scroll_end: bool | None = None,
        animate: bool = False,
    ) -> RichLog:
        """Write content: a ``str`` (markup if ``markup=True``), ``Content`` / Rich ``Text``,
        any Rich renderable, or another object (shown with ``str()``)."""
        if isinstance(content, str):
            self._call("write", content, "auto", scroll_end)
            return self
        rs_markup = getattr(content, "__rs_markup__", None)
        if isinstance(rs_markup, str):
            self._call("write", rs_markup, "text", scroll_end)
            return self
        rich_markup = getattr(content, "markup", None)
        if isinstance(rich_markup, str) and hasattr(content, "plain"):
            self._call("write", rich_markup, "text", scroll_end)
            return self
        if is_rich_renderable(content):
            self._call("write_rich", content, width, scroll_end)
            return self
        self._call("write", str(content), "plain", scroll_end)
        return self

    def clear(self) -> RichLog:
        self._call("clear")
        return self


_NATIVE_CLASSES["Log"] = Log
_NATIVE_CLASSES["RichLog"] = RichLog

"""Render Rich renderables (``Table``, ``Syntax``, ``Panel``, ...) into styled lines for the engine."""

from __future__ import annotations

from functools import lru_cache
from typing import Any

StyleTuple = tuple


def is_rich_renderable(obj: Any) -> bool:
    """Whether ``obj`` is a Rich renderable that isn't plain text / markup."""
    if isinstance(obj, str):
        return False
    if hasattr(obj, "__rs_markup__"):
        return False
    return hasattr(obj, "__rich_console__") or hasattr(obj, "__rich__")


@lru_cache(maxsize=8)
def _console(width: int) -> Any:
    from rich.console import Console

    return Console(
        width=width,
        color_system="truecolor",
        force_terminal=True,
        legacy_windows=False,
        no_color=False,
        markup=True,
        emoji=True,
        highlight=False,
    )


def _rgb(color: Any) -> tuple[int, int, int] | None:
    if color is None or color.is_default:
        return None
    t = color.get_truecolor()
    return (t.red, t.green, t.blue)


@lru_cache(maxsize=1024)
def _style(style: Any) -> StyleTuple | None:
    if style is None:
        return None
    link = style.link or None
    return (
        _rgb(style.color),
        _rgb(style.bgcolor),
        bool(style.bold),
        bool(style.dim),
        bool(style.italic),
        bool(style.underline),
        bool(style.strike),
        bool(style.reverse),
        link,
    )


def render_lines(renderable: Any, width: int) -> list[list[tuple[str, StyleTuple | None]]]:
    """Render ``renderable`` at ``width`` cells: a list of lines of ``(text, style)`` segments."""
    width = max(1, int(width))
    console = _console(width)
    options = console.options.update_width(width)
    lines = console.render_lines(renderable, options, pad=False, new_lines=False)
    out: list[list[tuple[str, StyleTuple | None]]] = []
    for line in lines:
        out.append([(seg.text, _style(seg.style)) for seg in line if not seg.control and seg.text])
    return out

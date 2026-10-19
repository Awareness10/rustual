"""Color name tables (Textual's ``textual._color_constants``)."""

from __future__ import annotations

from ._bridge import _native

ANSI_COLORS = [
    "black",
    "red",
    "green",
    "yellow",
    "blue",
    "magenta",
    "cyan",
    "white",
    "bright_black",
    "bright_red",
    "bright_green",
    "bright_yellow",
    "bright_blue",
    "bright_magenta",
    "bright_cyan",
    "bright_white",
]
"""The 16 ANSI color names, in index order."""

COLOR_NAME_TO_RGB: dict[str, tuple[int, int, int] | tuple[int, int, int, int]] = {
    name: tuple(rgb)
    for name, rgb in _native.color_names()  # type: ignore[misc]
}
"""Named colors → RGB (RGBA for translucent ones such as ``transparent``)."""

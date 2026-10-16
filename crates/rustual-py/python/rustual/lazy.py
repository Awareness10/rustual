"""``Lazy`` and ``Reveal`` (Textual's ``textual.lazy``).

Textual mounts a lazy widget after the first refresh to show the rest of the UI sooner. rustual
builds widgets fast enough that this isn't needed: ``Lazy(widget)`` and ``Reveal(widget)`` are
the widget itself, mounted immediately (children added with ``with Lazy(...)`` go to it, as in
Textual).
"""

from __future__ import annotations

from typing import Any

from .widget import Widget

__all__ = ["Lazy", "Reveal"]


class Lazy(Widget):
    """Mount ``widget`` (Textual defers it until after a refresh)."""

    def __new__(cls, widget: Widget) -> Any:  # type: ignore[misc]
        return widget


class Reveal(Widget):
    """Mount ``widget`` (Textual reveals its children one by one)."""

    def __new__(cls, widget: Widget, delay: float = 1 / 20) -> Any:  # type: ignore[misc]
        return widget

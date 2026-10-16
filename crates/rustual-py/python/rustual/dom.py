"""``DOMNode`` (Textual's ``textual.dom``): the base of the app, screens and widgets."""

from __future__ import annotations

from .widget import Widget as DOMNode


class NoScreen(Exception):
    """Raised when there is no screen for a node."""


__all__ = ["DOMNode", "NoScreen"]

"""textual_rs: a Textual-compatible Python API on the textual-rs (Rust) engine.

Port a Textual app by replacing ``textual`` with ``textual_rs`` in its imports::

    from textual_rs.app import App, ComposeResult
    from textual_rs.widgets import Button, Header, Footer
"""

from __future__ import annotations

from ._on import on
from .worker import work

__version__ = "0.1.0"

__all__ = ["on", "work", "__version__"]

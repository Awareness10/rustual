"""rustual: a Textual-compatible Python API on the rustual (Rust) engine.

Port a Textual app by replacing ``textual`` with ``rustual`` in its imports::

    from rustual.app import App, ComposeResult
    from rustual.widgets import Button, Header, Footer
"""

from __future__ import annotations

from ._on import on
from .worker import work

__version__ = "0.1.0"

__all__ = ["on", "work", "__version__"]

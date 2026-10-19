"""rustual: a Textual-compatible Python API on the rustual (Rust) engine.

Port a Textual app by replacing ``textual`` with ``rustual`` in its imports::

    from rustual.app import App, ComposeResult
    from rustual.widgets import Button, Header, Footer
"""

from __future__ import annotations

from ._log import Logger, log
from ._on import on
from .worker import work

try:
    from importlib.metadata import version as _version

    __version__ = _version("rustual-py")
except Exception:
    __version__ = "0.0.0"

__all__ = ["Logger", "__version__", "log", "on", "work"]

"""rustual: a Textual-compatible Python API on the rustual (Rust) engine.

Port a Textual app by replacing ``textual`` with ``rustual`` in its imports::

    from rustual.app import App, ComposeResult
    from rustual.widgets import Button, Header, Footer
"""

from __future__ import annotations

from ._on import on
from .worker import work

try:
    from importlib.metadata import version as _version

    __version__ = _version("rustual-py")
except Exception:  # noqa: BLE001 - not installed (e.g. running from a source tree)
    __version__ = "0.0.0"

__all__ = ["on", "work", "__version__"]

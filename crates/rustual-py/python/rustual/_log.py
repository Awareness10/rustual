"""``log`` / ``self.log`` (Textual's ``Logger``).

There is no devtools console; as in Textual without one, messages are appended to the file named
by ``TEXTUAL_LOG`` when it is set. They are also forwarded to Python's :mod:`logging` (logger
``"rustual"``; the group picks the level), so a ``logging`` handler can show them.
"""

from __future__ import annotations

import logging
import os
from typing import Any

_LEVELS = {
    "event": logging.DEBUG,
    "debug": logging.DEBUG,
    "info": logging.INFO,
    "warning": logging.WARNING,
    "error": logging.ERROR,
    "print": logging.INFO,
    "system": logging.DEBUG,
    "logging": logging.INFO,
    "worker": logging.DEBUG,
}

_logger = logging.getLogger("rustual")


class Logger:
    """Callable logger: ``log("message", key=value)``, or a group: ``log.warning(...)``."""

    def __init__(self, group: str = "info", verbose: bool = False) -> None:
        self._group = group
        self._verbose = verbose

    def __repr__(self) -> str:
        return f"Logger(group={self._group!r}, verbose={self._verbose})"

    def __call__(self, *args: object, **kwargs: Any) -> None:
        output = " ".join(str(arg) for arg in args)
        if kwargs:
            key_values = " ".join(f"{key}={value!r}" for key, value in kwargs.items())
            output = f"{output} {key_values}" if output else key_values
        log_file = os.environ.get("TEXTUAL_LOG")
        if log_file:
            with open(log_file, "a", encoding="utf-8") as f:
                print(output, file=f)
        _logger.log(_LEVELS.get(self._group, logging.INFO), output)

    def verbosity(self, verbose: bool) -> Logger:
        return Logger(self._group, verbose)

    @property
    def verbose(self) -> Logger:
        return Logger(self._group, True)

    @property
    def event(self) -> Logger:
        return Logger("event")

    @property
    def debug(self) -> Logger:
        return Logger("debug")

    @property
    def info(self) -> Logger:
        return Logger("info")

    @property
    def warning(self) -> Logger:
        return Logger("warning")

    @property
    def error(self) -> Logger:
        return Logger("error")

    @property
    def system(self) -> Logger:
        return Logger("system")

    @property
    def logging(self) -> Logger:
        return Logger("logging")

    @property
    def worker(self) -> Logger:
        return Logger("worker")


log = Logger()

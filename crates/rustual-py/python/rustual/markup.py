"""Markup helpers (Textual's ``textual.markup``)."""

from __future__ import annotations

import re
from typing import Callable, Match

_escape_sub: Callable[..., str] = re.compile(r"(\\*)(\[[a-z#/@][^[]*?])").sub


def escape(markup: str) -> str:
    """Escape text so that it won't be interpreted as markup (``[b]`` → ``\\[b]``)."""

    def escape_backslashes(match: Match[str]) -> str:
        backslashes, text = match.groups()
        return f"{backslashes}{backslashes}\\{text}"

    markup = _escape_sub(escape_backslashes, markup)
    if markup.endswith("\\") and not markup.endswith("\\\\"):
        return markup + "\\"
    return markup


__all__ = ["escape"]

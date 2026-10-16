"""Content helpers (a minimal subset of Textual's ``textual.content``)."""

from __future__ import annotations

from typing import Any


class Content:
    """Text content built from markup (``Content.from_markup``) or plain text."""

    def __init__(self, text: str = "", *, markup: str | None = None) -> None:
        self._plain = text
        self._markup = markup if markup is not None else escape(text)

    @classmethod
    def from_markup(cls, markup: str, **variables: Any) -> Content:
        if variables:
            markup = markup.format(**variables)
        from ._bridge import _native

        content = cls.__new__(cls)
        content._markup = markup
        content._plain = _native.markup_plain(markup)
        return content

    @property
    def plain(self) -> str:
        return self._plain

    @property
    def __rs_markup__(self) -> str:
        return self._markup

    def __str__(self) -> str:
        return self._plain

    def __repr__(self) -> str:
        return f"Content({self._markup!r})"


def escape(text: str) -> str:
    """Escape markup characters so ``text`` renders literally."""
    return text.replace("[", r"\[")


def to_markup(content: Any, markup: bool = True) -> tuple[str, bool]:
    """Normalize widget content to ``(text, is_markup)``."""
    if content is None:
        return "", markup
    if isinstance(content, str):
        return content, markup
    rs = getattr(content, "__rs_markup__", None)
    if isinstance(rs, str):
        return rs, True
    rich_markup = getattr(content, "markup", None)
    if isinstance(rich_markup, str) and hasattr(content, "plain"):
        return rich_markup, True
    return str(content), False

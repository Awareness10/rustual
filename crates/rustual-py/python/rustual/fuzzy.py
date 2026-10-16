"""Fuzzy matching (Textual's ``textual.fuzzy``), backed by rustual's matcher."""

from __future__ import annotations

from typing import Any

from .content import Content, escape


def _style_markup(style: Any) -> str:
    """A match style as a markup tag body (``"reverse"``, ``"bold #ff0000"``, ...)."""
    if style is None:
        return "reverse"
    return str(style) or "reverse"


class Matcher:
    """Matches strings against a query (``match`` scores, ``highlight`` marks the letters)."""

    def __init__(self, query: str, *, match_style: Any = None, case_sensitive: bool = False) -> None:
        self._query = query
        self._match_style = match_style
        self._case_sensitive = case_sensitive

    @property
    def query(self) -> str:
        return self._query

    @property
    def match_style(self) -> Any:
        return self._match_style

    @property
    def case_sensitive(self) -> bool:
        return self._case_sensitive

    def _match(self, candidate: str) -> tuple[float, list[int]]:
        from ._bridge import _native

        return _native.fuzzy_match(self._query, candidate, self._case_sensitive)

    def match(self, candidate: str) -> float:
        """How well ``candidate`` matches the query (0: no match; higher is better, on
        Textual's scale)."""
        return self._match(candidate)[0]

    def highlight(self, candidate: str) -> Content:
        """``candidate`` with the matched letters in the match style."""
        score, offsets = self._match(candidate)
        if not score:
            return Content(candidate)
        tag = _style_markup(self._match_style)
        marked = set(offsets)
        parts: list[str] = []
        for i, char in enumerate(candidate):
            text = escape(char)
            parts.append(f"[{tag}]{text}[/]" if i in marked and not char.isspace() else text)
        return Content.from_markup("".join(parts))


__all__ = ["Matcher"]

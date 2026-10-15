"""Shared helpers for the built-in widget bindings."""

from __future__ import annotations

from typing import Any

from ..content import escape, to_markup


def compact_property() -> property:
    """A ``compact`` property backed by the ``-textual-compact`` class (and the native kwarg)."""

    def fget(self: Any) -> bool:
        return self.has_class("-textual-compact")

    def fset(self: Any, value: bool) -> None:
        value = bool(value)
        if not self.is_mounted:
            # Native widgets that know about compact set the class themselves on mount.
            self.__dict__.setdefault("_rs_kw", {})["compact"] = value
        self.set_class(value, "-textual-compact")

    return property(fget, fset, doc="Enable compact display?")


def prompt_markup(prompt: Any, markup: bool = True) -> str:
    """A prompt (str, ``Content``, Rich ``Text``, other) as content markup."""
    text, is_markup = to_markup(prompt, markup)
    return text if is_markup else escape(text)

"""Signals (Textual's ``textual.signal``): publish / subscribe between widgets."""

from __future__ import annotations

from typing import Any, Callable, Generic, TypeVar

from .widget import _invoke

SignalT = TypeVar("SignalT")


class SignalError(Exception):
    pass


class Signal(Generic[SignalT]):
    """A signal that widgets can subscribe to (callbacks are dropped when the subscriber unmounts)."""

    def __init__(self, owner: Any, name: str) -> None:
        self._owner = owner
        self._name = name
        self._subscriptions: list[tuple[Any, Callable[..., Any]]] = []

    def __repr__(self) -> str:
        return f"Signal({self._name!r})"

    def subscribe(self, node: Any, callback: Callable[..., Any], immediate: bool = False) -> None:
        self._subscriptions.append((node, callback))

    def unsubscribe(self, node: Any) -> None:
        self._subscriptions = [(n, c) for n, c in self._subscriptions if n is not node]

    def publish(self, data: SignalT | None = None) -> None:
        for node, callback in list(self._subscriptions):
            if getattr(node, "_node", None) is not None and not node.is_mounted:
                self.unsubscribe(node)
                continue
            _invoke(callback, data)

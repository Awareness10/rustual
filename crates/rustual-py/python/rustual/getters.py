"""Descriptors for widget properties (Textual's ``textual.getters``)::

class MyScreen(Screen):
    output = getters.query_one("#output", RichLog)
    app = getters.app(MyApp)
"""

from __future__ import annotations

from collections.abc import Callable
from inspect import isclass
from typing import Any, Generic, TypeVar

from .widget import Widget

T = TypeVar("T")


class app(Generic[T]):
    """The running app, typed as your ``App`` subclass."""

    def __init__(self, app_type: type[T] | Callable[[], type[T]]) -> None:
        self._app_type = app_type if isclass(app_type) else app_type()  # type: ignore[operator]

    def __get__(self, obj: Any, obj_type: Any) -> T:
        from . import _context

        current = _context._active_app
        if current is None:
            raise RuntimeError("no active app")
        assert isinstance(current, self._app_type)
        return current  # type: ignore[return-value]


class query_one(Generic[T]):
    """A property returning ``self.query_one(selector, expect_type)`` on access."""

    def __init__(self, selector: str | type[T], expect_type: type[T] | None = None) -> None:
        self.expect_type: type[Any] = Widget if expect_type is None else expect_type
        if isinstance(selector, str):
            self.selector = selector
        else:
            self.selector = selector.__name__
            self.expect_type = selector

    def __get__(self, obj: Any, obj_type: Any) -> Any:
        if obj is None:
            return self
        return obj.query_one(self.selector, self.expect_type)


class child_by_id(Generic[T]):
    """A property returning the immediate child with this ``id``."""

    def __init__(self, child_id: str, expect_type: type[T] | None = None) -> None:
        self.child_id = child_id
        self.expect_type: type[Any] = Widget if expect_type is None else expect_type

    def __get__(self, obj: Any, obj_type: Any) -> Any:
        if obj is None:
            return self
        from .app import App

        # As in Textual, an app's children are those of its active screen.
        base = obj.screen if isinstance(obj, App) else obj
        return base.get_child_by_id(self.child_id, self.expect_type)


__all__ = ["app", "child_by_id", "query_one"]

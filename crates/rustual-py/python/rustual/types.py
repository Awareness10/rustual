"""Types used across the API (Textual's ``textual.types``)."""

from __future__ import annotations

from collections.abc import Awaitable, Callable
from pathlib import PurePath
from typing import Any, Literal, Union

from .widgets._placeholder import PlaceholderVariant
from .widgets.directory_tree import DirEntry
from .widgets.option_list import DuplicateID, OptionDoesNotExist
from .widgets.select import NoSelection

CSSPathType = Union[str, PurePath, "list[str | PurePath]"]
"""Valid ways of specifying paths to CSS files."""
AnimationLevel = Literal["none", "basic", "full"]
"""The levels that the ``TEXTUAL_ANIMATIONS`` env var can be set to."""
CallbackType = Callable[[], Awaitable[None]] | Callable[[], None]
"""Type used for arbitrary callables used in callbacks."""
IgnoreReturnCallbackType = Callable[[], Awaitable[Any]] | Callable[[], Any]
"""A callback whose return value is ignored."""
WatchCallbackType = Callable[..., Awaitable[None]] | Callable[..., None]
"""Type for a watch callback."""
InputValidationOn = Literal["blur", "changed", "submitted"]
"""When an ``Input`` validates its value."""
Direction = Literal[-1, 1]
"""Navigation direction."""
SelectType = Any
"""The type of a ``Select`` value."""
MessageTarget = Any
"""A message target (a widget or the app)."""


class UnusedParameter:
    """Marks an unused parameter default."""


class CSSPathError(Exception):
    """A CSS path could not be read."""


class NoActiveAppError(RuntimeError):
    """There is no active app."""


__all__ = [
    "AnimationLevel",
    "CSSPathError",
    "CSSPathType",
    "CallbackType",
    "DirEntry",
    "Direction",
    "DuplicateID",
    "IgnoreReturnCallbackType",
    "InputValidationOn",
    "MessageTarget",
    "NoActiveAppError",
    "NoSelection",
    "OptionDoesNotExist",
    "PlaceholderVariant",
    "SelectType",
    "UnusedParameter",
    "WatchCallbackType",
]

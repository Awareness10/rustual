"""The ``@on`` decorator (Textual's ``textual.on``)."""

from __future__ import annotations

from typing import Any, Callable, TypeVar

from .message import Message

DecoratedType = TypeVar("DecoratedType", bound=Callable[..., Any])


class OnDecoratorError(Exception):
    pass


def on(message_type: type[Message], selector: str | None = None, **kwargs: str) -> Callable[[DecoratedType], DecoratedType]:
    """Handle ``message_type``, optionally only from widgets matching ``selector``.

    Extra keyword arguments match other widget attributes of the message, for messages
    that list them in ``ALLOW_SELECTOR_MATCH`` (e.g. ``@on(TabbedContent.TabActivated, pane="#home")``).
    """
    selectors: dict[str, str] = {}
    if selector is not None:
        selectors["control"] = selector
    for attribute, css_selector in kwargs.items():
        if attribute not in message_type.ALLOW_SELECTOR_MATCH and attribute != "control":
            raise OnDecoratorError(
                f"The attribute {attribute!r} can't be matched; have you added it to {message_type.__name__}.ALLOW_SELECTOR_MATCH?"
            )
        selectors[attribute] = css_selector

    def decorator(method: DecoratedType) -> DecoratedType:
        existing = list(getattr(method, "_textual_on", []))
        existing.append((message_type, selectors))
        method._textual_on = existing  # type: ignore[attr-defined]
        return method

    return decorator

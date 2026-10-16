"""Messages: posted by widgets, they bubble up the DOM (Textual's ``textual.message``)."""

from __future__ import annotations

import re
from typing import TYPE_CHECKING, Any, ClassVar

if TYPE_CHECKING:
    from .widget import Widget

_CAMEL = re.compile(r"(?<!^)(?=[A-Z])")


def camel_to_snake(name: str) -> str:
    return _CAMEL.sub("_", name).lower()


class Message:
    """Base class for messages.

    The handler name is ``on_<namespace>_<name>``, where the namespace is the class the message
    is defined in (``Button.Pressed`` → ``on_button_pressed``).
    """

    bubble: ClassVar[bool] = True
    verbose: ClassVar[bool] = False
    no_dispatch: ClassVar[bool] = False
    namespace: ClassVar[str] = ""
    handler_name: ClassVar[str] = "on_message"
    ALLOW_SELECTOR_MATCH: ClassVar[set[str]] = set()

    def __init_subclass__(
        cls,
        bubble: bool | None = True,
        verbose: bool = False,
        no_dispatch: bool | None = False,
        namespace: str | None = None,
    ) -> None:
        super().__init_subclass__()
        if bubble is not None:
            cls.bubble = bubble
        cls.verbose = verbose
        if no_dispatch is not None:
            cls.no_dispatch = no_dispatch
        if namespace is not None:
            cls.namespace = namespace
            name = f"{namespace}_{camel_to_snake(cls.__name__)}"
        else:
            qualname = cls.__qualname__.rsplit("<locals>.", 1)[-1]
            parts = [p for p in qualname.split(".")]
            if len(parts) >= 2:
                name = f"{camel_to_snake(parts[-2])}_{camel_to_snake(parts[-1])}"
            else:
                name = camel_to_snake(parts[-1])
        cls.handler_name = f"on_{name}"

    def __init__(self) -> None:
        self.__post_init__()

    def __post_init__(self) -> None:
        """Initialize message state (also run by the ``__init__`` of ``@dataclass`` messages)."""
        self._sender: Any = None
        self._stop_propagation = False
        self._no_default_action = False
        self._forwarded = False

    def __repr__(self) -> str:
        return f"{type(self).__name__}()"

    # -- identity -------------------------------------------------------------------------

    @property
    def sender(self) -> Widget | None:
        """The widget that posted the message."""
        from .widget import _widget_for

        s = self._sender
        if isinstance(s, int):
            return _widget_for(s)
        return s

    @property
    def control(self) -> Widget | None:
        """The widget this message is about (overridden by widget messages)."""
        return None

    @property
    def is_forwarded(self) -> bool:
        return self._forwarded

    # -- flow control ---------------------------------------------------------------------

    def stop(self, stop: bool = True) -> Message:
        """Stop the message from bubbling to the parent."""
        self._stop_propagation = stop
        return self

    def prevent_default(self, prevent: bool = True) -> Message:
        """Prevent base classes (and the built-in widget) from handling the message."""
        self._no_default_action = prevent
        return self

    def set_forwarded(self) -> None:
        self._forwarded = True

    def can_replace(self, message: Message) -> bool:
        return False


class WidgetMessage(Message):
    """A message about the widget that sent it (``control`` is the sender)."""

    @property
    def control(self) -> Widget | None:
        return self.sender

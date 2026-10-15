"""System events (Textual's ``textual.events``)."""

from __future__ import annotations

from typing import Any

from .geometry import Offset, Size
from .message import Message


class Event(Message):
    """Base class for events. Events don't bubble unless noted."""


class Bubble:
    """Marker mix-in for events that bubble."""


class Mount(Event, bubble=False):
    pass


class Unmount(Event, bubble=False):
    pass


class Compose(Event, bubble=False):
    pass


class Ready(Event, bubble=False):
    pass


class Focus(Event, bubble=False):
    pass


class Blur(Event, bubble=False):
    pass


class Enter(Event, bubble=False):
    pass


class Leave(Event, bubble=False):
    pass


class Show(Event, bubble=False):
    pass


class Hide(Event, bubble=False):
    pass


class Resize(Event, bubble=False):
    def __init__(self, size: Size, virtual_size: Size | None = None, container_size: Size | None = None) -> None:
        super().__init__()
        self.size = size
        self.virtual_size = virtual_size or size
        self.container_size = container_size or size

    def __repr__(self) -> str:
        return f"Resize(size={self.size!r})"


class Key(Event):
    """A key press. ``key`` is the Textual key name (``"a"``, ``"ctrl+c"``, ``"enter"``)."""

    def __init__(self, key: str, character: str | None) -> None:
        super().__init__()
        self.key = key
        self.character = character
        self.aliases = [key]

    @property
    def name(self) -> str:
        """Key name usable in a method name (``ctrl+c`` → ``ctrl_c``)."""
        return self.key.replace("+", "_")

    @property
    def is_printable(self) -> bool:
        return self.character is not None and self.character.isprintable()

    def __repr__(self) -> str:
        return f"Key(key={self.key!r}, character={self.character!r})"


class Paste(Event):
    def __init__(self, text: str) -> None:
        super().__init__()
        self.text = text


class MouseEvent(Event):
    """Base class for mouse events; ``x``/``y`` are relative to the widget."""

    def __init__(
        self,
        x: int,
        y: int,
        screen_x: int,
        screen_y: int,
        button: int = 0,
        ctrl: bool = False,
        meta: bool = False,
        shift: bool = False,
        delta_x: int = 0,
        delta_y: int = 0,
        widget: Any = None,
    ) -> None:
        super().__init__()
        self.x = x
        self.y = y
        self.screen_x = screen_x
        self.screen_y = screen_y
        self.button = button
        self.ctrl = ctrl
        self.meta = meta
        self.shift = shift
        self.delta_x = delta_x
        self.delta_y = delta_y
        self.widget = widget

    @property
    def offset(self) -> Offset:
        return Offset(self.x, self.y)

    @property
    def screen_offset(self) -> Offset:
        return Offset(self.screen_x, self.screen_y)

    @property
    def delta(self) -> Offset:
        return Offset(self.delta_x, self.delta_y)

    def __repr__(self) -> str:
        return f"{type(self).__name__}(x={self.x}, y={self.y}, button={self.button})"


class MouseMove(MouseEvent, bubble=False):
    pass


class MouseDown(MouseEvent):
    pass


class MouseUp(MouseEvent):
    pass


class MouseScrollUp(MouseEvent):
    pass


class MouseScrollDown(MouseEvent):
    pass


class MouseScrollLeft(MouseEvent):
    pass


class MouseScrollRight(MouseEvent):
    pass


class Click(MouseEvent):
    @property
    def chain(self) -> int:
        return 1


# Native event kind → (class, handler name).
_EVENTS: dict[str, type[Event]] = {
    "key": Key,
    "mouse_down": MouseDown,
    "mouse_up": MouseUp,
    "click": Click,
    "mouse_move": MouseMove,
    "mouse_scroll_up": MouseScrollUp,
    "mouse_scroll_down": MouseScrollDown,
    "mouse_scroll_left": MouseScrollLeft,
    "mouse_scroll_right": MouseScrollRight,
    "enter": Enter,
    "leave": Leave,
    "focus": Focus,
    "blur": Blur,
    "resize": Resize,
    "paste": Paste,
}


_KEY_NAME_REPLACEMENTS = {
    "solidus": "slash",
    "reverse_solidus": "backslash",
    "commercial_at": "at",
    "hyphen_minus": "minus",
    "plus_sign": "plus",
    "low_line": "underscore",
}


def _character_to_key(character: str) -> str:
    """Textual's key name for a single character (``"+"`` → ``"plus"``, ``"="`` → ``"equals_sign"``)."""
    if not character.isalnum():
        try:
            import unicodedata

            key = unicodedata.name(character).lower().replace("-", "_").replace(" ", "_")
        except ValueError:
            key = character
    else:
        key = character
    return _KEY_NAME_REPLACEMENTS.get(key, key)


def _textual_key(key: str) -> str:
    """Convert a native key (punctuation as the character itself) to Textual's key name."""
    parts = key.split("+")
    if parts[-1] == "":  # the key itself is "+", e.g. "+" or "ctrl++"
        parts = [p for p in parts[:-2] if p] + ["+"]
    *modifiers, last = parts
    if len(last) == 1:
        last = _character_to_key(last)
    return "+".join([*modifiers, last])


def from_native(data: tuple) -> Event:
    kind = data[0]
    cls = _EVENTS[kind]
    if kind == "key":
        return Key(_textual_key(data[1]), data[2])
    if kind == "resize":
        return Resize(Size(data[1], data[2]))
    if kind == "paste":
        return Paste(data[1])
    if issubclass(cls, MouseEvent):
        return cls(*data[1:])
    return cls()

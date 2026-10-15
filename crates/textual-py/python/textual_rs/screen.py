"""Screens (Textual's ``textual.screen``)."""

from __future__ import annotations

from typing import Any, Callable, ClassVar, Generic, TypeVar

from ._bridge import native
from .widget import AwaitComplete, Widget, _invoke, _NATIVE_CLASSES, _widget_for

ScreenResultType = TypeVar("ScreenResultType")


class ScreenError(Exception):
    pass


class ScreenStackError(ScreenError):
    pass


class Screen(Widget, Generic[ScreenResultType]):
    """A screen: the root of a widget tree that fills the terminal.

    Push screens with ``app.push_screen(MyScreen(), callback)``; ``dismiss(result)`` pops the
    screen and calls the callback with the result.
    """

    _rs_kind: ClassVar[str] = "Screen"
    _rs_framework: ClassVar[bool] = True
    _rs_modal: ClassVar[bool] = False

    CSS: ClassVar[str] = ""
    CSS_PATH: ClassVar[Any] = None
    AUTO_FOCUS: ClassVar[str | None] = None
    TITLE: ClassVar[str | None] = None
    SUB_TITLE: ClassVar[str | None] = None

    def __init__(self, name: str | None = None, id: str | None = None, classes: str | None = None) -> None:
        super().__init__(name=name, id=id, classes=classes)
        self._result_callback: Callable[..., Any] | None = None

    @property
    def is_modal(self) -> bool:
        return self._rs_modal

    @property
    def is_current(self) -> bool:
        return self.is_mounted and native.active_screen() == self._node

    @property
    def focused(self) -> Widget | None:
        node = native.focused()
        if node is None:
            return None
        widget = _widget_for(node)
        return widget if widget.is_mounted and native.screen_of(node) == self._node else None

    def set_focus(self, widget: Widget | None, scroll_visible: bool = True) -> None:
        if widget is None:
            native.blur()
        else:
            widget.focus(scroll_visible)

    def dismiss(self, result: ScreenResultType | None = None) -> AwaitComplete:
        """Pop this screen and call the callback passed to ``push_screen`` with ``result``."""
        if not self.is_current:
            raise ScreenStackError("Can't dismiss a screen that isn't the active screen")
        callback = self.__dict__.get("_result_callback")
        native.pop_screen()
        if callback is not None:
            _invoke(callback, result)
        return AwaitComplete()

    def action_dismiss(self, result: Any = None) -> None:
        self.dismiss(result)

    @classmethod
    def _rs_css(cls) -> list[tuple[str, str]]:
        """``(key, css)`` for this screen class's ``CSS`` / ``CSS_PATH``."""
        from .app import _read_css_paths

        out: list[tuple[str, str]] = []
        for klass in reversed(cls.__mro__):
            if not isinstance(klass, type) or not issubclass(klass, Screen):
                continue
            key = f"screen:{klass.__module__}.{klass.__qualname__}"
            if klass.__dict__.get("CSS_PATH"):
                for i, css in enumerate(_read_css_paths(klass, klass.__dict__["CSS_PATH"])):
                    out.append((f"{key}:path{i}", css))
            if klass.__dict__.get("CSS"):
                out.append((key, klass.__dict__["CSS"]))
        return out

    def _register_css(self) -> None:
        for key, css in type(self)._rs_css():
            if not native.has_css(key):
                native.add_css(key, css)


class ModalScreen(Screen[ScreenResultType]):
    """A screen shown over the previous one (which stays visible behind it)."""

    _rs_modal: ClassVar[bool] = True
    _rs_framework: ClassVar[bool] = True


class SystemModalScreen(ModalScreen[ScreenResultType]):
    _rs_framework: ClassVar[bool] = True


_NATIVE_CLASSES["Screen"] = Screen
_NATIVE_CLASSES["ModalScreen"] = ModalScreen

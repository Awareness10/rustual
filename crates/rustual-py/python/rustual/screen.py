"""Screens (Textual's ``textual.screen``)."""

from __future__ import annotations

from collections.abc import Callable
from typing import Any, ClassVar, Generic, TypeVar

from ._bridge import native
from .widget import _NATIVE_CLASSES, AwaitComplete, Widget, _invoke, _widget_for

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
    HORIZONTAL_BREAKPOINTS: ClassVar[list[tuple[int, str]] | None] = None
    """Breakpoints by width replacing ``App.HORIZONTAL_BREAKPOINTS`` (``None``: the app's)."""
    VERTICAL_BREAKPOINTS: ClassVar[list[tuple[int, str]] | None] = None
    """Breakpoints by height replacing ``App.VERTICAL_BREAKPOINTS`` (``None``: the app's)."""
    COMMANDS: ClassVar[set[Any]] = set()
    """Command palette providers of this screen (added to ``App.COMMANDS``)."""

    def __init__(self, name: str | None = None, id: str | None = None, classes: str | None = None) -> None:
        super().__init__(name=name, id=id, classes=classes)
        self._result_callback: Callable[..., Any] | None = None
        self._screen_title: str | None = self.TITLE
        self._screen_sub_title: str | None = self.SUB_TITLE

    @property
    def title(self) -> str | None:
        """This screen's title, shown by the header instead of ``App.title`` (``None``: the app's)."""
        return self.__dict__.get("_screen_title", self.TITLE)

    @title.setter
    def title(self, value: str | None) -> None:
        self._screen_title = None if value is None else str(value)
        if self.is_mounted:
            native.set_screen_text(self._node, "title", self._screen_title)

    @property
    def sub_title(self) -> str | None:
        """This screen's sub-title (``None``: the app's)."""
        return self.__dict__.get("_screen_sub_title", self.SUB_TITLE)

    @sub_title.setter
    def sub_title(self, value: str | None) -> None:
        self._screen_sub_title = None if value is None else str(value)
        if self.is_mounted:
            native.set_screen_text(self._node, "sub_title", self._screen_sub_title)

    def _rs_on_mount(self) -> None:
        h, v = self.HORIZONTAL_BREAKPOINTS, self.VERTICAL_BREAKPOINTS
        if h is not None or v is not None:
            pairs = lambda bps: None if bps is None else [(int(n), str(c)) for n, c in bps]  # noqa: E731
            native.set_screen_breakpoints(self._node, pairs(h), pairs(v))
        if self.title is not None:
            native.set_screen_text(self._node, "title", self.title)
        if self.sub_title is not None:
            native.set_screen_text(self._node, "sub_title", self.sub_title)
        super()._rs_on_mount()

    @property
    def is_modal(self) -> bool:
        return self._rs_modal

    @property
    def is_current(self) -> bool:
        return self.is_mounted and native.active_screen() == self._node

    @property
    def is_active(self) -> bool:
        """Whether this is the active (top) screen."""
        return self.is_current

    def focus_next(self, selector: Any = "*") -> Widget | None:
        """Focus the next widget in the focus chain; returns the focused widget."""
        native.focus_next(True)
        return self.focused

    def focus_previous(self, selector: Any = "*") -> Widget | None:
        """Focus the previous widget in the focus chain; returns the focused widget."""
        native.focus_next(False)
        return self.focused

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

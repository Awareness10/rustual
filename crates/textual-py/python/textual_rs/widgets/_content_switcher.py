from __future__ import annotations

from typing import ClassVar

from ..widget import AwaitComplete, Widget, _NATIVE_CLASSES, _widget_for


class ContentSwitcher(Widget):
    """A container that displays only the child whose id is :attr:`current`."""

    _rs_kind: ClassVar[str] = "ContentSwitcher"
    _rs_framework: ClassVar[bool] = True

    def __init__(
        self,
        *children: Widget,
        name: str | None = None,
        id: str | None = None,
        classes: str | None = None,
        disabled: bool = False,
        initial: str | None = None,
    ) -> None:
        super().__init__(*children, name=name, id=id, classes=classes, disabled=disabled)
        self._rs_kw = {"initial": initial}

    @property
    def current(self) -> str | None:
        """The id of the visible child, or ``None``."""
        if self.is_mounted:
            return self._native_get("current")
        return self._rs_kw.get("initial")

    @current.setter
    def current(self, value: str | None) -> None:
        if self.is_mounted:
            self._native_set("current", value)
        else:
            self._rs_kw["initial"] = value

    @property
    def visible_content(self) -> Widget | None:
        if not self.is_mounted:
            return None
        return _widget_for(self._native_call("visible_content"))

    def add_content(self, widget: Widget, *, id: str | None = None, set_current: bool = False) -> AwaitComplete:
        """Add a (hidden) child, optionally making it the current one."""
        if id is not None:
            widget.id = id
        if widget.id is None:
            raise ValueError("Widget must have an ID")
        widget.display = False
        self.mount(widget)
        if set_current:
            self.current = widget.id
        return AwaitComplete()


_NATIVE_CLASSES["ContentSwitcher"] = ContentSwitcher

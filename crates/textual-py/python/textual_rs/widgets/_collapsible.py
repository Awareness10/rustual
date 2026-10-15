from __future__ import annotations

from typing import Any, ClassVar

from .._bridge import _native
from ..content import to_markup
from ..message import Message
from ..widget import Widget, _NATIVE_CLASSES


class CollapsibleTitle(Widget):
    """The clickable title of a :class:`Collapsible` (created by it)."""

    _rs_framework: ClassVar[bool] = True


class Collapsible(Widget):
    """A container whose contents can be shown / hidden by clicking its title."""

    _rs_kind: ClassVar[str] = "Collapsible"
    _rs_framework: ClassVar[bool] = True

    class Toggled(Message):
        """Base class for the ``Expanded`` / ``Collapsed`` messages."""

        def __init__(self, collapsible: Collapsible | None = None) -> None:
            super().__init__()
            self._collapsible = collapsible

        @property
        def collapsible(self) -> Collapsible:
            return self._collapsible if self._collapsible is not None else self.sender  # type: ignore[return-value]

        @property
        def control(self) -> Collapsible:
            return self.collapsible

    class Expanded(Toggled):
        """Posted when the collapsible is expanded (``on_collapsible_expanded``)."""

    class Collapsed(Toggled):
        """Posted when the collapsible is collapsed (``on_collapsible_collapsed``)."""

    def __init__(
        self,
        *children: Widget,
        title: Any = "Toggle",
        collapsed: bool = True,
        collapsed_symbol: str = "▶",
        expanded_symbol: str = "▼",
        name: str | None = None,
        id: str | None = None,
        classes: str | None = None,
        disabled: bool = False,
    ) -> None:
        super().__init__(*children, name=name, id=id, classes=classes, disabled=disabled)
        self._rs_kw = {
            "title": to_markup(title)[0],
            "collapsed": collapsed,
            "collapsed_symbol": collapsed_symbol,
            "expanded_symbol": expanded_symbol,
        }

    @property
    def title(self) -> str:
        return self._native_get("title", "Toggle")

    @title.setter
    def title(self, value: Any) -> None:
        self._native_set("title", to_markup(value)[0])

    @property
    def collapsed(self) -> bool:
        return self._native_get("collapsed", True)

    @collapsed.setter
    def collapsed(self, value: bool) -> None:
        self._native_set("collapsed", bool(value))


_native.register_class("Collapsible.Expanded", lambda **kw: Collapsible.Expanded())
_native.register_class("Collapsible.Collapsed", lambda **kw: Collapsible.Collapsed())
_NATIVE_CLASSES["Collapsible"] = Collapsible
_NATIVE_CLASSES["CollapsibleTitle"] = CollapsibleTitle

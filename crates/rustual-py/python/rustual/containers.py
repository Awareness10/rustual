"""Container widgets (Textual's ``textual.containers``)."""

from __future__ import annotations

from typing import ClassVar

from .widget import Widget, _NATIVE_CLASSES


class _ContainerBase(Widget):
    """Base of the containers: like Textual 8's, they accept ``can_focus`` / ``can_focus_children``."""

    _rs_framework = True

    def __init__(
        self,
        *children: Widget,
        name: str | None = None,
        id: str | None = None,
        classes: str | None = None,
        disabled: bool = False,
        can_focus: bool | None = None,
        can_focus_children: bool | None = None,
        can_maximize: bool | None = None,
    ) -> None:
        super().__init__(*children, name=name, id=id, classes=classes, disabled=disabled)
        if can_focus is not None:
            self.can_focus = can_focus
        if can_focus_children is not None:
            self.can_focus_children = can_focus_children


def _container(name: str, doc: str, can_focus: bool | None = None) -> type[Widget]:
    namespace: dict[str, object] = {
        "__doc__": doc,
        "_rs_kind": name,
        "_rs_framework": True,
        "__module__": __name__,
    }
    cls = type(name, (_ContainerBase,), namespace)
    _NATIVE_CLASSES[name] = cls
    return cls


Container = _container("Container", "A simple container (vertical layout, fills its parent).")
Vertical = _container("Vertical", "Children arranged top to bottom.")
VerticalGroup = _container("VerticalGroup", "Vertical layout with auto height.")
Horizontal = _container("Horizontal", "Children arranged left to right.")
HorizontalGroup = _container("HorizontalGroup", "Horizontal layout with auto height.")
Center = _container("Center", "Centers children horizontally.")
Right = _container("Right", "Aligns children to the right.")
Middle = _container("Middle", "Centers children vertically.")
CenterMiddle = _container("CenterMiddle", "Centers children horizontally and vertically.")
Grid = _container("Grid", "Grid layout.")
ItemGrid = _container("ItemGrid", "A grid that sizes its columns to the items.")
ScrollableContainer = _container("ScrollableContainer", "A container that scrolls (focusable).")
VerticalScroll = _container("VerticalScroll", "Vertical layout with vertical scrolling.")
HorizontalScroll = _container("HorizontalScroll", "Horizontal layout with horizontal scrolling.")

__all__ = [
    "Center",
    "CenterMiddle",
    "Container",
    "Grid",
    "Horizontal",
    "HorizontalGroup",
    "HorizontalScroll",
    "ItemGrid",
    "Middle",
    "Right",
    "ScrollableContainer",
    "Vertical",
    "VerticalGroup",
    "VerticalScroll",
]

_ = ClassVar

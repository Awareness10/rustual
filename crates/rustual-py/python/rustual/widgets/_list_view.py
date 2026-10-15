from __future__ import annotations

from typing import Any, ClassVar, Iterable

from .._bridge import _native
from ..message import Message
from ..widget import AwaitComplete, AwaitMount, Widget, _NATIVE_CLASSES, _widget_for


class ListItem(Widget):
    """An item of a :class:`ListView`; its children are ordinary widgets."""

    _rs_kind: ClassVar[str] = "ListItem"
    _rs_framework: ClassVar[bool] = True

    def __init__(
        self,
        *children: Widget,
        name: str | None = None,
        id: str | None = None,
        classes: str | None = None,
        disabled: bool = False,
        markup: bool = True,
    ) -> None:
        super().__init__(*children, name=name, id=id, classes=classes, disabled=disabled, markup=markup)

    @property
    def highlighted(self) -> bool:
        return self.has_class("-highlight")


class ListView(Widget):
    """A vertical list of :class:`ListItem` widgets with a highlighted item."""

    _rs_kind: ClassVar[str] = "ListView"
    _rs_framework: ClassVar[bool] = True

    class _ListMessage(Message):
        def __init__(self, list_view: ListView | None, item: ListItem | None) -> None:
            super().__init__()
            self._list_view = list_view
            self._item = item
            self._item_node: int | None = None

        @property
        def item(self) -> Any:
            if self._item is None and self._item_node is not None:
                self._item = _widget_for(self._item_node)
            return self._item

        @property
        def list_view(self) -> ListView:
            return self._list_view if self._list_view is not None else self.sender  # type: ignore[return-value]

        @property
        def control(self) -> ListView:
            return self.list_view

    class Highlighted(_ListMessage):
        """The highlighted item changed (``item`` is ``None`` when nothing is highlighted)."""

        def __init__(self, list_view: ListView | None, item: ListItem | None) -> None:
            super().__init__(list_view, item)
            self.index: int | None = None

    class Selected(_ListMessage):
        """An item was selected (enter / click)."""

        def __init__(self, list_view: ListView | None, item: ListItem | None, index: int = 0) -> None:
            super().__init__(list_view, item)
            self.index = index

    def __init__(
        self,
        *children: ListItem,
        initial_index: int | None = 0,
        name: str | None = None,
        id: str | None = None,
        classes: str | None = None,
        disabled: bool = False,
    ) -> None:
        super().__init__(*children, name=name, id=id, classes=classes, disabled=disabled)
        self._rs_kw = {"initial_index": initial_index}

    @property
    def _items(self) -> list[ListItem]:
        return [c for c in self.children if isinstance(c, ListItem)]

    def __len__(self) -> int:
        return len(self._items)

    @property
    def index(self) -> int | None:
        if self.is_mounted:
            return self._native_get("index")
        return self._rs_kw.get("initial_index")

    @index.setter
    def index(self, index: int | None) -> None:
        if self.is_mounted:
            if index is not None and index < 0:
                index = 0
            self._native_call("set_index", index)
        else:
            self._rs_kw["initial_index"] = index

    def validate_index(self, index: int | None) -> int | None:
        count = len(self)
        if index is None or not count:
            return None
        return max(0, min(index, count - 1))

    @property
    def highlighted_child(self) -> ListItem | None:
        index = self.index
        items = self._items
        if index is not None and 0 <= index < len(items):
            return items[index]
        return None

    def extend(self, items: Iterable[ListItem]) -> AwaitMount:
        return self.mount(*items)

    def append(self, item: ListItem) -> AwaitMount:
        return self.extend([item])

    def insert(self, index: int, items: Iterable[ListItem]) -> AwaitMount:
        items = list(items)
        existing = self._items
        if index < len(existing):
            return self.mount(*items, before=existing[index])
        return self.mount(*items)

    def clear(self) -> AwaitComplete:
        if self.is_mounted:
            self._native_call("clear")
        else:
            self._pending_children.clear()
        return AwaitComplete()

    def pop(self, index: int | None = None) -> AwaitComplete:
        if self.is_mounted:
            self._native_call("pop", index)
        else:
            self._pending_children.pop(-1 if index is None else index)
        return AwaitComplete()

    def remove_items(self, indices: Iterable[int]) -> AwaitComplete:
        if self.is_mounted:
            self._native_call("remove_items", list(indices))
        else:
            remove = {i % len(self._pending_children) for i in indices}
            self._pending_children[:] = [c for i, c in enumerate(self._pending_children) if i not in remove]
        return AwaitComplete()


def _highlighted(index: int | None, item: int | None) -> ListView.Highlighted:
    message = ListView.Highlighted(None, None)
    message._item_node = item
    message.index = index
    return message


def _selected(index: int, item: int) -> ListView.Selected:
    message = ListView.Selected(None, None, index)
    message._item_node = item
    return message


_native.register_class("ListView.Highlighted", _highlighted)
_native.register_class("ListView.Selected", _selected)
_NATIVE_CLASSES["ListView"] = ListView
_NATIVE_CLASSES["ListItem"] = ListItem

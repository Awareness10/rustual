from __future__ import annotations

from collections.abc import Iterable
from typing import Any, ClassVar, Generic, TypeVar

from .._bridge import _native
from ..message import Message
from ..widget import _NATIVE_CLASSES
from ._helpers import prompt_markup
from ._option_list import Option, OptionList

SelectionType = TypeVar("SelectionType")
"""The type for the value of a ``Selection`` in a ``SelectionList``."""

MessageSelectionType = TypeVar("MessageSelectionType")
"""The type for the value of a ``Selection`` in a ``SelectionList`` message."""


class SelectionError(TypeError):
    """Type of an error raised if a selection is badly-formed."""


class Selection(Generic[SelectionType], Option):
    """A selection for a ``SelectionList``."""

    def __init__(
        self,
        prompt: Any,
        value: SelectionType,
        initial_state: bool = False,
        id: str | None = None,
        disabled: bool = False,
    ) -> None:
        markup = prompt_markup(prompt)
        lines = markup.splitlines()
        super().__init__(lines[0] if lines else "", id, disabled)
        self._value: SelectionType = value
        self._initial_state: bool = initial_state

    @property
    def value(self) -> SelectionType:
        """The value for this selection."""
        return self._value

    @property
    def initial_state(self) -> bool:
        """The initial selected state for the selection."""
        return self._initial_state


class SelectionList(Generic[SelectionType], OptionList):
    """A vertical selection list that allows making multiple selections."""

    _rs_kind: ClassVar[str] = "SelectionList"
    _rs_framework: ClassVar[bool] = True

    COMPONENT_CLASSES: ClassVar[set[str]] = {
        "selection-list--button",
        "selection-list--button-selected",
        "selection-list--button-highlighted",
        "selection-list--button-selected-highlighted",
    }

    class SelectionMessage(Generic[MessageSelectionType], Message):
        """Base class for all selection messages."""

        def __init__(self, selection_list: SelectionList[Any] | None = None, index: int = 0) -> None:
            super().__init__()
            self._selection_list = selection_list
            self._selection: Selection[Any] | None = None
            self.selection_index: int = index

        @property
        def selection_list(self) -> SelectionList[MessageSelectionType]:
            """The selection list that sent the message."""
            return self._selection_list if self._selection_list is not None else self.sender  # type: ignore[return-value]

        @property
        def selection(self) -> Selection[MessageSelectionType]:
            """The selection that the message relates to."""
            if self._selection is None:
                self._selection = self.selection_list.get_option_at_index(self.selection_index)
            return self._selection

        @property
        def control(self) -> OptionList:
            return self.selection_list

        def __repr__(self) -> str:
            return f"{type(self).__qualname__}(selection_index={self.selection_index!r})"

    class SelectionHighlighted(SelectionMessage[MessageSelectionType]):
        """Posted when a selection is highlighted (``on_selection_list_selection_highlighted``)."""

    class SelectionToggled(SelectionMessage[MessageSelectionType]):
        """Posted when a selection is explicitly toggled (``on_selection_list_selection_toggled``)."""

    class SelectedChanged(Generic[MessageSelectionType], Message):
        """Posted when the collection of selected values changes
        (``on_selection_list_selected_changed``)."""

        def __init__(self, selection_list: SelectionList[Any] | None = None, selected: list[Any] | None = None) -> None:
            super().__init__()
            self._selection_list = selection_list
            self._selected = selected

        @property
        def selection_list(self) -> SelectionList[MessageSelectionType]:
            return self._selection_list if self._selection_list is not None else self.sender  # type: ignore[return-value]

        @property
        def control(self) -> SelectionList[MessageSelectionType]:
            return self.selection_list

        def __repr__(self) -> str:
            return f"SelectionList.SelectedChanged(selected={self._selected!r})"

    def __init__(
        self,
        *selections: Selection[SelectionType] | tuple[Any, SelectionType] | tuple[Any, SelectionType, bool],
        name: str | None = None,
        id: str | None = None,
        classes: str | None = None,
        disabled: bool = False,
        compact: bool = False,
    ) -> None:
        super().__init__(*selections, name=name, id=id, classes=classes, disabled=disabled, compact=compact)

    # -- native plumbing -------------------------------------------------------------------

    def _make_option(self, content: Any) -> Selection[SelectionType]:
        if isinstance(content, Selection):
            return content
        if not isinstance(content, tuple) or len(content) not in (2, 3):
            raise SelectionError(
                "Expected a Selection or a tuple of (prompt, value) or (prompt, value, initial_state)."
            )
        return Selection(*content)

    def _item(self, content: Any) -> Any:
        s: Selection[Any] = content
        return (prompt_markup(s.prompt), s.value, bool(s.initial_state), s.id, bool(s.disabled))

    def _rs_kwargs(self) -> dict[str, Any]:
        kw = super()._rs_kwargs()
        kw["selections"] = kw.pop("options")
        return kw

    def add_options(self, items: Iterable[Any]) -> SelectionList[SelectionType]:
        """Add new selection options (``Selection`` objects or tuples)."""
        items = list(items)
        if any(item is None for item in items):
            raise SelectionError("Separators can't be added to a SelectionList.")
        super().add_options([self._make_option(item) for item in items])
        return self

    def add_option(self, item: Any = None) -> SelectionList[SelectionType]:
        """Add a new selection option."""
        return self.add_options([item])

    def replace_option_prompt_at_index(self, index: int, prompt: Any) -> OptionList:
        raise NotImplementedError("SelectionList doesn't support replacing prompts")

    def scroll_to_highlight(self, top: bool = False) -> None:
        pass

    # -- selection state -------------------------------------------------------------------

    @property
    def selected(self) -> list[SelectionType]:
        """The selected values (in the order they were selected)."""
        if self.is_mounted:
            return list(self._native_get("selected"))
        return [s.value for s in self._options if s.initial_state]  # type: ignore[attr-defined]

    @staticmethod
    def _value(selection: Any) -> Any:
        return selection.value if isinstance(selection, Selection) else selection

    def _pre_mount_set(self, value: Any, state: bool | None) -> None:
        for s in self._options:
            if s.value == value:  # type: ignore[attr-defined]
                s._initial_state = (not s._initial_state) if state is None else state  # type: ignore[attr-defined]

    def select(self, selection: Selection[SelectionType] | SelectionType) -> SelectionList[SelectionType]:
        """Mark the given selection as selected."""
        if self.is_mounted:
            self._native_call("select", self._value(selection))
        else:
            self._pre_mount_set(self._value(selection), True)
        return self

    def deselect(self, selection: Selection[SelectionType] | SelectionType) -> SelectionList[SelectionType]:
        """Mark the given selection as not selected."""
        if self.is_mounted:
            self._native_call("deselect", self._value(selection))
        else:
            self._pre_mount_set(self._value(selection), False)
        return self

    def toggle(self, selection: Selection[SelectionType] | SelectionType) -> SelectionList[SelectionType]:
        """Toggle the selected state of the given selection."""
        if self.is_mounted:
            self._native_call("toggle", self._value(selection))
        else:
            self._pre_mount_set(self._value(selection), None)
        return self

    def _all(self, method: str, state: bool | None) -> SelectionList[SelectionType]:
        if self.is_mounted:
            self._native_call(method)
        else:
            for s in self._options:
                self._pre_mount_set(s.value, state)  # type: ignore[attr-defined]
        return self

    def select_all(self) -> SelectionList[SelectionType]:
        """Select all items."""
        return self._all("select_all", True)

    def deselect_all(self) -> SelectionList[SelectionType]:
        """Deselect all items."""
        return self._all("deselect_all", False)

    def toggle_all(self) -> SelectionList[SelectionType]:
        """Toggle all items."""
        return self._all("toggle_all", None)


def _selection_message(cls: type[SelectionList.SelectionMessage[Any]]) -> Any:
    return lambda index=0: cls(None, index)


_native.register_class("SelectionList.SelectionHighlighted", _selection_message(SelectionList.SelectionHighlighted))
_native.register_class("SelectionList.SelectionToggled", _selection_message(SelectionList.SelectionToggled))
_native.register_class(
    "SelectionList.SelectedChanged", lambda selected=None: SelectionList.SelectedChanged(None, selected)
)
_NATIVE_CLASSES["SelectionList"] = SelectionList

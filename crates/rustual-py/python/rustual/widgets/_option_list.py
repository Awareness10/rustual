from __future__ import annotations

from collections.abc import Iterable, Sequence
from typing import Any, ClassVar

from .._bridge import _native
from ..message import Message
from ..widget import _NATIVE_CLASSES, Widget
from ._helpers import compact_property, prompt_markup


class OptionListError(Exception):
    """An error with the option list."""


class DuplicateID(OptionListError):
    """Raised if a duplicate ID is used when adding options to an option list."""


class OptionDoesNotExist(OptionListError):
    """Raised when a request has been made for an option that doesn't exist."""


class Option:
    """This class holds details of options in the list."""

    def __init__(self, prompt: Any, id: str | None = None, disabled: bool = False) -> None:
        self._prompt = prompt
        self._id = id
        self.disabled = disabled
        self._divider = False

    @property
    def prompt(self) -> Any:
        """The original prompt."""
        return self._prompt

    @property
    def id(self) -> str | None:
        """Optional ID for the option."""
        return self._id

    def _set_prompt(self, prompt: Any) -> None:
        self._prompt = prompt

    def __hash__(self) -> int:
        return id(self)

    def __repr__(self) -> str:
        args = [repr(self._prompt)]
        if self._id is not None:
            args.append(f"id={self._id!r}")
        if self.disabled:
            args.append("disabled=True")
        return f"{type(self).__name__}({', '.join(args)})"


OptionListContent = Option | Any | None
"""The type of an item of content in the option list (``None`` adds a separator)."""


class OptionList(Widget):
    """A navigable list of options (``ScrollView``-based in Textual)."""

    _rs_kind: ClassVar[str] = "OptionList"
    _rs_framework: ClassVar[bool] = True
    can_focus: bool = True

    class OptionMessage(Message):
        """Base class for all option messages."""

        def __init__(
            self,
            option_list: OptionList | None = None,
            option: Option | None = None,
            index: int = 0,
            option_id: str | None = None,
        ) -> None:
            super().__init__()
            self._option_list = option_list
            self._option = option
            self.option_id: str | None = option.id if option is not None else option_id
            self.option_index: int = index

        @property
        def option_list(self) -> OptionList:
            """The option list that sent the message."""
            return self._option_list if self._option_list is not None else self.sender  # type: ignore[return-value]

        @property
        def option(self) -> Option:
            """The option that the message relates to."""
            if self._option is None:
                self._option = self.option_list._option_at(self.option_index, self.option_id)
            return self._option

        @property
        def control(self) -> OptionList:
            return self.option_list

        def __repr__(self) -> str:
            return f"{type(self).__qualname__}(option_index={self.option_index!r}, option_id={self.option_id!r})"

    class OptionHighlighted(OptionMessage):
        """Posted when an option is highlighted (``on_option_list_option_highlighted``)."""

    class OptionSelected(OptionMessage):
        """Posted when an option is selected (``on_option_list_option_selected``)."""

    def __init__(
        self,
        *content: OptionListContent,
        name: str | None = None,
        id: str | None = None,
        classes: str | None = None,
        disabled: bool = False,
        markup: bool = True,
        compact: bool = False,
    ) -> None:
        super().__init__(name=name, id=id, classes=classes, disabled=disabled, markup=markup)
        self._rs_kw = {"compact": compact}
        self.compact = compact
        self._options: list[Option] = []
        self._id_to_option: dict[str, Option] = {}
        self._pre_highlight: int | None = None
        self.add_options(content)
        if self._options:
            self._pre_highlight = next((i for i, o in enumerate(self._options) if not o.disabled), None)

    compact = compact_property()

    # -- native plumbing -------------------------------------------------------------------

    def _item(self, content: Any) -> Any:
        """Native form of one content item."""
        if content is None:
            return None
        markup = self.__dict__.get("_markup", True)
        return (prompt_markup(content.prompt, markup), content.id, bool(content.disabled))

    def _make_option(self, content: Any) -> Option:
        return content if isinstance(content, Option) else Option(content)

    def _native_items(self) -> list[Any]:
        items: list[Any] = []
        for option in self._options:
            items.append(self._item(option))
            if option._divider:
                items.append(None)
        return items

    def _rs_kwargs(self) -> dict[str, Any]:
        kw = super()._rs_kwargs()
        kw["options"] = self._native_items()
        return kw

    def _rs_on_mount(self) -> None:
        super()._rs_on_mount()
        if self._pre_highlight is not None and self._native_get("highlighted") != self._pre_highlight:
            self._native_set("highlighted", self._pre_highlight)
        self._pre_highlight = None

    def _option_at(self, index: int, option_id: str | None = None) -> Option:
        options = self.__dict__.get("_options")
        if options is not None:
            # Messages are resolved when handled; the list may have changed since.
            if 0 <= index < len(options) and (option_id is None or options[index].id == option_id):
                return options[index]
            if option_id is not None and option_id in self._id_to_option:
                return self._id_to_option[option_id]
            return Option("", option_id)
        # A list created by Rust code (no Python mirror): build an option from native data.
        prompt, oid, disabled = self._native_get("options", [])[index]
        return Option(prompt, oid, disabled)

    # -- options ---------------------------------------------------------------------------

    @property
    def options(self) -> Sequence[Option]:
        """Sequence of options in the OptionList (read-only)."""
        return self._options

    @property
    def option_count(self) -> int:
        """The number of options."""
        return len(self._options)

    @property
    def highlighted(self) -> int | None:
        """The index of the highlighted option, or ``None``."""
        if not self.is_mounted:
            return self._pre_highlight
        return self._native_get("highlighted")

    @highlighted.setter
    def highlighted(self, value: int | None) -> None:
        if value is not None:
            value = max(0, min(int(value), len(self._options) - 1)) if self._options else None
        if self.is_mounted:
            self._native_set("highlighted", value)
        else:
            self._pre_highlight = value

    @property
    def highlighted_option(self) -> Option | None:
        """The highlighted option, or ``None``."""
        h = self.highlighted
        return None if h is None else self._options[h]

    def clear_options(self) -> OptionList:
        """Clear the content of the option list."""
        self._options.clear()
        self._id_to_option.clear()
        self._pre_highlight = None
        if self.is_mounted:
            self._native_call("clear_options")
        return self

    def set_options(self, options: Iterable[OptionListContent]) -> OptionList:
        """Set options, clearing existing options."""
        self.clear_options()
        return self.add_options(options)

    def add_options(self, new_options: Iterable[OptionListContent]) -> OptionList:
        """Add new options (``None`` adds a separator after the previous option)."""
        new_options = list(new_options)
        ids = [o.id for o in new_options if isinstance(o, Option) and o.id is not None]
        if len(ids) != len(set(ids)):
            raise DuplicateID("New options contain duplicated IDs; Ensure that the IDs are unique.")
        for option_id in ids:
            if option_id in self._id_to_option:
                raise DuplicateID(f"Unable to add option with duplicate ID {option_id!r}")
        added: list[Any] = []
        for content in new_options:
            if content is None:
                if self._options:
                    self._options[-1]._divider = True
                added.append(None)
                continue
            option = self._make_option(content)
            if option.id is not None:
                self._id_to_option[option.id] = option
            self._options.append(option)
            added.append(self._item(option))
        if self.is_mounted and added:
            self._native_call("add_options", added)
        return self

    def add_option(self, option: OptionListContent = None) -> OptionList:
        """Add a new option to the end of the list (``None`` for a separator)."""
        return self.add_options([option])

    def get_option(self, option_id: str) -> Option:
        """Get the option with the given ID."""
        try:
            return self._id_to_option[option_id]
        except KeyError:
            raise OptionDoesNotExist(f"There is no option with an ID of {option_id!r}") from None

    def get_option_index(self, option_id: str) -> int:
        """Get the index of the option with the given ID."""
        option = self.get_option(option_id)
        return self._options.index(option)

    def get_option_at_index(self, index: int) -> Option:
        """Get the option at the given index."""
        try:
            return self._options[index]
        except IndexError:
            raise OptionDoesNotExist(f"There is no option with an index of {index}") from None

    def _set_option_disabled(self, index: int, disabled: bool) -> OptionList:
        option = self.get_option_at_index(index)
        option.disabled = disabled
        if self.is_mounted:
            self._native_call("set_option_disabled", index, disabled)
        return self

    def enable_option_at_index(self, index: int) -> OptionList:
        """Enable the option at the given index."""
        return self._set_option_disabled(index, False)

    def disable_option_at_index(self, index: int) -> OptionList:
        """Disable the option at the given index."""
        return self._set_option_disabled(index, True)

    def enable_option(self, option_id: str) -> OptionList:
        """Enable the option with the given ID."""
        return self.enable_option_at_index(self.get_option_index(option_id))

    def disable_option(self, option_id: str) -> OptionList:
        """Disable the option with the given ID."""
        return self.disable_option_at_index(self.get_option_index(option_id))

    def remove_option(self, option_id: str) -> OptionList:
        """Remove the option with the given ID."""
        return self.remove_option_at_index(self.get_option_index(option_id))

    def remove_option_at_index(self, index: int) -> OptionList:
        """Remove the option at the given index."""
        option = self.get_option_at_index(index)
        if self.is_mounted:
            self._native_call("remove_option_at_index", index)
        del self._options[index]
        if option.id is not None:
            self._id_to_option.pop(option.id, None)
        return self

    def replace_option_prompt(self, option_id: str, prompt: Any) -> OptionList:
        """Replace the prompt of the option with the given ID."""
        return self.replace_option_prompt_at_index(self.get_option_index(option_id), prompt)

    def replace_option_prompt_at_index(self, index: int, prompt: Any) -> OptionList:
        """Replace the prompt of the option at the given index."""
        option = self.get_option_at_index(index)
        option._set_prompt(prompt)
        if self.is_mounted:
            self._native_call(
                "replace_option_prompt_at_index", index, prompt_markup(prompt, self.__dict__.get("_markup", True))
            )
        return self

    def scroll_to_highlight(self, top: bool = False) -> None:
        """Scroll so the highlighted option is visible."""
        if self.is_mounted:
            self._native_call("scroll_to_highlight")

    # -- actions ---------------------------------------------------------------------------

    def action_cursor_up(self) -> None:
        self._native_call("run_action", "cursor_up")

    def action_cursor_down(self) -> None:
        self._native_call("run_action", "cursor_down")

    def action_first(self) -> None:
        if self.is_mounted:
            self._native_call("run_action", "first")
        else:
            self._pre_highlight = next((i for i, o in enumerate(self._options) if not o.disabled), None)

    def action_last(self) -> None:
        self._native_call("run_action", "last")

    def action_page_up(self) -> None:
        self._native_call("run_action", "page_up")

    def action_page_down(self) -> None:
        self._native_call("run_action", "page_down")

    def action_select(self) -> None:
        self._native_call("run_action", "select")


def _option_message(cls: type[OptionList.OptionMessage]) -> Any:
    return lambda index=0, option_id=None: cls(None, None, index, option_id)


_native.register_class("OptionList.OptionHighlighted", _option_message(OptionList.OptionHighlighted))
_native.register_class("OptionList.OptionSelected", _option_message(OptionList.OptionSelected))
_NATIVE_CLASSES["OptionList"] = OptionList

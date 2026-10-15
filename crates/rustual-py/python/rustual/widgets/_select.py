from __future__ import annotations

from typing import Any, ClassVar, Generic, Hashable, Iterable, TypeVar

from .._bridge import _native
from ..message import Message
from ..widget import Widget, _NATIVE_CLASSES
from ._helpers import compact_property, prompt_markup


class NoSelection:
    """Used by the ``Select`` widget to flag the unselected state (``Select.NULL``)."""

    def __repr__(self) -> str:
        return "Select.NULL"


NULL = NoSelection()


class InvalidSelectValueError(Exception):
    """Raised when setting a ``Select`` to an unknown option."""


class EmptySelectError(Exception):
    """Raised when a ``Select`` has no options and ``allow_blank=False``."""


SelectType = TypeVar("SelectType", bound=Hashable)


class Select(Generic[SelectType], Widget):
    """Widget to select from a list of possible options (a drop-down shown in an overlay)."""

    _rs_kind: ClassVar[str] = "Select"
    _rs_framework: ClassVar[bool] = True
    can_focus: bool = True

    NULL = NULL
    """Constant to flag that the widget has no selection."""
    BLANK = NULL
    """Alias of ``NULL`` (older Textual versions)."""

    class Changed(Message):
        """Posted when the select value was changed (``on_select_changed``)."""

        def __init__(self, select: Select[Any] | None = None, value: Any = NULL) -> None:
            super().__init__()
            self._select = select
            self.value = value

        @property
        def select(self) -> Select[Any]:
            return self._select if self._select is not None else self.sender  # type: ignore[return-value]

        @property
        def control(self) -> Select[Any]:
            return self.select

        def __repr__(self) -> str:
            return f"Select.Changed(value={self.value!r})"

    def __init__(
        self,
        options: Iterable[tuple[Any, SelectType]],
        *,
        prompt: str = "Select",
        allow_blank: bool = True,
        value: SelectType | NoSelection = NULL,
        type_to_search: bool = True,
        name: str | None = None,
        id: str | None = None,
        classes: str | None = None,
        disabled: bool = False,
        tooltip: str | None = None,
        compact: bool = False,
    ) -> None:
        super().__init__(name=name, id=id, classes=classes, disabled=disabled)
        self._allow_blank = allow_blank
        self._type_to_search = type_to_search
        self._setup_options(options)
        if value is not NULL and value not in self._legal_values:
            raise InvalidSelectValueError(f"Illegal select value {value!r}.")
        self._rs_kw = {
            "options": self._native_options(),
            "prompt": prompt,
            "allow_blank": allow_blank,
            "value": None if value is NULL else value,
        }
        if not allow_blank and value is NULL:
            self._rs_kw["value"] = self._options[0][1]
        if tooltip is not None:
            self.tooltip = tooltip
        self.compact = compact

    @classmethod
    def from_values(
        cls,
        values: Iterable[SelectType],
        *,
        prompt: str = "Select",
        allow_blank: bool = True,
        value: SelectType | NoSelection = NULL,
        type_to_search: bool = True,
        name: str | None = None,
        id: str | None = None,
        classes: str | None = None,
        disabled: bool = False,
        compact: bool = False,
    ) -> Select[SelectType]:
        """A ``Select`` whose option prompts are ``str(value)``."""
        return cls(
            [(str(v), v) for v in values],
            prompt=prompt,
            allow_blank=allow_blank,
            value=value,
            type_to_search=type_to_search,
            name=name,
            id=id,
            classes=classes,
            disabled=disabled,
            compact=compact,
        )

    def _setup_options(self, options: Iterable[tuple[Any, SelectType]]) -> None:
        self._options: list[tuple[Any, Any]] = list(options)
        if not self._options and not self._allow_blank:
            raise EmptySelectError("Select options cannot be empty if selection can't be blank.")
        self._legal_values = [v for _, v in self._options]

    def _native_options(self) -> list[tuple[str, Any]]:
        return [(prompt_markup(p), v) for p, v in self._options]

    compact = compact_property()

    @property
    def value(self) -> SelectType | NoSelection:
        """The selected value, or ``Select.NULL``."""
        v = self._native_get("value")
        return NULL if v is None else v

    @value.setter
    def value(self, value: SelectType | NoSelection) -> None:
        if value is NULL or value is None:
            if not self._allow_blank:
                raise InvalidSelectValueError("Can't clear selection if allow_blank is set to False.")
            self._native_set("value", None)
            return
        if value not in self._legal_values:
            raise InvalidSelectValueError(f"Illegal select value {value!r}.")
        self._native_set("value", value)

    @property
    def selection(self) -> SelectType | None:
        """The selected value, or ``None`` (never ``NULL``)."""
        value = self.value
        return None if isinstance(value, NoSelection) else value

    @property
    def prompt(self) -> str:
        """The prompt shown when no value is selected."""
        return self._native_get("prompt", "Select")

    @prompt.setter
    def prompt(self, prompt: str) -> None:
        self._native_set("prompt", str(prompt))

    @property
    def expanded(self) -> bool:
        """Whether the overlay (drop-down) is shown."""
        return bool(self._native_get("expanded", False))

    @expanded.setter
    def expanded(self, expanded: bool) -> None:
        if expanded and self.is_mounted and not self.expanded:
            self._native_call("show_overlay")

    def set_options(self, options: Iterable[tuple[Any, SelectType]]) -> None:
        """Set the options (the selection is reset)."""
        self._setup_options(options)
        if self.is_mounted:
            self._native_call("set_options", self._native_options())
        else:
            self._rs_kw["options"] = self._native_options()
            self._rs_kw["value"] = None if self._allow_blank else self._options[0][1]

    def is_blank(self) -> bool:
        """Whether nothing is selected."""
        return self.value is NULL

    def clear(self) -> None:
        """Clear the selection (requires ``allow_blank``)."""
        self.value = NULL

    def action_show_overlay(self) -> None:
        """Show the overlay."""
        self._native_call("show_overlay")


def _changed(value: Any = None) -> Select.Changed:
    return Select.Changed(None, NULL if value is None else value)


_native.register_class("Select.Changed", _changed)
_NATIVE_CLASSES["Select"] = Select

from __future__ import annotations

from dataclasses import dataclass
from typing import Any, ClassVar, Literal, NamedTuple, Tuple

from .._bridge import _native
from ..content import Content
from ..message import Message
from ..widget import Widget, _NATIVE_CLASSES

Location = Tuple[int, int]
"""A location (row, column) within a document."""

BUILTIN_LANGUAGES = ["python", "rust", "javascript", "json", "toml"]
"""Languages with built-in (keyword-based) highlighting."""


class ThemeDoesNotExist(Exception):
    """Raised when the user tries to use a theme which does not exist."""


class LanguageDoesNotExist(Exception):
    """Raised when the user tries to use a language which does not exist."""


class Selection(NamedTuple):
    """A range of characters from ``start`` (the anchor) to ``end`` (the cursor)."""

    start: Location = (0, 0)
    end: Location = (0, 0)

    @classmethod
    def cursor(cls, location: Location) -> Selection:
        """A zero-width selection at ``location``."""
        return cls(location, location)

    @property
    def is_empty(self) -> bool:
        start, end = self
        return start == end


@dataclass
class EditResult:
    """Information about a completed edit."""

    end_location: Location
    """The location at the end of the inserted text."""
    replaced_text: str
    """The text that was replaced."""


class Document:
    """A snapshot of a text document (lines plus newline style).

    ``TextArea.document`` returns a snapshot of the current text; edit through the
    ``TextArea`` methods.
    """

    def __init__(self, text: str) -> None:
        self._newline = "\r\n" if "\r\n" in text else ("\r" if "\r" in text and "\n" not in text else "\n")
        lines = text.splitlines()
        if text.endswith(("\n", "\r")) or not text:
            lines.append("")
        self._lines = lines

    @property
    def text(self) -> str:
        return self._newline.join(self._lines)

    @property
    def newline(self) -> str:
        return self._newline

    @property
    def lines(self) -> list[str]:
        return list(self._lines)

    @property
    def line_count(self) -> int:
        return len(self._lines)

    @property
    def start(self) -> Location:
        return (0, 0)

    @property
    def end(self) -> Location:
        return (len(self._lines) - 1, len(self._lines[-1]))

    def get_line(self, index: int) -> str:
        return self._lines[index]

    def __getitem__(self, index: int) -> str:
        return self._lines[index]

    def get_size(self, tab_width: int) -> tuple[int, int]:
        width = max((len(line.expandtabs(tab_width)) for line in self._lines), default=0)
        return (width, len(self._lines))

    def get_index_from_location(self, location: Location) -> int:
        row, column = location
        return sum(len(line) + len(self._newline) for line in self._lines[:row]) + column

    def get_location_from_index(self, index: int) -> Location:
        for row, line in enumerate(self._lines):
            if index <= len(line):
                return (row, index)
            index -= len(line) + len(self._newline)
        return self.end

    def get_text_range(self, start: Location, end: Location) -> str:
        if start > end:
            start, end = end, start
        (r0, c0), (r1, c1) = start, end
        if r0 == r1:
            return self._lines[r0][c0:c1]
        parts = [self._lines[r0][c0:], *self._lines[r0 + 1 : r1], self._lines[r1][:c1]]
        return self._newline.join(parts)


def _attr(name: str, default: Any, *, native_get: bool = True, native_set: bool = True) -> property:
    """A property backed by the native widget (falling back to the constructor kwargs)."""

    def fget(self: TextArea) -> Any:
        if native_get:
            return self._native_get(name, default)
        return self.__dict__.get("_rs_kw", {}).get(name, default)

    def fset(self: TextArea, value: Any) -> None:
        self.__dict__.setdefault("_rs_kw", {})[name] = value
        if native_set and self.is_mounted:
            self._native_set(name, value)

    return property(fget, fset)


class TextArea(Widget):
    """A multi-line text editor."""

    _rs_kind: ClassVar[str] = "TextArea"
    _rs_framework: ClassVar[bool] = True

    class Changed(Message):
        """Posted when the text changes (``on_text_area_changed``)."""

        def __init__(self, text_area: TextArea | None = None) -> None:
            super().__init__()
            self._text_area = text_area

        @property
        def text_area(self) -> TextArea:
            return self._text_area if self._text_area is not None else self.sender  # type: ignore[return-value]

        @property
        def control(self) -> TextArea:
            return self.text_area

    class SelectionChanged(Message):
        """Posted when the selection or cursor moves (``on_text_area_selection_changed``)."""

        def __init__(self, selection: Selection, text_area: TextArea | None = None) -> None:
            super().__init__()
            self.selection = Selection(*selection)
            self._text_area = text_area

        @property
        def text_area(self) -> TextArea:
            return self._text_area if self._text_area is not None else self.sender  # type: ignore[return-value]

        @property
        def control(self) -> TextArea:
            return self.text_area

    def __init__(
        self,
        text: str = "",
        *,
        language: str | None = None,
        theme: str = "css",
        soft_wrap: bool = True,
        tab_behavior: Literal["focus", "indent"] = "focus",
        read_only: bool = False,
        show_cursor: bool = True,
        show_line_numbers: bool = False,
        line_number_start: int = 1,
        max_checkpoints: int = 50,
        name: str | None = None,
        id: str | None = None,
        classes: str | None = None,
        disabled: bool = False,
        tooltip: Any = None,
        compact: bool = False,
        highlight_cursor_line: bool = True,
        placeholder: Any = "",
    ) -> None:
        super().__init__(name=name, id=id, classes=classes, disabled=disabled)
        self._rs_kw = {
            "text": text,
            "language": language,
            "theme": theme,
            "soft_wrap": soft_wrap,
            "tab_behavior": tab_behavior,
            "read_only": read_only,
            "show_cursor": show_cursor,
            "show_line_numbers": show_line_numbers,
            "line_number_start": line_number_start,
            "max_checkpoints": max_checkpoints,
            "compact": compact,
            "highlight_cursor_line": highlight_cursor_line,
            "placeholder": str(placeholder),
        }
        if compact:
            self.add_class("-textual-compact")
        if read_only:
            self.add_class("-read-only")
        if tooltip is not None:
            self.tooltip = tooltip

    @classmethod
    def code_editor(
        cls,
        text: str = "",
        *,
        language: str | None = None,
        theme: str = "monokai",
        soft_wrap: bool = False,
        tab_behavior: Literal["focus", "indent"] = "indent",
        read_only: bool = False,
        show_cursor: bool = True,
        show_line_numbers: bool = True,
        line_number_start: int = 1,
        max_checkpoints: int = 50,
        name: str | None = None,
        id: str | None = None,
        classes: str | None = None,
        disabled: bool = False,
        tooltip: Any = None,
        compact: bool = False,
        highlight_cursor_line: bool = True,
        placeholder: Any = "",
    ) -> TextArea:
        """A text area configured for code: line numbers, no soft wrap, tab indents."""
        return cls(
            text,
            language=language,
            theme=theme,
            soft_wrap=soft_wrap,
            tab_behavior=tab_behavior,
            read_only=read_only,
            show_cursor=show_cursor,
            show_line_numbers=show_line_numbers,
            line_number_start=line_number_start,
            max_checkpoints=max_checkpoints,
            name=name,
            id=id,
            classes=classes,
            disabled=disabled,
            tooltip=tooltip,
            compact=compact,
            highlight_cursor_line=highlight_cursor_line,
            placeholder=placeholder,
        )

    # -- configuration -----------------------------------------------------------------------

    soft_wrap = _attr("soft_wrap", True)
    read_only = _attr("read_only", False)
    show_line_numbers = _attr("show_line_numbers", False)
    indent_width = _attr("indent_width", 4)
    tab_behavior = _attr("tab_behavior", "focus")
    match_cursor_bracket = _attr("match_cursor_bracket", True, native_get=False)
    cursor_blink = _attr("cursor_blink", True, native_get=False)
    highlight_cursor_line = _attr("highlight_cursor_line", True, native_get=False)
    indent_type = _attr("indent_type", "spaces", native_get=False)
    max_checkpoints = _attr("max_checkpoints", 50, native_get=False)
    # Constructor-only in the engine (changing them after mount has no effect).
    show_cursor = _attr("show_cursor", True, native_get=False, native_set=False)
    line_number_start = _attr("line_number_start", 1, native_get=False, native_set=False)

    @property
    def language(self) -> str | None:
        return self._native_get("language")

    @language.setter
    def language(self, value: str | None) -> None:
        self._native_set("language", value)

    @property
    def theme(self) -> str:
        return self._native_get("theme", "css")

    @theme.setter
    def theme(self, value: str) -> None:
        if value not in self.available_themes:
            raise ThemeDoesNotExist(f"{value!r} is not a builtin theme, or it has not been registered.")
        self._native_set("theme", value)

    @property
    def placeholder(self) -> str:
        return self._native_get("placeholder", "")

    @placeholder.setter
    def placeholder(self, value: Any) -> None:
        self._native_set("placeholder", str(value))

    @property
    def compact(self) -> bool:
        return self.has_class("-textual-compact")

    @compact.setter
    def compact(self, value: bool) -> None:
        self.set_class(bool(value), "-textual-compact")

    @property
    def available_themes(self) -> set[str]:
        if self.is_mounted:
            return set(self._native_get("available_themes"))
        return {"css", "monokai", "dracula", "github_light", "vscode_dark"}

    @property
    def available_languages(self) -> set[str]:
        return set(BUILTIN_LANGUAGES)

    # -- text ------------------------------------------------------------------------------------

    @property
    def text(self) -> str:
        """The entire text."""
        return self._native_get("text", "")

    @text.setter
    def text(self, value: str) -> None:
        self.load_text(value)

    def load_text(self, text: str) -> None:
        """Replace the text, clearing the undo history."""
        if self.is_mounted:
            self._native_call("load_text", text)
        else:
            self._rs_kw["text"] = text

    @property
    def document(self) -> Document:
        """A snapshot of the current document."""
        return Document(self.text)

    def get_line(self, index: int) -> Content:
        return Content(self._native_call("get_line", index))

    def get_text_range(self, start: Location, end: Location) -> str:
        return self._native_call("get_text_range", tuple(start), tuple(end))

    # -- selection & cursor ------------------------------------------------------------------

    @property
    def selection(self) -> Selection:
        if not self.is_mounted:
            return Selection()
        start, end = self._native_get("selection")
        return Selection(tuple(start), tuple(end))

    @selection.setter
    def selection(self, value: tuple[Location, Location]) -> None:
        start, end = value
        self._native_set("selection", (tuple(start), tuple(end)))

    @property
    def selected_text(self) -> str:
        return self._native_get("selected_text", "")

    @property
    def cursor_location(self) -> Location:
        if not self.is_mounted:
            return (0, 0)
        return tuple(self._native_get("cursor_location"))  # type: ignore[return-value]

    @cursor_location.setter
    def cursor_location(self, location: Location) -> None:
        self.move_cursor(location)

    @property
    def matching_bracket_location(self) -> Location | None:
        loc = self._native_get("matching_bracket_location")
        return tuple(loc) if loc is not None else None  # type: ignore[return-value]

    @property
    def gutter_width(self) -> int:
        return self._native_get("gutter_width", 0)

    @property
    def cursor_at_first_line(self) -> bool:
        return self._native_get("cursor_at_first_line", True)

    @property
    def cursor_at_last_line(self) -> bool:
        return self._native_get("cursor_at_last_line", True)

    @property
    def cursor_at_start_of_line(self) -> bool:
        return self._native_get("cursor_at_start_of_line", True)

    @property
    def cursor_at_end_of_line(self) -> bool:
        return self._native_get("cursor_at_end_of_line", True)

    @property
    def cursor_at_start_of_text(self) -> bool:
        return self._native_get("cursor_at_start_of_text", True)

    @property
    def cursor_at_end_of_text(self) -> bool:
        return self._native_get("cursor_at_end_of_text", True)

    def move_cursor(self, location: Location, select: bool = False, center: bool = False, record_width: bool = True) -> None:
        self._native_call("move_cursor", tuple(location), select)

    def move_cursor_relative(
        self, rows: int = 0, columns: int = 0, select: bool = False, center: bool = False, record_width: bool = True
    ) -> None:
        self._native_call("move_cursor_relative", rows, columns, select)

    def select_line(self, index: int) -> None:
        self._native_call("select_line", index)

    def select_all(self) -> None:
        self._native_call("select_all")

    def scroll_cursor_visible(self, center: bool = False, animate: bool = False) -> None:
        self._native_call("scroll_cursor_visible")

    # -- editing ---------------------------------------------------------------------------------

    def insert(self, text: str, location: Location | None = None, *, maintain_selection_offset: bool = True) -> EditResult:
        """Insert ``text`` at ``location`` (default: the cursor)."""
        end, replaced = self._native_call("insert", text, tuple(location) if location is not None else None)
        return EditResult(tuple(end), replaced)  # type: ignore[arg-type]

    def delete(self, start: Location, end: Location, *, maintain_selection_offset: bool = True) -> EditResult:
        """Delete the text between two locations."""
        e, replaced = self._native_call("delete", tuple(start), tuple(end))
        return EditResult(tuple(e), replaced)  # type: ignore[arg-type]

    def replace(self, insert: str, start: Location, end: Location, *, maintain_selection_offset: bool = True) -> EditResult:
        """Replace the text between two locations with ``insert``."""
        e, replaced = self._native_call("replace", insert, tuple(start), tuple(end))
        return EditResult(tuple(e), replaced)  # type: ignore[arg-type]

    def clear(self) -> EditResult:
        """Delete all text."""
        e, replaced = self._native_call("clear")
        return EditResult(tuple(e), replaced)  # type: ignore[arg-type]

    def undo(self) -> None:
        self._native_call("undo")

    def redo(self) -> None:
        self._native_call("redo")

    @property
    def can_undo(self) -> bool:
        return self._native_get("can_undo", False)

    @property
    def can_redo(self) -> bool:
        return self._native_get("can_redo", False)


_native.register_class("TextArea.Changed", lambda **kw: TextArea.Changed())
_native.register_class("TextArea.SelectionChanged", lambda **kw: TextArea.SelectionChanged(kw["selection"]))
_NATIVE_CLASSES["TextArea"] = TextArea

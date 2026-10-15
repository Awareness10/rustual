from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Callable, ClassVar, Iterable, Literal, NamedTuple

from .._bridge import _native
from ..coordinate import Coordinate
from ..message import Message
from ..widget import Widget, _NATIVE_CLASSES
from ._detached import DetachedNative, markup_of

CursorType = Literal["cell", "row", "column", "none"]
CellType = Any

_AUTO_PREFIX = "\0auto-"


class CellDoesNotExist(Exception):
    """The cell key / index doesn't exist."""


class RowDoesNotExist(Exception):
    """The row key / index doesn't exist."""


class ColumnDoesNotExist(Exception):
    """The column key / index doesn't exist."""


class DuplicateKey(Exception):
    """A row or column with this key already exists."""


class StringKey:
    """A key that optionally wraps a string; lookups with the string behave the same."""

    value: str | None

    def __init__(self, value: str | None = None) -> None:
        self.value = value
        self._rs = value

    @classmethod
    def _from_rs(cls, rs: str) -> Any:
        key = cls.__new__(cls)
        key._rs = rs
        key.value = None if rs.startswith(_AUTO_PREFIX) else rs
        return key

    def __hash__(self) -> int:
        return hash(self._rs) if self._rs is not None else id(self)

    def __eq__(self, other: object) -> bool:
        if isinstance(other, str):
            return self.value == other
        if isinstance(other, StringKey):
            if self._rs is None or other._rs is None:
                return self is other
            return self._rs == other._rs
        return NotImplemented

    def __lt__(self, other: object) -> bool:
        if isinstance(other, str):
            return (self.value or "") < other
        if isinstance(other, StringKey):
            return (self.value or "") < (other.value or "")
        return NotImplemented

    def __repr__(self) -> str:
        return f"{type(self).__name__}(value={self.value!r})"


class RowKey(StringKey):
    """Uniquely identifies a row in a ``DataTable``."""


class ColumnKey(StringKey):
    """Uniquely identifies a column in a ``DataTable``."""


class CellKey(NamedTuple):
    """A (row key, column key) pair identifying a cell."""

    row_key: RowKey
    column_key: ColumnKey

    def __rich_repr__(self) -> Any:
        yield "row_key", self.row_key
        yield "column_key", self.column_key


@dataclass
class Column:
    """Metadata of a column."""

    key: ColumnKey
    label: str
    width: int = 0
    content_width: int = 0
    auto_width: bool = False

    def get_render_width(self, data_table: DataTable) -> int:
        return (self.content_width if self.auto_width else self.width) + 2 * data_table.cell_padding


@dataclass
class Row:
    """Metadata of a row."""

    key: RowKey
    height: int
    label: str | None = None


def _rk(key: RowKey | str) -> str:
    return key._rs if isinstance(key, StringKey) else str(key)


def _ck(key: ColumnKey | str) -> str:
    return key._rs if isinstance(key, StringKey) else str(key)


class DataTable(DetachedNative, Widget):
    """A tabular widget with a cell / row / column cursor."""

    def __class_getitem__(cls, item: object) -> type:
        """``DataTable[CellType]`` works as in Textual (generic over the cell / data type)."""
        return cls

    _rs_kind: ClassVar[str] = "DataTable"
    _rs_framework: ClassVar[bool] = True
    can_focus = True

    class _TableMessage(Message):
        _data_table: Any = None

        @property
        def data_table(self) -> DataTable:
            return self._data_table if self._data_table is not None else self.sender  # type: ignore[return-value]

        @property
        def control(self) -> DataTable:
            return self.data_table

    class _CellMessage(_TableMessage):
        def __init__(self, data_table: Any, value: CellType, coordinate: Coordinate, cell_key: CellKey) -> None:
            super().__init__()
            self._data_table = data_table
            self.value = value
            self.coordinate = coordinate
            self.cell_key = cell_key

    class CellHighlighted(_CellMessage):
        """The cursor moved to a cell (``cursor_type == "cell"``)."""

    class CellSelected(_CellMessage):
        """A cell was selected (enter / click)."""

    class _RowMessage(_TableMessage):
        def __init__(self, data_table: Any, cursor_row: int, row_key: RowKey) -> None:
            super().__init__()
            self._data_table = data_table
            self.cursor_row = cursor_row
            self.row_key = row_key

    class RowHighlighted(_RowMessage):
        """The cursor moved to a row (``cursor_type == "row"``)."""

    class RowSelected(_RowMessage):
        """A row was selected."""

    class _ColumnMessage(_TableMessage):
        def __init__(self, data_table: Any, cursor_column: int, column_key: ColumnKey) -> None:
            super().__init__()
            self._data_table = data_table
            self.cursor_column = cursor_column
            self.column_key = column_key

    class ColumnHighlighted(_ColumnMessage):
        """The cursor moved to a column (``cursor_type == "column"``)."""

    class ColumnSelected(_ColumnMessage):
        """A column was selected."""

    class HeaderSelected(_TableMessage):
        """A column header was clicked."""

        def __init__(self, data_table: Any, column_key: ColumnKey, column_index: int, label: str) -> None:
            super().__init__()
            self._data_table = data_table
            self.column_key = column_key
            self.column_index = column_index
            self.label = label

    class RowLabelSelected(_TableMessage):
        """A row label was clicked."""

        def __init__(self, data_table: Any, row_key: RowKey, row_index: int, label: str) -> None:
            super().__init__()
            self._data_table = data_table
            self.row_key = row_key
            self.row_index = row_index
            self.label = label

    def __init__(
        self,
        *,
        show_header: bool = True,
        show_row_labels: bool = True,
        fixed_rows: int = 0,
        fixed_columns: int = 0,
        zebra_stripes: bool = False,
        header_height: int = 1,
        show_cursor: bool = True,
        cursor_foreground_priority: Literal["renderable", "css"] = "css",
        cursor_background_priority: Literal["renderable", "css"] = "renderable",
        cursor_type: CursorType = "cell",
        cell_padding: int = 1,
        name: str | None = None,
        id: str | None = None,
        classes: str | None = None,
        disabled: bool = False,
    ) -> None:
        super().__init__(name=name, id=id, classes=classes, disabled=disabled)
        self._rs_kw = {
            "show_header": show_header,
            "show_row_labels": show_row_labels,
            "fixed_rows": fixed_rows,
            "fixed_columns": fixed_columns,
            "zebra_stripes": zebra_stripes,
            "header_height": header_height,
            "show_cursor": show_cursor,
            "cursor_foreground_priority": cursor_foreground_priority,
            "cursor_background_priority": cursor_background_priority,
            "cursor_type": cursor_type,
            "cell_padding": cell_padding,
        }

    # -- options -------------------------------------------------------------------------------

    def _option(name: str) -> property:  # type: ignore[misc]
        def fget(self: DataTable) -> Any:
            return self._get(name)

        def fset(self: DataTable, value: Any) -> None:
            self._call(f"set_{name}", value)

        return property(fget, fset)

    show_header = _option("show_header")
    show_row_labels = _option("show_row_labels")
    fixed_rows = _option("fixed_rows")
    fixed_columns = _option("fixed_columns")
    zebra_stripes = _option("zebra_stripes")
    header_height = _option("header_height")
    show_cursor = _option("show_cursor")
    cursor_type = _option("cursor_type")
    cell_padding = _option("cell_padding")
    del _option

    # -- cursor ----------------------------------------------------------------------------------

    @property
    def cursor_coordinate(self) -> Coordinate:
        return Coordinate(*self._get("cursor_coordinate"))

    @cursor_coordinate.setter
    def cursor_coordinate(self, value: tuple[int, int]) -> None:
        self._call("set_cursor_coordinate", (int(value[0]), int(value[1])))

    @property
    def cursor_row(self) -> int:
        return self.cursor_coordinate.row

    @cursor_row.setter
    def cursor_row(self, row: int) -> None:
        self.move_cursor(row=row)

    @property
    def cursor_column(self) -> int:
        return self.cursor_coordinate.column

    @cursor_column.setter
    def cursor_column(self, column: int) -> None:
        self.move_cursor(column=column)

    @property
    def hover_coordinate(self) -> Coordinate:
        return Coordinate(*self._get("hover_coordinate"))

    @property
    def hover_row(self) -> int:
        return self.hover_coordinate.row

    @property
    def hover_column(self) -> int:
        return self.hover_coordinate.column

    def move_cursor(
        self,
        *,
        row: int | None = None,
        column: int | None = None,
        animate: bool = False,
        scroll: bool = True,
    ) -> None:
        """Move the cursor (posting the ``*Highlighted`` message). Needs a mounted table."""
        self._call("move_cursor", row, column)

    @property
    def cursor_row_key(self) -> RowKey:
        return self.coordinate_to_cell_key(self.cursor_coordinate).row_key

    # -- size / structure --------------------------------------------------------------------------

    @property
    def row_count(self) -> int:
        return self._get("row_count")

    @property
    def column_count(self) -> int:
        return self._get("column_count")

    @property
    def ordered_columns(self) -> list[Column]:
        return [
            Column(ColumnKey._from_rs(key), label, width, content_width, auto_width)
            for key, label, width, content_width, auto_width in self._get("columns")
        ]

    @property
    def columns(self) -> dict[ColumnKey, Column]:
        return {c.key: c for c in self.ordered_columns}

    @property
    def ordered_rows(self) -> list[Row]:
        return [
            Row(RowKey._from_rs(key), self._call("get_row_height", key))
            for key in self._get("row_keys")
        ]

    @property
    def rows(self) -> dict[RowKey, Row]:
        return {r.key: r for r in self.ordered_rows}

    def is_valid_row_index(self, row_index: int) -> bool:
        return 0 <= row_index < self.row_count

    def is_valid_column_index(self, column_index: int) -> bool:
        return 0 <= column_index < self.column_count

    def is_valid_coordinate(self, coordinate: tuple[int, int]) -> bool:
        return self.is_valid_row_index(coordinate[0]) and self.is_valid_column_index(coordinate[1])

    # -- adding / removing ---------------------------------------------------------------------------

    def add_column(
        self,
        label: Any,
        *,
        width: int | None = None,
        key: str | None = None,
        default: CellType | None = None,
    ) -> ColumnKey:
        """Add a column; ``width=None`` fits the content. Returns its key."""
        return ColumnKey._from_rs(self._call("add_column", markup_of(label), width, key, default))

    def add_columns(self, *columns: Any) -> list[ColumnKey]:
        """Add columns from labels or ``(label, key)`` tuples."""
        keys = []
        for column in columns:
            if isinstance(column, tuple):
                label, key = column
                keys.append(self.add_column(label, key=key))
            else:
                keys.append(self.add_column(column))
        return keys

    def add_row(
        self,
        *cells: CellType,
        height: int | None = 1,
        key: str | None = None,
        label: Any = None,
    ) -> RowKey:
        """Add a row; ``height=None`` fits the tallest cell. Returns its key."""
        label_markup = None if label is None else markup_of(label)
        return RowKey._from_rs(self._call("add_row", list(cells), height, key, label_markup))

    def add_rows(self, rows: Iterable[Iterable[CellType]]) -> list[RowKey]:
        return [self.add_row(*row) for row in rows]

    def remove_row(self, row_key: RowKey | str) -> None:
        self._call("remove_row", _rk(row_key))

    def remove_column(self, column_key: ColumnKey | str) -> None:
        self._call("remove_column", _ck(column_key))

    def clear(self, columns: bool = False) -> DataTable:
        """Remove all rows (and columns if ``columns``)."""
        self._call("clear", columns)
        return self

    # -- cells ------------------------------------------------------------------------------------------

    def update_cell(
        self,
        row_key: RowKey | str,
        column_key: ColumnKey | str,
        value: CellType,
        *,
        update_width: bool = False,
    ) -> None:
        self._call("update_cell", _rk(row_key), _ck(column_key), value, update_width)

    def update_cell_at(self, coordinate: tuple[int, int], value: CellType, *, update_width: bool = False) -> None:
        self._call("update_cell_at", (coordinate[0], coordinate[1]), value, update_width)

    def get_cell(self, row_key: RowKey | str, column_key: ColumnKey | str) -> CellType:
        return self._call("get_cell", _rk(row_key), _ck(column_key))

    def get_cell_at(self, coordinate: tuple[int, int]) -> CellType:
        return self._call("get_cell_at", (coordinate[0], coordinate[1]))

    def get_cell_coordinate(self, row_key: RowKey | str, column_key: ColumnKey | str) -> Coordinate:
        return Coordinate(*self._call("get_cell_coordinate", _rk(row_key), _ck(column_key)))

    def get_row(self, row_key: RowKey | str) -> list[CellType]:
        return self._call("get_row", _rk(row_key))

    def get_row_at(self, row_index: int) -> list[CellType]:
        if not self.is_valid_row_index(row_index):
            raise RowDoesNotExist(f"Row index {row_index!r} is not valid.")
        return self._call("get_row_at", row_index)

    def get_row_index(self, row_key: RowKey | str) -> int:
        return self._call("get_row_index", _rk(row_key))

    def get_column(self, column_key: ColumnKey | str) -> Iterable[CellType]:
        return iter(self._call("get_column", _ck(column_key)))

    def get_column_at(self, column_index: int) -> Iterable[CellType]:
        if not self.is_valid_column_index(column_index):
            raise ColumnDoesNotExist(f"Column index {column_index!r} is not valid.")
        return iter(self._call("get_column_at", column_index))

    def get_column_index(self, column_key: ColumnKey | str) -> int:
        return self._call("get_column_index", _ck(column_key))

    def get_row_height(self, row_key: RowKey | str) -> int:
        return self._call("get_row_height", _rk(row_key))

    def coordinate_to_cell_key(self, coordinate: tuple[int, int]) -> CellKey:
        row, column = self._call("coordinate_to_cell_key", (coordinate[0], coordinate[1]))
        return CellKey(RowKey._from_rs(row), ColumnKey._from_rs(column))

    # -- sorting ----------------------------------------------------------------------------------------

    def sort(
        self,
        *columns: ColumnKey | str,
        key: Callable[[Any], Any] | None = None,
        reverse: bool = False,
    ) -> DataTable:
        """Sort rows by the values in ``columns`` and / or a ``key`` function."""
        self._call("sort", [_ck(c) for c in columns], key, reverse)
        return self


def _cell_key(keys: tuple[str, str]) -> CellKey:
    return CellKey(RowKey._from_rs(keys[0]), ColumnKey._from_rs(keys[1]))


def _register() -> None:
    reg = _native.register_class
    for name in ("CellHighlighted", "CellSelected"):
        cls = getattr(DataTable, name)
        reg(
            f"DataTable.{name}",
            lambda cls=cls, **kw: cls(None, kw["value"], Coordinate(*kw["coordinate"]), _cell_key(kw["cell_key"])),
        )
    for name in ("RowHighlighted", "RowSelected"):
        cls = getattr(DataTable, name)
        reg(f"DataTable.{name}", lambda cls=cls, **kw: cls(None, kw["cursor_row"], RowKey._from_rs(kw["row_key"])))
    for name in ("ColumnHighlighted", "ColumnSelected"):
        cls = getattr(DataTable, name)
        reg(
            f"DataTable.{name}",
            lambda cls=cls, **kw: cls(None, kw["cursor_column"], ColumnKey._from_rs(kw["column_key"])),
        )
    reg(
        "DataTable.HeaderSelected",
        lambda **kw: DataTable.HeaderSelected(None, ColumnKey._from_rs(kw["column_key"]), kw["column_index"], kw["label"]),
    )
    reg(
        "DataTable.RowLabelSelected",
        lambda **kw: DataTable.RowLabelSelected(None, RowKey._from_rs(kw["row_key"]), kw["row_index"], kw["label"]),
    )
    for exc in (CellDoesNotExist, RowDoesNotExist, ColumnDoesNotExist, DuplicateKey):
        reg(f"DataTable.{exc.__name__}", lambda exc=exc, message="": exc(message))


_register()
_NATIVE_CLASSES["DataTable"] = DataTable

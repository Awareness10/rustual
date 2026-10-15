"""Coordinates of cells in a grid (Textual's ``textual.coordinate``)."""

from __future__ import annotations

from typing import NamedTuple


class Coordinate(NamedTuple):
    """A (row, column) position in a grid such as a ``DataTable``."""

    row: int
    column: int

    def left(self) -> Coordinate:
        return Coordinate(self.row, self.column - 1)

    def right(self) -> Coordinate:
        return Coordinate(self.row, self.column + 1)

    def up(self) -> Coordinate:
        return Coordinate(self.row - 1, self.column)

    def down(self) -> Coordinate:
        return Coordinate(self.row + 1, self.column)

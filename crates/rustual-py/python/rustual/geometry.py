"""Geometry primitives (Textual's ``textual.geometry``)."""

from __future__ import annotations

from typing import NamedTuple


class Offset(NamedTuple):
    x: int = 0
    y: int = 0

    def __add__(self, other: object) -> Offset:  # type: ignore[override]
        if isinstance(other, tuple):
            return Offset(self.x + other[0], self.y + other[1])
        return NotImplemented

    def __sub__(self, other: object) -> Offset:
        if isinstance(other, tuple):
            return Offset(self.x - other[0], self.y - other[1])
        return NotImplemented

    @property
    def is_origin(self) -> bool:
        return self.x == 0 and self.y == 0


class Size(NamedTuple):
    width: int = 0
    height: int = 0

    @property
    def area(self) -> int:
        return self.width * self.height

    @property
    def region(self) -> Region:
        return Region(0, 0, self.width, self.height)

    def __bool__(self) -> bool:
        return self.width * self.height != 0

    def contains(self, x: int, y: int) -> bool:
        return 0 <= x < self.width and 0 <= y < self.height


class Region(NamedTuple):
    x: int = 0
    y: int = 0
    width: int = 0
    height: int = 0

    @property
    def offset(self) -> Offset:
        return Offset(self.x, self.y)

    @property
    def size(self) -> Size:
        return Size(self.width, self.height)

    @property
    def right(self) -> int:
        return self.x + self.width

    @property
    def bottom(self) -> int:
        return self.y + self.height

    def contains(self, x: int, y: int) -> bool:
        return self.x <= x < self.right and self.y <= y < self.bottom

    def __bool__(self) -> bool:
        return self.width * self.height > 0


class Spacing(NamedTuple):
    top: int = 0
    right: int = 0
    bottom: int = 0
    left: int = 0

    @property
    def width(self) -> int:
        return self.left + self.right

    @property
    def height(self) -> int:
        return self.top + self.bottom

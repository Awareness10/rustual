"""Colors (a subset of Textual's ``textual.color``): parsing, blending and gradients."""

from __future__ import annotations

import re
from typing import NamedTuple, Sequence

_HEX = re.compile(r"#([0-9a-fA-F]{3,8})$")
_RGB = re.compile(r"rgba?\(\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)\s*(?:,\s*([\d.]+)\s*)?\)$")


class ColorParseError(Exception):
    pass


class Color(NamedTuple):
    """An RGB color with alpha (0-1)."""

    r: int
    g: int
    b: int
    a: float = 1.0

    @classmethod
    def parse(cls, color: str | Color) -> Color:
        if isinstance(color, Color):
            return color
        text = color.strip()
        m = _HEX.match(text)
        if m:
            h = m.group(1)
            if len(h) in (3, 4):
                h = "".join(c * 2 for c in h)
            if len(h) not in (6, 8):
                raise ColorParseError(f"invalid color {color!r}")
            a = int(h[6:8], 16) / 255 if len(h) == 8 else 1.0
            return cls(int(h[0:2], 16), int(h[2:4], 16), int(h[4:6], 16), a)
        m = _RGB.match(text)
        if m:
            r, g, b, a = m.groups()
            return cls(int(r), int(g), int(b), float(a) if a else 1.0)
        try:
            from rich.color import Color as RichColor

            t = RichColor.parse(text).get_truecolor()
            return cls(t.red, t.green, t.blue)
        except Exception as error:  # noqa: BLE001
            raise ColorParseError(f"invalid color {color!r}") from error

    @classmethod
    def from_rgb(cls, r: float, g: float, b: float, a: float = 1.0) -> Color:
        return cls(round(r), round(g), round(b), a)

    @property
    def hex(self) -> str:
        return f"#{self.r:02X}{self.g:02X}{self.b:02X}"

    @property
    def rgb(self) -> tuple[int, int, int]:
        return (self.r, self.g, self.b)

    def blend(self, destination: Color, factor: float, alpha: float | None = None) -> Color:
        if factor <= 0:
            return self
        if factor >= 1:
            return destination
        r = self.r + (destination.r - self.r) * factor
        g = self.g + (destination.g - self.g) * factor
        b = self.b + (destination.b - self.b) * factor
        a = self.a + (destination.a - self.a) * factor if alpha is None else alpha
        return Color(int(r), int(g), int(b), a)

    @property
    def rich_color(self):  # noqa: ANN201 - rich is optional
        from rich.color import Color as RichColor

        return RichColor.from_rgb(self.r, self.g, self.b)

    def __str__(self) -> str:
        return self.hex


class Gradient:
    """A color gradient defined by ``(offset, color)`` stops with offsets in 0..1."""

    def __init__(self, *stops: tuple[float, Color | str], quality: int = 50) -> None:
        parsed = sorted(((float(o), Color.parse(c)) for o, c in stops), key=lambda s: s[0])
        if len(parsed) < 2:
            raise ValueError("a gradient needs at least two stops")
        self._stops = parsed
        self._quality = quality
        self._cache: dict[int, Color] = {}

    @classmethod
    def from_colors(cls, *colors: Color | str, quality: int = 50) -> Gradient:
        n = len(colors) - 1
        return cls(*((i / n, c) for i, c in enumerate(colors)), quality=quality)

    @property
    def colors(self) -> list[Color]:
        return [self.get_color(i / (self._quality - 1)) for i in range(self._quality)]

    def get_color(self, position: float) -> Color:
        position = min(1.0, max(0.0, position))
        key = int(position * 1000)
        cached = self._cache.get(key)
        if cached is not None:
            return cached
        stops: Sequence[tuple[float, Color]] = self._stops
        color = stops[-1][1]
        for (o1, c1), (o2, c2) in zip(stops, stops[1:]):
            if o1 <= position <= o2:
                color = c1.blend(c2, (position - o1) / (o2 - o1) if o2 > o1 else 0.0)
                break
        self._cache[key] = color
        return color

    def get_rich_color(self, position: float):  # noqa: ANN201 - rich is optional
        return self.get_color(position).rich_color

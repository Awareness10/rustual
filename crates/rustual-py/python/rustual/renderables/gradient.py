"""Gradient renderables (port of Textual's ``textual.renderables.gradient``, MIT)."""

from __future__ import annotations

from collections.abc import Sequence
from math import cos, pi, sin
from typing import Any

from ..color import Color, Gradient


class VerticalGradient:
    """A vertical gradient between two colors."""

    def __init__(self, color1: str, color2: str) -> None:
        self._color1 = Color.parse(color1)
        self._color2 = Color.parse(color2)

    def __rich_console__(self, console: Any, options: Any) -> Any:
        from rich.segment import Segment
        from rich.style import Style

        width = options.max_width
        height = options.height or options.max_height
        for y in range(height):
            top = self._color1.blend(self._color2, (y * 2) / max(1, height * 2 - 1)).rich_color
            bottom = self._color1.blend(self._color2, (y * 2 + 1) / max(1, height * 2 - 1)).rich_color
            yield Segment("▀" * width, Style.from_color(top, bottom))
            yield Segment.line()


class LinearGradient:
    """A linear gradient with a rotation.

    Args:
        angle: Angle of rotation in degrees.
        stops: ``(offset, color)`` pairs, offsets between 0 and 1.
    """

    def __init__(self, angle: float, stops: Sequence[tuple[float, Color | str]]) -> None:
        self.angle = angle
        self._stops = [(stop, Color.parse(color) if isinstance(color, str) else color) for stop, color in stops]
        self._color_gradient = Gradient(*self._stops)

    def __rich_console__(self, console: Any, options: Any) -> Any:
        from rich.segment import Segment
        from rich.style import Style

        width = options.max_width
        height = options.height or options.max_height

        angle_radians = -self.angle * pi / 180.0
        sin_angle = sin(angle_radians)
        cos_angle = cos(angle_radians)
        center_x = width / 2
        center_y = height
        new_line = Segment.line()
        get_color = self._color_gradient.get_rich_color
        from_color = Style.from_color

        for line_y in range(height):
            point_y = float(line_y) * 2 - center_y
            point_x = 0 - center_x
            x1 = (center_x + (point_x * cos_angle - point_y * sin_angle)) / width
            x2 = (center_x + (point_x * cos_angle - (point_y + 1.0) * sin_angle)) / width
            point_x = width - center_x
            end_x1 = (center_x + (point_x * cos_angle - point_y * sin_angle)) / width
            delta_x = (end_x1 - x1) / width
            if abs(delta_x) < 0.0001:
                yield Segment("▀" * width, from_color(get_color(x1), get_color(x2)))
            else:
                yield from [
                    Segment("▀", from_color(get_color(x1 + x * delta_x), get_color(x2 + x * delta_x)))
                    for x in range(width)
                ]
            yield new_line

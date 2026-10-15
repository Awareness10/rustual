"""Attribute animation (Textual's ``widget.animate`` / ``styles.animate``), driven by a native timer."""

from __future__ import annotations

import math
import time
from dataclasses import dataclass
from typing import Any, Callable

from ._bridge import native


def _out_bounce(x: float) -> float:
    n1, d1 = 7.5625, 2.75
    if x < 1 / d1:
        return n1 * x * x
    if x < 2 / d1:
        x -= 1.5 / d1
        return n1 * x * x + 0.75
    if x < 2.5 / d1:
        x -= 2.25 / d1
        return n1 * x * x + 0.9375
    x -= 2.625 / d1
    return n1 * x * x + 0.984375


EASING: dict[str, Callable[[float], float]] = {
    "none": lambda x: 1.0,
    "round": lambda x: 0.0 if x < 0.5 else 1.0,
    "linear": lambda x: x,
    "in_sine": lambda x: 1 - math.cos((x * math.pi) / 2),
    "in_out_sine": lambda x: -(math.cos(math.pi * x) - 1) / 2,
    "out_sine": lambda x: math.sin((x * math.pi) / 2),
    "in_quad": lambda x: x * x,
    "in_out_quad": lambda x: 2 * x * x if x < 0.5 else 1 - pow(-2 * x + 2, 2) / 2,
    "out_quad": lambda x: 1 - (1 - x) * (1 - x),
    "in_cubic": lambda x: x * x * x,
    "in_out_cubic": lambda x: 4 * x * x * x if x < 0.5 else 1 - pow(-2 * x + 2, 3) / 2,
    "out_cubic": lambda x: 1 - pow(1 - x, 3),
    "in_quart": lambda x: pow(x, 4),
    "in_out_quart": lambda x: 8 * pow(x, 4) if x < 0.5 else 1 - pow(-2 * x + 2, 4) / 2,
    "out_quart": lambda x: 1 - pow(1 - x, 4),
    "in_quint": lambda x: pow(x, 5),
    "in_out_quint": lambda x: 16 * pow(x, 5) if x < 0.5 else 1 - pow(-2 * x + 2, 5) / 2,
    "out_quint": lambda x: 1 - pow(1 - x, 5),
    "in_expo": lambda x: 0 if x == 0 else pow(2, 10 * x - 10),
    "in_out_expo": lambda x: 0 if x == 0 else 1 if x == 1 else (pow(2, 20 * x - 10) / 2 if x < 0.5 else (2 - pow(2, -20 * x + 10)) / 2),
    "out_expo": lambda x: 1 if x == 1 else 1 - pow(2, -10 * x),
    "in_circ": lambda x: 1 - math.sqrt(1 - pow(x, 2)),
    "in_out_circ": lambda x: (1 - math.sqrt(1 - pow(2 * x, 2))) / 2 if x < 0.5 else (math.sqrt(1 - pow(-2 * x + 2, 2)) + 1) / 2,
    "out_circ": lambda x: math.sqrt(1 - pow(x - 1, 2)),
    "in_back": lambda x: 2.70158 * x * x * x - 1.70158 * x * x,
    "in_out_back": lambda x: (pow(2 * x, 2) * ((2.5949095 + 1) * 2 * x - 2.5949095)) / 2 if x < 0.5 else (pow(2 * x - 2, 2) * ((2.5949095 + 1) * (x * 2 - 2) + 2.5949095) + 2) / 2,
    "out_back": lambda x: 1 + 2.70158 * pow(x - 1, 3) + 1.70158 * pow(x - 1, 2),
    "in_elastic": lambda x: 0 if x == 0 else 1 if x == 1 else -pow(2, 10 * x - 10) * math.sin((x * 10 - 10.75) * ((2 * math.pi) / 3)),
    "out_elastic": lambda x: 0 if x == 0 else 1 if x == 1 else pow(2, -10 * x) * math.sin((x * 10 - 0.75) * ((2 * math.pi) / 3)) + 1,
    "in_bounce": lambda x: 1 - _out_bounce(1 - x),
    "out_bounce": _out_bounce,
    "in_out_bounce": lambda x: (1 - _out_bounce(1 - 2 * x)) / 2 if x < 0.5 else (1 + _out_bounce(2 * x - 1)) / 2,
}
DEFAULT_EASING = "in_out_cubic"


@dataclass
class _Animation:
    target: Any
    attribute: str
    start: float
    end: Any
    start_time: float
    duration: float
    easing: Callable[[float], float]
    on_complete: Callable[[], Any] | None
    setter: Callable[[Any, str, Any], None]

    def step(self, now: float) -> bool:
        progress = 1.0 if self.duration <= 0 else min(1.0, (now - self.start_time) / self.duration)
        if progress >= 1.0:
            self.setter(self.target, self.attribute, self.end)
            return True
        eased = self.easing(progress)
        value = self.start + (self.end - self.start) * eased
        if isinstance(self.end, int) and isinstance(self.start, int):
            value = round(value)
        self.setter(self.target, self.attribute, value)
        return False


class Animator:
    FPS = 60

    def __init__(self) -> None:
        self._animations: dict[tuple[int, str], _Animation] = {}
        self._timer: int | None = None

    def animate(
        self,
        target: Any,
        attribute: str,
        value: Any,
        *,
        final_value: Any = ...,
        duration: float | None = None,
        speed: float | None = None,
        delay: float = 0.0,
        easing: str | Callable[[float], float] = DEFAULT_EASING,
        on_complete: Callable[[], Any] | None = None,
        level: str = "full",
        setter: Callable[[Any, str, Any], None] = setattr,
        getter: Callable[[Any, str], Any] = getattr,
    ) -> None:
        start = getter(target, attribute)
        end = value if final_value is ... else final_value
        if not isinstance(start, (int, float)) or not isinstance(value, (int, float)):
            setter(target, attribute, end)
            if on_complete is not None:
                on_complete()
            return
        if duration is None:
            duration = abs(value - start) / speed if speed else 1.0
        ease = EASING.get(easing, EASING["linear"]) if isinstance(easing, str) else easing
        self._animations[(id(target), attribute)] = _Animation(
            target, attribute, float(start), end, time.monotonic() + delay, duration, ease, on_complete, setter
        )
        self._ensure_timer()

    def is_being_animated(self, target: Any, attribute: str) -> bool:
        return (id(target), attribute) in self._animations

    def stop(self, target: Any, attribute: str, complete: bool = True) -> None:
        anim = self._animations.pop((id(target), attribute), None)
        if anim is not None and complete:
            anim.setter(anim.target, anim.attribute, anim.end)

    def _ensure_timer(self) -> None:
        if self._timer is None:
            self._timer = native.set_timer(native.root(), 1 / self.FPS, self._tick, True)
        self._tick()

    def _tick(self) -> None:
        now = time.monotonic()
        finished = []
        for key, anim in list(self._animations.items()):
            if now < anim.start_time:
                continue
            if anim.step(now):
                finished.append(key)
        for key in finished:
            anim = self._animations.pop(key, None)
            if anim is not None and anim.on_complete is not None:
                from .widget import _invoke

                _invoke(anim.on_complete)
        if not self._animations and self._timer is not None:
            native.cancel_timer(self._timer)
            self._timer = None

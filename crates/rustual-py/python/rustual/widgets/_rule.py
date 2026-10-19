from __future__ import annotations

from typing import ClassVar, Literal

from ..widget import _NATIVE_CLASSES, Widget

RuleOrientation = Literal["horizontal", "vertical"]
"""The valid orientations of the rule widget."""

LineStyle = Literal["ascii", "blank", "dashed", "double", "heavy", "hidden", "none", "solid", "thick"]
"""The valid line styles of the rule widget."""

_VALID_RULE_ORIENTATIONS = {"horizontal", "vertical"}
_VALID_LINE_STYLES = {"ascii", "blank", "dashed", "double", "heavy", "hidden", "none", "solid", "thick"}


class InvalidRuleOrientation(Exception):
    """Exception raised for an invalid rule orientation."""


class InvalidLineStyle(Exception):
    """Exception raised for an invalid rule line style."""


class Rule(Widget):
    """A rule widget to separate content, similar to a ``<hr>`` HTML tag."""

    _rs_kind: ClassVar[str] = "Rule"
    _rs_framework: ClassVar[bool] = True

    def __init__(
        self,
        orientation: RuleOrientation = "horizontal",
        line_style: LineStyle = "solid",
        *,
        name: str | None = None,
        id: str | None = None,
        classes: str | None = None,
        disabled: bool = False,
    ) -> None:
        super().__init__(name=name, id=id, classes=classes, disabled=disabled)
        self._rs_kw = {
            "orientation": self._check_orientation(orientation),
            "line_style": self._check_line_style(line_style),
        }

    @staticmethod
    def _check_orientation(orientation: str) -> str:
        if orientation not in _VALID_RULE_ORIENTATIONS:
            raise InvalidRuleOrientation(f"Valid rule orientations are {sorted(_VALID_RULE_ORIENTATIONS)!r}")
        return orientation

    @staticmethod
    def _check_line_style(style: str) -> str:
        if style not in _VALID_LINE_STYLES:
            raise InvalidLineStyle(f"Valid rule line styles are {sorted(_VALID_LINE_STYLES)!r}")
        return style

    @property
    def orientation(self) -> RuleOrientation:
        return self._native_get("orientation", "horizontal")

    @orientation.setter
    def orientation(self, value: RuleOrientation) -> None:
        self._native_set("orientation", self._check_orientation(value))

    @property
    def line_style(self) -> LineStyle:
        return self._native_get("line_style", "solid")

    @line_style.setter
    def line_style(self, value: LineStyle) -> None:
        self._native_set("line_style", self._check_line_style(value))

    @classmethod
    def horizontal(
        cls,
        line_style: LineStyle = "solid",
        *,
        name: str | None = None,
        id: str | None = None,
        classes: str | None = None,
        disabled: bool = False,
    ) -> Rule:
        return cls("horizontal", line_style, name=name, id=id, classes=classes, disabled=disabled)

    @classmethod
    def vertical(
        cls,
        line_style: LineStyle = "solid",
        *,
        name: str | None = None,
        id: str | None = None,
        classes: str | None = None,
        disabled: bool = False,
    ) -> Rule:
        return cls("vertical", line_style, name=name, id=id, classes=classes, disabled=disabled)


_NATIVE_CLASSES["Rule"] = Rule

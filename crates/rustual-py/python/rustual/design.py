"""The design system (Textual's ``textual.design``): CSS variables from a few base colors."""

from __future__ import annotations

from typing import Any


class ColorSystem:
    """Generates the CSS variables (``$primary``, ``$surface``, ``$text-muted``, ...) of a theme."""

    def __init__(
        self,
        primary: str,
        secondary: str | None = None,
        warning: str | None = None,
        error: str | None = None,
        success: str | None = None,
        accent: str | None = None,
        foreground: str | None = None,
        background: str | None = None,
        surface: str | None = None,
        panel: str | None = None,
        boost: str | None = None,
        dark: bool = False,
        luminosity_spread: float = 0.15,
        text_alpha: float = 0.95,
        variables: dict[str, str] | None = None,
        ansi: bool = False,
    ) -> None:
        self._spec: dict[str, Any] = {
            "name": "color-system",
            "primary": str(primary),
            "secondary": _opt(secondary),
            "warning": _opt(warning),
            "error": _opt(error),
            "success": _opt(success),
            "accent": _opt(accent),
            "foreground": _opt(foreground),
            "background": _opt(background),
            "surface": _opt(surface),
            "panel": _opt(panel),
            "boost": _opt(boost),
            "dark": dark,
            "luminosity_spread": luminosity_spread,
            "text_alpha": text_alpha,
            "variables": dict(variables or {}),
            "ansi": ansi,
        }
        self.dark = dark
        self.variables = self._spec["variables"]

    def generate(self) -> dict[str, str]:
        """All CSS variables (names without ``$``) mapped to CSS values."""
        from ._bridge import _native

        return dict(_native.theme_variables(self._spec))


def _opt(color: Any) -> str | None:
    if color is None:
        return None
    hex_ = getattr(color, "hex", None)  # a Color object
    return hex_ if isinstance(hex_, str) else str(color)


__all__ = ["ColorSystem"]

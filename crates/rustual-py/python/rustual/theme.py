"""Themes (Textual's ``textual.theme``)."""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any


@dataclass
class Theme:
    name: str
    primary: str
    secondary: str | None = None
    warning: str | None = None
    error: str | None = None
    success: str | None = None
    accent: str | None = None
    foreground: str | None = None
    background: str | None = None
    surface: str | None = None
    panel: str | None = None
    boost: str | None = None
    dark: bool = True
    luminosity_spread: float = 0.15
    text_alpha: float = 0.95
    variables: dict[str, str] = field(default_factory=dict)
    ansi: bool = False

    def to_color_system(self) -> Any:
        """The :class:`~rustual.design.ColorSystem` for this theme."""
        from .design import ColorSystem

        return ColorSystem(
            primary=self.primary,
            secondary=self.secondary,
            warning=self.warning,
            error=self.error,
            success=self.success,
            accent=self.accent,
            foreground=self.foreground,
            background=self.background,
            surface=self.surface,
            panel=self.panel,
            boost=self.boost,
            dark=self.dark,
            luminosity_spread=self.luminosity_spread,
            text_alpha=self.text_alpha,
            variables=self.variables,
            ansi=self.ansi,
        )

    def _native(self) -> dict[str, Any]:
        return {
            "name": self.name,
            "primary": self.primary,
            "secondary": self.secondary,
            "warning": self.warning,
            "error": self.error,
            "success": self.success,
            "accent": self.accent,
            "foreground": self.foreground,
            "background": self.background,
            "surface": self.surface,
            "panel": self.panel,
            "boost": self.boost,
            "dark": self.dark,
            "luminosity_spread": self.luminosity_spread,
            "text_alpha": self.text_alpha,
            "variables": dict(self.variables),
            "ansi": bool(self.ansi),
        }


def _builtin_themes() -> dict[str, Theme]:
    from ._bridge import _native

    themes: dict[str, Theme] = {}
    for name in _native.builtin_theme_names():
        fields = _native.builtin_theme(name)
        if fields is not None:
            themes[name] = Theme(**fields)
    return themes


BUILTIN_THEMES: dict[str, Theme] = _builtin_themes()
"""The built-in themes by name (Textual's ``BUILTIN_THEMES``)."""

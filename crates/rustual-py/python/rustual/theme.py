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
        }

from __future__ import annotations

from typing import ClassVar, Literal

from ..widget import _NATIVE_CLASSES, Widget

PlaceholderVariant = Literal["default", "size", "text"]
_VALID_PLACEHOLDER_VARIANTS_ORDERED: list[str] = ["default", "size", "text"]


class InvalidPlaceholderVariant(Exception):
    """Raised when an invalid Placeholder variant is set."""


class Placeholder(Widget):
    """A simple placeholder widget; click it to cycle through its variants."""

    _rs_kind: ClassVar[str] = "Placeholder"
    _rs_framework: ClassVar[bool] = True

    def __init__(
        self,
        label: str | None = None,
        variant: PlaceholderVariant = "default",
        *,
        name: str | None = None,
        id: str | None = None,
        classes: str | None = None,
        disabled: bool = False,
    ) -> None:
        super().__init__(name=name, id=id, classes=classes, disabled=disabled)
        self._rs_kw = {"label": label, "variant": self._check(variant)}

    @staticmethod
    def _check(variant: str) -> str:
        if variant not in _VALID_PLACEHOLDER_VARIANTS_ORDERED:
            raise InvalidPlaceholderVariant(f"Valid placeholder variants are {_VALID_PLACEHOLDER_VARIANTS_ORDERED!r}")
        return variant

    @property
    def variant(self) -> PlaceholderVariant:
        return self._native_get("variant", "default")

    @variant.setter
    def variant(self, value: PlaceholderVariant) -> None:
        self._native_set("variant", self._check(value))

    def cycle_variant(self) -> Placeholder:
        """Switch to the next variant (default → size → text → default)."""
        if self.is_mounted:
            self._native_call("cycle_variant")
        else:
            order = _VALID_PLACEHOLDER_VARIANTS_ORDERED
            self.variant = order[(order.index(self.variant) + 1) % len(order)]  # type: ignore[assignment]
        return self


_NATIVE_CLASSES["Placeholder"] = Placeholder

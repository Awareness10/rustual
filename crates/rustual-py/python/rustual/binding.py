"""Key bindings (Textual's ``textual.binding``)."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Iterable, Union


class BindingError(Exception):
    pass


@dataclass(frozen=True)
class Binding:
    key: str
    action: str
    description: str = ""
    show: bool = True
    key_display: str | None = None
    priority: bool = False
    tooltip: str = ""
    id: str | None = None
    system: bool = False

    def _native(self) -> tuple:
        return (
            self.key,
            self.action,
            self.description,
            self.show,
            self.key_display,
            self.priority,
            self.id,
        )


BindingType = Union[Binding, tuple[str, str], tuple[str, str, str]]


def make_binding(b: BindingType) -> Binding:
    if isinstance(b, Binding):
        return b
    if isinstance(b, tuple) and len(b) in (2, 3):
        return Binding(*b)
    raise BindingError(f"invalid binding {b!r}; expected Binding or (key, action[, description])")


def collect_bindings(cls: type) -> list[Binding]:
    """Bindings of a class, merged along the MRO (base first) unless ``inherit_bindings=False``."""
    classes: list[type] = []
    for klass in cls.__mro__:
        if "BINDINGS" in klass.__dict__:
            classes.append(klass)
        if not klass.__dict__.get("_inherit_bindings", True):
            break
    result: dict[str, Binding] = {}
    for klass in reversed(classes):
        for b in klass.__dict__["BINDINGS"]:
            binding = make_binding(b)
            # Keyed by key, as Textual's `_merge_bindings`: a subclass binding replaces the
            # base class's binding for the same key, keeping its position.
            result[binding.key] = binding
    return list(result.values())


def _iter(bindings: Iterable[BindingType]) -> list[Binding]:
    return [make_binding(b) for b in bindings]

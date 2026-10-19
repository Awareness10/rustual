from __future__ import annotations

import pprint
from typing import Any, ClassVar

from ..widget import _NATIVE_CLASSES, Widget
from ._detached import DetachedNative


def _reprs(obj: Any) -> tuple[str, str]:
    compact = repr(obj)
    try:
        expanded = pprint.pformat(obj, indent=1, width=40, sort_dicts=False)
    except Exception:
        expanded = compact
    return compact, expanded


class Pretty(DetachedNative, Widget):
    """Displays a pretty-printed, highlighted representation of a Python object."""

    _rs_kind: ClassVar[str] = "Pretty"
    _rs_framework: ClassVar[bool] = True

    def __init__(
        self,
        object: Any,
        *,
        name: str | None = None,
        id: str | None = None,
        classes: str | None = None,
    ) -> None:
        super().__init__(name=name, id=id, classes=classes)
        compact, expanded = _reprs(object)
        self._rs_kw = {"compact": compact, "expanded": expanded}

    def update(self, object: Any) -> None:
        """Show a different object."""
        self._call("update", *_reprs(object))


_NATIVE_CLASSES["Pretty"] = Pretty

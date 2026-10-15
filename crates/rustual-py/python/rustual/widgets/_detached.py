"""Native widgets that can be filled before they are mounted.

A ``DataTable``, ``Tree``, ``Log``... is often populated in ``compose()`` before it is mounted.
Until then the Python object owns a *detached* Rust widget (``_native.Detached``), created
lazily from the constructor kwargs; when the widget mounts, the engine takes it over. Methods
call :meth:`_call` / :meth:`_get`, which go to the mounted widget or to the detached one.
"""

from __future__ import annotations

from typing import Any

from .._bridge import _native, native
from ..content import escape, to_markup


class DetachedNative:
    """Mixin for :class:`~rustual.widget.Widget` subclasses backed by a data widget."""

    def _rs_kwargs(self) -> dict[str, Any]:
        kwargs = super()._rs_kwargs()  # type: ignore[misc]
        detached = self.__dict__.pop("_rs_detached", None)
        if detached is not None:
            kwargs["_detached"] = detached
        return kwargs

    def _detached(self) -> Any:
        detached = self.__dict__.get("_rs_detached")
        if detached is None:
            kwargs = dict(self.__dict__.get("_rs_kw", {}))
            detached = _native.detached_new(self._rs_kind, kwargs)  # type: ignore[attr-defined]
            self.__dict__["_rs_detached"] = detached
        return detached

    def _call(self, method: str, *args: Any) -> Any:
        if self.is_mounted:  # type: ignore[attr-defined]
            return native.widget_call(self._node, method, args)  # type: ignore[attr-defined]
        return self._detached().call(method, args)

    def _get(self, attr: str) -> Any:
        if self.is_mounted:  # type: ignore[attr-defined]
            return native.widget_get(self._node, attr)  # type: ignore[attr-defined]
        return self._detached().get(attr)


def markup_of(content: Any) -> str:
    """Markup for a label: a ``str`` is markup, ``Content`` / Rich ``Text`` keep their
    styles, anything else is shown with ``str()``."""
    text, is_markup = to_markup(content)
    return text if is_markup else escape(text)

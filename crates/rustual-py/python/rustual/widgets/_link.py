from __future__ import annotations

import webbrowser
from typing import Any, ClassVar

from .. import _context
from .._bridge import _native
from ..widget import Widget, _NATIVE_CLASSES


class Link(Widget):
    """A clickable link that opens a URL."""

    _rs_kind: ClassVar[str] = "Link"
    _rs_framework: ClassVar[bool] = True

    def __init__(
        self,
        text: str,
        *,
        url: str | None = None,
        tooltip: Any = None,
        name: str | None = None,
        id: str | None = None,
        classes: str | None = None,
        disabled: bool = False,
    ) -> None:
        super().__init__(name=name, id=id, classes=classes, disabled=disabled)
        self._rs_kw = {"text": text, "url": text if url is None else url}
        if tooltip is not None:
            self.tooltip = tooltip

    @property
    def text(self) -> str:
        return self._native_get("text", "")

    @text.setter
    def text(self, value: str) -> None:
        self._native_set("text", str(value))

    @property
    def url(self) -> str:
        return self._native_get("url", "")

    @url.setter
    def url(self, value: str) -> None:
        self._native_set("url", str(value))

    def action_open_link(self) -> None:
        """Open the link's URL."""
        self._native_call("action_open_link")


def _open_url(url: str) -> None:
    """Open URLs for `Link` / `Markdown`: through ``App.open_url`` when the app defines one
    (Textual's hook), else with the ``webbrowser`` module (as Textual does)."""
    try:
        app = _context.active_app()
    except Exception:
        app = None
    opener = getattr(app, "open_url", None)
    if opener is not None:
        opener(url)
    else:
        webbrowser.open(url)


_native.set_url_opener(_open_url)
_NATIVE_CLASSES["Link"] = Link

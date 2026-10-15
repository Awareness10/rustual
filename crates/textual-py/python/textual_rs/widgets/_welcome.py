from __future__ import annotations

from typing import ClassVar

from ..widget import Widget, _NATIVE_CLASSES


class Welcome(Widget):
    """A welcome widget: some text and an "OK" button (``#close``).

    As in Textual, pressing "OK" only posts ``Button.Pressed``; handle it to exit.
    """

    _rs_kind: ClassVar[str] = "Welcome"
    _rs_framework: ClassVar[bool] = True

    def __init__(
        self,
        *,
        name: str | None = None,
        id: str | None = None,
        classes: str | None = None,
        disabled: bool = False,
    ) -> None:
        super().__init__(name=name, id=id, classes=classes, disabled=disabled)
        self._rs_kw = {"exit_on_close": False}


_NATIVE_CLASSES["Welcome"] = Welcome

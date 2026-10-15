from __future__ import annotations

from typing import Any, ClassVar

from .._rich import is_rich_renderable
from ..content import to_markup
from ..widget import Widget, _NATIVE_CLASSES


class Static(Widget):
    """Displays content (markup by default); change it with :meth:`update`."""

    _rs_kind: ClassVar[str] = "Static"
    _rs_framework: ClassVar[bool] = True

    def __init__(
        self,
        content: Any = "",
        *,
        expand: bool = False,
        shrink: bool = False,
        markup: bool = True,
        name: str | None = None,
        id: str | None = None,
        classes: str | None = None,
        disabled: bool = False,
    ) -> None:
        super().__init__(name=name, id=id, classes=classes, disabled=disabled, markup=markup)
        self._content = content
        if is_rich_renderable(content):
            self._rs_kw = {"content": "", "markup": False, "rich": content}
        else:
            text, is_markup = to_markup(content, markup)
            self._rs_kw = {"content": text, "markup": is_markup}

    def update(self, content: Any = "", *, layout: bool = True) -> None:
        """Replace the content (markup string, ``Content`` or Rich ``Text``)."""
        self._content = content
        if is_rich_renderable(content):
            if self.is_mounted:
                self._native_call("update_rich", content)
            else:
                self._rs_kw.update(content="", markup=False, rich=content)
            return
        text, is_markup = to_markup(content, self.__dict__.get("_markup", True))
        if self.is_mounted:
            self._native_call("update", text, is_markup)
        else:
            self._rs_kw.update(content=text, markup=is_markup, rich=None)

    @property
    def content(self) -> Any:
        return self.__dict__.get("_content", "")

    @property
    def renderable(self) -> Any:
        return self.content

    @property
    def visual(self) -> Any:
        return self.content


_NATIVE_CLASSES["Static"] = Static

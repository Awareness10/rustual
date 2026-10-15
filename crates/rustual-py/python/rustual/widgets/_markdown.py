from __future__ import annotations

from pathlib import Path, PurePath
from typing import Any, Callable, ClassVar

from .._bridge import _native
from ..message import Message
from ..widget import AwaitComplete, Widget, _NATIVE_CLASSES, _widget_for

TableOfContentsType = list[tuple[int, str, "str | None"]]
"""Table of contents entries: ``(level, label, block_id)``."""


def _find_markdown(widget: Widget | None) -> Markdown | None:
    """The `Markdown` at or above `widget` (or the document of an enclosing viewer)."""
    node = widget
    while node is not None:
        if isinstance(node, Markdown):
            return node
        if isinstance(node, MarkdownViewer):
            return node.document
        node = node.parent
    return None


class Markdown(Widget):
    """A Markdown document, rendered as a tree of block widgets."""

    _rs_kind: ClassVar[str] = "Markdown"
    _rs_framework: ClassVar[bool] = True

    class LinkClicked(Message):
        """Posted when a link in the document is clicked (``on_markdown_link_clicked``)."""

        def __init__(self, markdown: Markdown | None = None, href: str = "") -> None:
            super().__init__()
            self._markdown = markdown
            self.href = href

        @property
        def markdown(self) -> Markdown:
            if self._markdown is None:
                self._markdown = _find_markdown(self.sender)
            return self._markdown  # type: ignore[return-value]

        @property
        def control(self) -> Markdown:
            return self.markdown

    class TableOfContentsUpdated(Message):
        """Posted when the table of contents changes."""

        def __init__(self, markdown: Markdown | None = None, table_of_contents: TableOfContentsType | None = None) -> None:
            super().__init__()
            self._markdown = markdown
            self.table_of_contents = [tuple(entry) for entry in table_of_contents or []]

        @property
        def markdown(self) -> Markdown:
            if self._markdown is None:
                self._markdown = _find_markdown(self.sender)
            return self._markdown  # type: ignore[return-value]

        @property
        def control(self) -> Markdown:
            return self.markdown

    class TableOfContentsSelected(Message):
        """Posted when an entry of a table of contents is selected."""

        def __init__(self, markdown: Markdown | None = None, block_id: str = "") -> None:
            super().__init__()
            self._markdown = markdown
            self.block_id = block_id

        @property
        def markdown(self) -> Markdown | None:
            if self._markdown is None:
                self._markdown = _find_markdown(self.sender)
            return self._markdown

        @property
        def control(self) -> Markdown | None:
            return self.markdown

    def __init__(
        self,
        markdown: str | None = None,
        *,
        name: str | None = None,
        id: str | None = None,
        classes: str | None = None,
        parser_factory: Callable[[], Any] | None = None,
        open_links: bool = True,
    ) -> None:
        super().__init__(name=name, id=id, classes=classes)
        self._rs_kw = {"markdown": markdown or "", "open_links": open_links}

    @property
    def source(self) -> str:
        """The Markdown source."""
        return self._native_get("source", self.__dict__.get("_rs_kw", {}).get("markdown", ""))

    @property
    def table_of_contents(self) -> TableOfContentsType:
        """The document's headings: ``(level, label, block_id)``."""
        if not self.is_mounted:
            return []
        return [tuple(entry) for entry in self._native_get("table_of_contents", [])]  # type: ignore[misc]

    @property
    def open_links(self) -> bool:
        return self._native_get("open_links", True)

    @open_links.setter
    def open_links(self, value: bool) -> None:
        self._native_set("open_links", bool(value))

    def update(self, markdown: str) -> AwaitComplete:
        """Replace the document."""
        if self.is_mounted:
            self._native_call("update", markdown)
        else:
            self._rs_kw["markdown"] = markdown
        return AwaitComplete()

    def append(self, markdown: str) -> AwaitComplete:
        """Append Markdown to the document."""
        if self.is_mounted:
            self._native_call("append", markdown)
        else:
            self._rs_kw["markdown"] = self._rs_kw.get("markdown", "") + markdown
        return AwaitComplete()

    async def load(self, path: Path | str) -> None:
        """Load a document from a file (raises ``OSError`` if it can't be read)."""
        self._native_call("load", str(path))

    def goto_anchor(self, anchor: str) -> bool:
        """Scroll to the heading matching ``anchor``; returns ``False`` if there is none."""
        return bool(self._native_call("goto_anchor", anchor))


class MarkdownTableOfContents(Widget):
    """A table of contents for a Markdown document (created by :class:`MarkdownViewer`)."""

    _rs_kind: ClassVar[str] = "MarkdownTableOfContents"
    _rs_framework: ClassVar[bool] = True

    @property
    def table_of_contents(self) -> TableOfContentsType:
        if not self.is_mounted:
            return []
        return [tuple(entry) for entry in self._native_get("table_of_contents", [])]  # type: ignore[misc]


class _Navigator:
    """Read-only view of a MarkdownViewer's history."""

    def __init__(self, viewer: MarkdownViewer) -> None:
        self._viewer = viewer

    @property
    def location(self) -> PurePath:
        return PurePath(self._viewer._native_get("location", "."))

    @property
    def start(self) -> bool:
        """Is the current location the first in the history?"""
        return bool(self._viewer._native_get("navigator_start", True))

    @property
    def end(self) -> bool:
        """Is the current location the last in the history?"""
        return bool(self._viewer._native_get("navigator_end", True))


class MarkdownViewer(Widget):
    """A scrolling Markdown document with a table of contents and link navigation."""

    _rs_kind: ClassVar[str] = "MarkdownViewer"
    _rs_framework: ClassVar[bool] = True

    class NavigatorUpdated(Message):
        """Posted after navigating to a document (``on_markdown_viewer_navigator_updated``)."""

    def __init__(
        self,
        markdown: str | None = None,
        *,
        show_table_of_contents: bool = True,
        name: str | None = None,
        id: str | None = None,
        classes: str | None = None,
        parser_factory: Callable[[], Any] | None = None,
        open_links: bool = True,
    ) -> None:
        super().__init__(name=name, id=id, classes=classes)
        self._rs_kw = {
            "markdown": markdown or "",
            "show_table_of_contents": show_table_of_contents,
            "open_links": open_links,
        }

    @property
    def document(self) -> Markdown:
        """The :class:`Markdown` document."""
        return _widget_for(self._native_call("document"))

    @property
    def table_of_contents(self) -> MarkdownTableOfContents:
        return self.query_one(MarkdownTableOfContents)

    @property
    def show_table_of_contents(self) -> bool:
        return self._native_get("show_table_of_contents", True)

    @show_table_of_contents.setter
    def show_table_of_contents(self, value: bool) -> None:
        self._native_set("show_table_of_contents", bool(value))

    @property
    def navigator(self) -> _Navigator:
        """The viewer's history (``start`` / ``end`` / ``location``), like Textual's ``Navigator``."""
        return _Navigator(self)

    def go(self, location: str | PurePath) -> AwaitComplete:
        """Navigate to a document (``file.md``, ``file.md#anchor`` or ``#anchor``)."""
        return AwaitComplete(self._native_call("go", str(location)))

    def back(self) -> AwaitComplete:
        return AwaitComplete(self._native_call("back"))

    def forward(self) -> AwaitComplete:
        return AwaitComplete(self._native_call("forward"))


_native.register_class("Markdown.LinkClicked", lambda **kw: Markdown.LinkClicked(None, kw.get("href", "")))
_native.register_class(
    "Markdown.TableOfContentsUpdated",
    lambda **kw: Markdown.TableOfContentsUpdated(None, kw.get("table_of_contents")),
)
_native.register_class(
    "Markdown.TableOfContentsSelected",
    lambda **kw: Markdown.TableOfContentsSelected(None, kw.get("block_id", "")),
)
_native.register_class("MarkdownViewer.NavigatorUpdated", lambda **kw: MarkdownViewer.NavigatorUpdated())
_NATIVE_CLASSES["Markdown"] = Markdown
_NATIVE_CLASSES["MarkdownViewer"] = MarkdownViewer
_NATIVE_CLASSES["MarkdownTableOfContents"] = MarkdownTableOfContents

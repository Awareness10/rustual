from __future__ import annotations

from itertools import zip_longest
from typing import Any, ClassVar

from .._bridge import _native
from ..content import to_markup
from ..css.query import NoMatches
from ..message import Message
from ..widget import _NATIVE_CLASSES, AwaitComplete, Widget, _widget_for
from ._tabs import Tab, Tabs


class ContentTab(Tab):
    """A tab associated with a :class:`TabPane` (its id is the pane id with a prefix)."""

    _rs_framework: ClassVar[bool] = True
    _PREFIX = "--content-tab-"

    @classmethod
    def add_prefix(cls, content_id: str) -> str:
        return content_id if content_id.startswith(cls._PREFIX) else f"{cls._PREFIX}{content_id}"

    @classmethod
    def sans_prefix(cls, content_id: str) -> str:
        return content_id[len(cls._PREFIX) :] if content_id.startswith(cls._PREFIX) else content_id


class ContentTabs(Tabs):
    """The tabs of a :class:`TabbedContent` (created by it)."""

    _rs_framework: ClassVar[bool] = True


class TabPane(Widget):
    """A page of a :class:`TabbedContent`, with the title shown in its tab."""

    _rs_kind: ClassVar[str] = "TabPane"
    _rs_framework: ClassVar[bool] = True

    def __init__(
        self,
        title: Any,
        *children: Widget,
        name: str | None = None,
        id: str | None = None,
        classes: str | None = None,
        disabled: bool = False,
    ) -> None:
        super().__init__(*children, name=name, id=id, classes=classes, disabled=disabled)
        self._title = to_markup(title)[0]
        self._rs_kw = {"title": self._title}


class TabbedContent(Widget):
    """Tabs plus a content switcher showing the :class:`TabPane` of the active tab.

    Compose panes with ``with TabbedContent(): with TabPane("Title", id="a"): yield ...``.
    """

    _rs_kind: ClassVar[str] = "TabbedContent"
    _rs_framework: ClassVar[bool] = True

    class TabActivated(Message):
        """Posted when the active tab changes (``on_tabbed_content_tab_activated``)."""

        ALLOW_SELECTOR_MATCH = {"pane"}

        def __init__(
            self,
            tabbed_content: TabbedContent | None = None,
            tab: ContentTab | None = None,
            *,
            pane_id: str = "",
            tab_id: str = "",
        ) -> None:
            super().__init__()
            self._tabbed_content = tabbed_content
            self._tab = tab
            self._pane: TabPane | None = None
            self._pane_id = pane_id or (ContentTab.sans_prefix(tab.id or "") if tab is not None else "")

        @property
        def tabbed_content(self) -> TabbedContent:
            return self._tabbed_content if self._tabbed_content is not None else self.sender  # type: ignore[return-value]

        @property
        def tab(self) -> ContentTab:
            if self._tab is None:
                self._tab = self.tabbed_content.get_tab(self._pane_id)
            return self._tab  # type: ignore[return-value]

        @property
        def pane(self) -> TabPane:
            if self._pane is None:
                self._pane = self.tabbed_content.get_pane(self._pane_id)
            return self._pane

        @property
        def control(self) -> TabbedContent:
            return self.tabbed_content

    class Cleared(Message):
        """Posted when no tab pane is active (``on_tabbed_content_cleared``)."""

        def __init__(self, tabbed_content: TabbedContent | None = None) -> None:
            super().__init__()
            self._tabbed_content = tabbed_content

        @property
        def tabbed_content(self) -> TabbedContent:
            return self._tabbed_content if self._tabbed_content is not None else self.sender  # type: ignore[return-value]

        @property
        def control(self) -> TabbedContent:
            return self.tabbed_content

    def __init__(
        self,
        *titles: Any,
        initial: str = "",
        name: str | None = None,
        id: str | None = None,
        classes: str | None = None,
        disabled: bool = False,
    ) -> None:
        super().__init__(name=name, id=id, classes=classes, disabled=disabled)
        self.titles = [to_markup(title)[0] for title in titles]
        self._initial = initial
        self._tab_counter = 0
        self._rs_kw = {"initial": initial}

    def _set_id(self, pane: TabPane) -> TabPane:
        if pane.id is None:
            self._tab_counter = self.__dict__.get("_tab_counter", 0) + 1
            pane.id = f"tab-{self._tab_counter}"
        return pane

    def _rs_spec(self) -> dict[str, Any]:
        spec = super()._rs_spec()
        # Wrap plain children in panes and give every pane an id (Textual's compose).
        spec["children"] = [
            self._set_id(content if isinstance(content, TabPane) else TabPane(title or f"Tab {index}", content))
            for index, (title, content) in enumerate(zip_longest(self.titles, spec["children"]), 1)
            if content is not None
        ]
        return spec

    # -- state ---------------------------------------------------------------------------

    @property
    def active(self) -> str:
        """The id of the active pane, or ``""``."""
        if self.is_mounted:
            return self._native_call("active")
        return self.__dict__.get("_initial", "")

    @active.setter
    def active(self, pane_id: str) -> None:
        if self.is_mounted:
            self._native_set("active", pane_id)
        else:
            self._initial = pane_id
            self._rs_kw["initial"] = pane_id

    @property
    def active_pane(self) -> TabPane | None:
        if not self.is_mounted:
            return None
        return _widget_for(self._native_call("active_pane"))

    @property
    def tab_count(self) -> int:
        if not self.is_mounted:
            return len(self._pending_children)
        return self._native_call("tab_count")

    def get_tab(self, pane_id: str | TabPane) -> Tab:
        """The tab for a pane (or pane id)."""
        target = pane_id.id if isinstance(pane_id, TabPane) else pane_id
        node = self._native_call("get_tab", ContentTab.sans_prefix(target or ""))
        if node is None:
            raise ValueError(f"No Tab associated with the pane id {target!r}")
        return _widget_for(node)

    def get_pane(self, pane_id: str | ContentTab) -> TabPane:
        """The pane for a pane id (or its tab)."""
        target = pane_id.id if isinstance(pane_id, Tab) else pane_id
        node = self._native_call("get_pane", ContentTab.sans_prefix(target or ""))
        if node is None:
            raise NoMatches(f"No TabPane with id {target!r}")
        return _widget_for(node)

    # -- changing panes --------------------------------------------------------------------

    def add_pane(
        self,
        pane: TabPane,
        *,
        before: TabPane | str | None = None,
        after: TabPane | str | None = None,
    ) -> AwaitComplete:
        """Add a pane at the end, or ``before`` / ``after`` another pane."""
        if isinstance(before, TabPane):
            before = before.id
        if isinstance(after, TabPane):
            after = after.id
        if before is not None and after is not None:
            raise Tabs.TabError("Unable to add a tab both 'before' and 'after' a tab")
        self._set_id(pane)
        if not self.is_mounted:
            self._pending_children.append(pane)
            return AwaitComplete()
        try:
            self._native_call("add_pane", pane, before, after)
        except ValueError as error:
            raise Tabs.TabError(str(error)) from None
        return AwaitComplete()

    def remove_pane(self, pane_id: str) -> AwaitComplete:
        if self.is_mounted:
            self._native_call("remove_pane", pane_id)
        return AwaitComplete()

    def clear_panes(self) -> AwaitComplete:
        if self.is_mounted:
            self._native_call("clear_panes")
        else:
            self._pending_children.clear()
        return AwaitComplete()

    def _tab_op(self, op: str, tab_id: str) -> None:
        if not self._native_call(op, ContentTab.sans_prefix(tab_id)):
            raise Tabs.TabError(f"There is no tab with ID {tab_id!r}")

    def disable_tab(self, tab_id: str) -> None:
        self._tab_op("disable_tab", tab_id)

    def enable_tab(self, tab_id: str) -> None:
        self._tab_op("enable_tab", tab_id)

    def hide_tab(self, tab_id: str) -> None:
        self._tab_op("hide_tab", tab_id)

    def show_tab(self, tab_id: str) -> None:
        self._tab_op("show_tab", tab_id)


_native.register_class(
    "TabbedContent.TabActivated",
    lambda **kw: TabbedContent.TabActivated(pane_id=kw.get("pane_id", ""), tab_id=kw.get("tab_id", "")),
)
_native.register_class("TabbedContent.Cleared", lambda **kw: TabbedContent.Cleared())
_NATIVE_CLASSES["TabPane"] = TabPane
_NATIVE_CLASSES["TabbedContent"] = TabbedContent
_NATIVE_CLASSES["ContentTabs"] = ContentTabs

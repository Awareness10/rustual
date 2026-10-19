from __future__ import annotations

from typing import Any, ClassVar

from .._bridge import _native
from ..content import to_markup
from ..message import Message
from ..widget import _NATIVE_CLASSES, AwaitComplete, Widget, _widget_for


class Tab(Widget):
    """A single tab of a :class:`Tabs` widget."""

    _rs_kind: ClassVar[str] = "Tab"
    _rs_framework: ClassVar[bool] = True

    def __init__(
        self,
        label: Any,
        *,
        id: str | None = None,
        classes: str | None = None,
        disabled: bool = False,
    ) -> None:
        super().__init__(id=id, classes=classes, disabled=disabled)
        self._rs_kw = {"label": to_markup(label)[0]}

    @property
    def label(self) -> str:
        """The tab label (markup)."""
        return self._native_get("label", "")

    @label.setter
    def label(self, label: Any) -> None:
        self._native_set("label", to_markup(label)[0])

    @property
    def label_text(self) -> str:
        """The undecorated text of the label."""
        if self.is_mounted:
            return self._native_get("label_text", "")
        return self.__dict__.get("_rs_kw", {}).get("label", "")

    def update(self, content: Any = "") -> None:
        self.label = content


class Tabs(Widget):
    """A row of tabs; posts :class:`Tabs.TabActivated` when the active tab changes."""

    _rs_kind: ClassVar[str] = "Tabs"
    _rs_framework: ClassVar[bool] = True

    class TabError(Exception):
        """Raised when there is an error relating to tabs."""

    class TabMessage(Message):
        """Base class for messages about a tab."""

        ALLOW_SELECTOR_MATCH = {"tab"}

        def __init__(self, tabs: Tabs | None = None, tab: Tab | None = None, *, tab_id: str = "") -> None:
            super().__init__()
            self._tabs = tabs
            self._tab = tab
            self._tab_id = tab_id if tab is None else (tab.id or "")

        @property
        def tabs(self) -> Tabs:
            return self._tabs if self._tabs is not None else self.sender  # type: ignore[return-value]

        @property
        def tab(self) -> Tab | None:
            if self._tab is None and self._tab_id:
                self._tab = self.tabs.get_tab(self._tab_id)
            return self._tab

        @property
        def control(self) -> Tabs:
            return self.tabs

    class TabActivated(TabMessage):
        """Posted when a new tab is activated (``on_tabs_tab_activated``)."""

    class TabDisabled(TabMessage):
        """Posted when a tab is disabled."""

    class TabEnabled(TabMessage):
        """Posted when a tab is enabled."""

    class TabHidden(TabMessage):
        """Posted when a tab is hidden."""

    class TabShown(TabMessage):
        """Posted when a tab is shown."""

    class Cleared(Message):
        """Posted when there are no more tab objects (``on_tabs_cleared``)."""

        def __init__(self, tabs: Tabs | None = None) -> None:
            super().__init__()
            self._tabs = tabs

        @property
        def tabs(self) -> Tabs:
            return self._tabs if self._tabs is not None else self.sender  # type: ignore[return-value]

        @property
        def control(self) -> Tabs:
            return self.tabs

    def __init__(
        self,
        *tabs: Tab | Any,
        active: str | None = None,
        name: str | None = None,
        id: str | None = None,
        classes: str | None = None,
        disabled: bool = False,
    ) -> None:
        super().__init__(name=name, id=id, classes=classes, disabled=disabled)
        self._tabs_counter = 0
        self._pending_children.extend(self._make_tab(tab) for tab in tabs)
        self._rs_kw = {"active": active}

    def _new_tab_id(self) -> int:
        self._tabs_counter = self.__dict__.get("_tabs_counter", 0) + 1
        return self._tabs_counter

    def _make_tab(self, tab: Tab | Any) -> Tab:
        if isinstance(tab, Tab):
            if tab.id is None:
                tab.id = f"tab-{self._new_tab_id()}"
            return tab
        return Tab(tab, id=f"tab-{self._new_tab_id()}")

    # -- state ---------------------------------------------------------------------------

    @property
    def active(self) -> str:
        """The id of the active tab, or ``""``."""
        if self.is_mounted:
            return self._native_call("active")
        return self._rs_kw.get("active") or ""

    @active.setter
    def active(self, tab_id: str) -> None:
        if self.is_mounted:
            self._native_set("active", tab_id)
        else:
            self._rs_kw["active"] = tab_id

    @property
    def active_tab(self) -> Tab | None:
        if not self.is_mounted:
            return None
        return _widget_for(self._native_call("active_tab"))

    @property
    def tab_count(self) -> int:
        if not self.is_mounted:
            return sum(isinstance(c, Tab) for c in self._pending_children)
        return self._native_call("tab_count")

    def get_tab(self, tab_id: str) -> Tab | None:
        if not self.is_mounted:
            return next((t for t in self._pending_children if t.id == tab_id), None)
        return _widget_for(self._native_call("get_tab", tab_id))

    # -- changing tabs ---------------------------------------------------------------------

    def add_tab(
        self, tab: Tab | Any, *, before: Tab | str | None = None, after: Tab | str | None = None
    ) -> AwaitComplete:
        """Add a tab (a :class:`Tab` or a label) at the end, or ``before`` / ``after`` another."""
        if before is not None and after is not None:
            raise Tabs.TabError("Unable to add a tab both 'before' and 'after' a tab")
        new_tab = self._make_tab(tab)
        if isinstance(before, Tab):
            before = before.id
        if isinstance(after, Tab):
            after = after.id
        if not self.is_mounted:
            self._pending_children.append(new_tab)
            return AwaitComplete()
        try:
            self._native_call("add_tab", new_tab, before, after)
        except ValueError as error:
            raise Tabs.TabError(str(error)) from None
        return AwaitComplete()

    def remove_tab(self, tab_or_id: Tab | str | None) -> AwaitComplete:
        if tab_or_id is None:
            return AwaitComplete()
        tab_id = tab_or_id.id if isinstance(tab_or_id, Tab) else tab_or_id
        if self.is_mounted and tab_id:
            self._native_call("remove_tab", tab_id)
        return AwaitComplete()

    def clear(self) -> AwaitComplete:
        if self.is_mounted:
            self._native_call("clear")
        else:
            self._pending_children.clear()
        return AwaitComplete()

    def _tab_op(self, op: str, tab_id: str) -> Tab:
        if not self._native_call(op, tab_id):
            raise Tabs.TabError(f"There is no tab with ID {tab_id!r} to {op}.")
        return self.get_tab(tab_id)  # type: ignore[return-value]

    def disable(self, tab_id: str) -> Tab:
        return self._tab_op("disable", tab_id)

    def enable(self, tab_id: str) -> Tab:
        return self._tab_op("enable", tab_id)

    def hide(self, tab_id: str) -> Tab:
        return self._tab_op("hide", tab_id)

    def show(self, tab_id: str) -> Tab:
        return self._tab_op("show", tab_id)

    def action_next_tab(self) -> None:
        self._native_call("next_tab")

    def action_previous_tab(self) -> None:
        self._native_call("previous_tab")


def _tab_message(cls: type) -> Any:
    return lambda **kw: cls(None, None, tab_id=kw.get("tab_id", ""))


_native.register_class("Tabs.TabActivated", _tab_message(Tabs.TabActivated))
_native.register_class("Tabs.TabDisabled", _tab_message(Tabs.TabDisabled))
_native.register_class("Tabs.TabEnabled", _tab_message(Tabs.TabEnabled))
_native.register_class("Tabs.TabHidden", _tab_message(Tabs.TabHidden))
_native.register_class("Tabs.TabShown", _tab_message(Tabs.TabShown))
_native.register_class("Tabs.Cleared", lambda **kw: Tabs.Cleared())
_NATIVE_CLASSES["Tab"] = Tab
_NATIVE_CLASSES["Tabs"] = Tabs

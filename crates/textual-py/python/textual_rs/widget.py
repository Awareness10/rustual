"""The ``Widget`` base class (Textual's ``textual.widget`` + ``textual.dom``).

A Python widget is mounted in the Rust DOM through an adapter: the Rust side calls the
``_rs_*`` hooks below (compose, mount, events, messages, render, actions), and the methods
here call back into the engine through :data:`textual_rs._bridge.native` using the widget's
node id. Built-in widgets (``Button``, ``Input``, ...) are Python subclasses whose
``_rs_kind`` names the Rust widget that backs them.
"""

from __future__ import annotations

import asyncio
import inspect
import threading
from typing import TYPE_CHECKING, Any, Awaitable, Callable, ClassVar, Iterable, Iterator, TypeVar

from . import _context, events
from ._bridge import native
from .binding import Binding, collect_bindings
from .css.query import DOMQuery, NoMatches, TooManyMatches, WrongType
from .geometry import Offset, Region, Size
from .message import Message
from .reactive import Reactive, count_parameters

if TYPE_CHECKING:
    from .app import App
    from .screen import Screen
    from .worker import Worker

WidgetType = TypeVar("WidgetType", bound="Widget")


class MountError(Exception):
    pass


class WidgetError(Exception):
    pass


# ------------------------------------------------------------------------------- awaitables


class AwaitComplete:
    """An awaitable for operations that already completed (mount, remove, ...).

    Mounting and removal are applied by the engine before the next frame; awaiting this
    object never blocks, so ``await self.mount(...)`` works in sync and async code alike.
    """

    def __init__(self, result: Any = None) -> None:
        self._result = result

    def __await__(self) -> Iterator[Any]:
        return self._result
        yield  # pragma: no cover - makes this a generator

    def __call__(self) -> Any:
        return self._result


class AwaitMount(AwaitComplete):
    pass


AwaitRemove = AwaitComplete


def _run_awaitable(owner: Any, awaitable: Awaitable[Any]) -> Any:
    """Run a coroutine returned by a handler on the app's asyncio loop."""
    return _context.run_coroutine(awaitable)


def _invoke(method: Callable[..., Any], *args: Any) -> Any:
    """Call a handler with as many of ``args`` as it accepts; run it if it's async."""
    count = count_parameters(method)
    result = method(*args[:count]) if count else method()
    if inspect.isawaitable(result):
        return _run_awaitable(None, result)
    return result


# ------------------------------------------------------------------------------- node lookup

# Python classes for Rust type names (proxies for nodes created by Rust widgets).
_NATIVE_CLASSES: dict[str, type[Widget]] = {}
_proxies: dict[int, Widget] = {}


def _widget_for(node: int | None) -> Any:
    """The Python object for a node id."""
    if node is None:
        return None
    obj = native.node_object(node)
    if obj is not None:
        return obj
    proxy = _proxies.get(node)
    if proxy is None:
        names = native.type_names(node)
        cls: type[Widget] = Widget
        for name in names:
            if name in _NATIVE_CLASSES:
                cls = _NATIVE_CLASSES[name]
                break
        proxy = cls.__new__(cls)
        proxy._node = node
        proxy._rs_proxy = True
        _proxies[node] = proxy
    return proxy


def _selector(selector: Any) -> str:
    if selector is None:
        return "*"
    if isinstance(selector, str):
        return selector
    if isinstance(selector, type):
        return selector.__name__
    raise TypeError(f"expected a selector string or widget type, got {selector!r}")


# ------------------------------------------------------------------------------- compose stack

_compose = threading.local()


def _stacks() -> tuple[list[list[Widget]], list[list[Widget]]]:
    if not hasattr(_compose, "stacks"):
        _compose.stacks = []
        _compose.composed = []
    return _compose.stacks, _compose.composed


def compose(node: Any) -> list[Widget]:
    """Run ``node.compose()`` and collect the widgets (supporting ``with Container():``)."""
    stacks, composed_lists = _stacks()
    nodes: list[Widget] = []
    compose_stack: list[Widget] = []
    composed: list[Widget] = []
    stacks.append(compose_stack)
    composed_lists.append(composed)
    try:
        result = node.compose()
        if result is None:
            result = ()
        for child in result:
            if not isinstance(child, Widget):
                raise MountError(f"Can't mount {type(child)}; expected a Widget instance.")
            if composed:
                nodes.extend(composed)
                composed.clear()
            if compose_stack:
                compose_stack[-1]._pending_children.append(child)
            else:
                nodes.append(child)
        if composed:
            nodes.extend(composed)
            composed.clear()
    finally:
        stacks.pop()
        composed_lists.pop()
    return nodes


# ------------------------------------------------------------------------------- styles proxy


def _css_value(value: Any) -> str:
    if value is None:
        return "initial"
    if isinstance(value, bool):
        return "true" if value else "false"
    if isinstance(value, (tuple, list)):
        return " ".join(_css_value(v) for v in value)
    return str(value)


class Styles:
    """``widget.styles``: set CSS properties inline (``widget.styles.background = "red"``)."""

    def __init__(self, widget: Widget) -> None:
        object.__setattr__(self, "_widget", widget)

    def __setattr__(self, name: str, value: Any) -> None:
        widget: Widget = object.__getattribute__(self, "_widget")
        prop = name.replace("_", "-")
        widget._inline[prop] = value
        widget._apply_inline(f"{prop}: {_css_value(value)}")

    def __getattr__(self, name: str) -> Any:
        widget: Widget = object.__getattribute__(self, "_widget")
        prop = name.replace("_", "-")
        if prop in ("display", "visibility") and widget.is_mounted:
            return native.computed_style(widget._node, prop)
        return widget._inline.get(prop)

    def parse_inline(self, css: str) -> None:
        widget: Widget = object.__getattribute__(self, "_widget")
        widget._apply_inline(css)

    def animate(self, attribute: str, value: Any, **kwargs: Any) -> None:
        """Animate a numeric style (e.g. ``opacity``, ``offset-x``, ``width``) to ``value``."""
        widget: Widget = object.__getattribute__(self, "_widget")

        def get(target: Any, attr: str) -> Any:
            current = widget._inline.get(attr.replace("_", "-"))
            if isinstance(current, (int, float)):
                return current
            return 1.0 if attr == "opacity" else 0

        _context_animator().animate(self, attribute, value, getter=get, **kwargs)

    def refresh(self, layout: bool = False) -> None:
        native.refresh()


# ------------------------------------------------------------------------------- Widget


class Widget:
    """Base class for all widgets."""

    _rs_kind: ClassVar[str] = "Widget"
    _rs_framework: ClassVar[bool] = True
    _inherit_bindings: ClassVar[bool] = True

    DEFAULT_CSS: ClassVar[str] = ""
    DEFAULT_CLASSES: ClassVar[str] = ""
    COMPONENT_CLASSES: ClassVar[set[str]] = set()
    BINDINGS: ClassVar[list[Any]] = []
    HELP: ClassVar[str | None] = None
    SCOPED_CSS: ClassVar[bool] = True
    ALLOW_MAXIMIZE: ClassVar[bool | None] = None
    can_focus: bool = False
    can_focus_children: bool = True

    # Instance defaults (also valid for proxies created without __init__).
    _node: int | None = None
    _rs_proxy: bool = False
    _id: str | None = None
    _name: str | None = None
    _init_classes: tuple[str, ...] = ()
    _init_disabled: bool = False
    _border_title: str | None = None
    _border_subtitle: str | None = None
    _tooltip: str | None = None

    def __init_subclass__(
        cls,
        can_focus: bool | None = None,
        can_focus_children: bool | None = None,
        inherit_css: bool = True,
        inherit_bindings: bool = True,
        inherit_component_classes: bool = True,
        **kwargs: Any,
    ) -> None:
        super().__init_subclass__(**kwargs)
        cls._inherit_bindings = inherit_bindings
        if can_focus is not None:
            cls.can_focus = can_focus
        if can_focus_children is not None:
            cls.can_focus_children = can_focus_children
        if "_rs_framework" not in cls.__dict__:
            cls._rs_framework = False
        # Collect @on handlers declared in this class.
        decorated: dict[type, list[tuple[Callable[..., Any], dict[str, str]]]] = {}
        for value in cls.__dict__.values():
            for message_type, selectors in getattr(value, "_textual_on", ()):
                decorated.setdefault(message_type, []).append((value, selectors))
        cls._decorated_handlers = decorated

    def __init__(
        self,
        *children: Widget,
        name: str | None = None,
        id: str | None = None,
        classes: str | None = None,
        disabled: bool = False,
        markup: bool = True,
    ) -> None:
        self._node = None
        self._id = id
        self._name = name
        classes_list = list(self.DEFAULT_CLASSES.split()) + (classes.split() if classes else [])
        self._init_classes = tuple(classes_list)
        self._init_disabled = disabled
        self._markup = markup
        self._pending_children: list[Widget] = list(children)
        self._rs_kw: dict[str, Any] = {}
        for child in children:
            if not isinstance(child, Widget):
                raise MountError(f"Can't mount {type(child)}; expected a Widget instance.")

    # -- internal state helpers ------------------------------------------------------------

    @property
    def _inline(self) -> dict[str, Any]:
        d = self.__dict__.get("_inline_d")
        if d is None:
            d = self.__dict__["_inline_d"] = {}
        return d

    @property
    def _inline_css(self) -> list[str]:
        d = self.__dict__.get("_inline_css_d")
        if d is None:
            d = self.__dict__["_inline_css_d"] = []
        return d

    def _apply_inline(self, css: str) -> None:
        if self.is_mounted:
            native.set_inline_style(self._node, css)
        else:
            self._inline_css.append(css)

    def _rs_kwargs(self) -> dict[str, Any]:
        """Constructor arguments of the native base widget."""
        return dict(self.__dict__.get("_rs_kw", {}))

    def _native_get(self, attr: str, default: Any = None) -> Any:
        if self.is_mounted:
            return native.widget_get(self._node, attr)
        return self.__dict__.get("_rs_kw", {}).get(attr, default)

    def _native_set(self, attr: str, value: Any) -> None:
        if self.is_mounted:
            native.widget_set(self._node, attr, value)
        else:
            self.__dict__.setdefault("_rs_kw", {})[attr] = value

    def _native_call(self, method: str, *args: Any) -> Any:
        if not self.is_mounted:
            raise WidgetError(f"{self!r} is not mounted")
        return native.widget_call(self._node, method, args)

    # -- hooks called by the engine ---------------------------------------------------------

    @classmethod
    def _rs_class_info(cls) -> dict[str, Any]:
        type_names: list[str] = []
        for klass in cls.__mro__:
            if not isinstance(klass, type) or not issubclass(klass, Widget):
                continue
            if klass.__name__ in ("Widget", "App", "DOMNode") and klass.__dict__.get("_rs_framework"):
                continue
            if klass.__name__ not in type_names:
                type_names.append(klass.__name__)
        css_parts = [
            klass.__dict__["DEFAULT_CSS"]
            for klass in reversed(cls.__mro__)
            if "DEFAULT_CSS" in klass.__dict__ and klass.__dict__["DEFAULT_CSS"]
        ]
        handlers: set[str] = set()
        for klass in cls.__mro__:
            for attr in klass.__dict__:
                if attr.startswith(("on_", "key_", "action_")):
                    handlers.add(attr)
                elif attr.startswith("_on_"):
                    handlers.add(attr[1:])
            for message_type in klass.__dict__.get("_decorated_handlers", {}):
                for mcls in _message_subclasses(message_type):
                    handlers.add(mcls.handler_name)

        def user_defines(name: str) -> bool:
            return getattr(cls, name) is not getattr(Widget, name)

        return {
            "type_names": type_names,
            "default_css": "\n".join(css_parts),
            "handlers": sorted(handlers),
            "has_render": user_defines("render"),
            "has_compose": user_defines("compose"),
            "has_content_width": user_defines("get_content_width"),
            "has_content_height": user_defines("get_content_height"),
            "has_check_action": user_defines("check_action"),
            "bindings": [b._native() for b in collect_bindings(cls)],
            "help": cls.HELP,
        }

    def _rs_spec(self) -> dict[str, Any]:
        if self._node is not None and self.is_mounted:
            raise MountError(f"{self!r} is already mounted")
        styles = ";".join(self._inline_css) or None
        children = list(self._pending_children)
        self._pending_children = []
        return {
            "kind": self._rs_kind,
            "kwargs": self._rs_kwargs(),
            "id": self._id,
            "classes": list(self._init_classes),
            "styles": styles,
            "disabled": self._init_disabled,
            "border_title": self._border_title,
            "border_subtitle": self._border_subtitle,
            "tooltip": self._tooltip,
            "children": children,
            "can_focus": bool(self.can_focus) if self._explicit("can_focus") else None,
            "can_focus_children": bool(self.can_focus_children) if self._explicit("can_focus_children") else None,
        }

    def _explicit(self, name: str) -> bool:
        """Whether ``name`` is set by the instance or a user (non-framework) class."""
        if name in self.__dict__:
            return True
        return any(
            name in klass.__dict__ and not klass.__dict__.get("_rs_framework", False)
            for klass in type(self).__mro__
        ) or (self._rs_kind == "Widget" and name == "can_focus")

    def _rs_compose(self) -> list[Widget]:
        return compose(self)

    def _rs_on_mount(self) -> None:
        Reactive._initialize_object(self)
        self._dispatch(events.Mount())

    def _rs_on_unmount(self) -> None:
        self._dispatch(events.Unmount())

    def _rs_event(self, data: tuple) -> int:
        event = events.from_native(data)
        self._dispatch(event)
        if isinstance(event, events.Key) and not event._stop_propagation and not event._no_default_action:
            method = getattr(self, f"key_{event.name}", None)
            if method is not None:
                _invoke(method, event)
                event.stop()
                event.prevent_default()
        return _flow_bits(event)

    def _rs_message(self, message: Message) -> int:
        self._dispatch(message)
        return _flow_bits(message)

    def _rs_render(self) -> Any:
        return self.render()

    def _rs_action(self, name: str, args: list[str]) -> bool:
        method = getattr(self, f"action_{name}", None)
        if method is None:
            return False
        import ast

        parsed: list[Any] = []
        for a in args:
            try:
                parsed.append(ast.literal_eval(a))
            except (ValueError, SyntaxError):
                parsed.append(a)
        result = method(*parsed)
        if inspect.isawaitable(result):
            _run_awaitable(self, result)
        return True

    def _rs_check_action(self, name: str, args: list[str]) -> bool | None:
        import ast

        parsed: list[Any] = []
        for a in args:
            try:
                parsed.append(ast.literal_eval(a))
            except (ValueError, SyntaxError):
                parsed.append(a)
        return self.check_action(name, tuple(parsed))

    def check_action(self, action: str, parameters: tuple[object, ...]) -> bool | None:
        """Return ``True`` to allow ``action``, ``False`` to hide its bindings, ``None`` to
        show them disabled. Re-evaluated on every refresh (see :meth:`refresh_bindings`)."""
        return True

    def refresh_bindings(self) -> None:
        native.refresh()

    def _rs_content_width(self, width: int, height: int) -> int | None:
        return self.get_content_width(Size(width, height), Size(width, height))

    def _rs_content_height(self, width: int, height: int, content_width: int) -> int | None:
        return self.get_content_height(Size(width, height), Size(width, height), content_width)

    def _rs_invoke(self, callback: Callable[..., Any]) -> None:
        result = callback()
        if inspect.isawaitable(result):
            _run_awaitable(self, result)

    # -- dispatch ------------------------------------------------------------------------------

    def _get_dispatch_methods(self, method_name: str, message: Message) -> Iterator[Callable[..., Any]]:
        dispatched: set[Any] = set()
        message_mro = [c for c in type(message).__mro__ if isinstance(c, type) and issubclass(c, Message)]
        private_name = f"_{method_name}"
        for cls in type(self).__mro__:
            if message._no_default_action:
                break
            decorated = cls.__dict__.get("_decorated_handlers")
            if decorated:
                for mcls in message_mro:
                    for method, selectors in decorated.get(mcls, ()):
                        if method in dispatched:
                            continue
                        if selectors and not _selectors_match(message, selectors):
                            continue
                        dispatched.add(method)
                        yield method.__get__(self, cls)
            method = cls.__dict__.get(private_name) or cls.__dict__.get(method_name)
            if method is not None and not getattr(method, "_textual_on", None):
                yield method.__get__(self, cls)

    def _dispatch(self, message: Message) -> None:
        for method in self._get_dispatch_methods(message.handler_name, message):
            _invoke(method, message)

    # -- overridables ----------------------------------------------------------------------------

    def compose(self) -> Iterable[Widget]:
        """Yield child widgets (use ``with Container():`` blocks to nest)."""
        return ()

    def render(self) -> Any:
        """Return the widget's content: markup ``str``, a ``Content`` or a Rich ``Text``."""
        return ""

    def get_content_width(self, container: Size, viewport: Size) -> int | None:
        return None

    def get_content_height(self, container: Size, viewport: Size, width: int) -> int | None:
        return None

    # -- context manager compose ------------------------------------------------------------------

    def __enter__(self: WidgetType) -> WidgetType:
        stacks, _ = _stacks()
        if not stacks:
            raise MountError("'with' blocks are only valid inside compose()")
        stacks[-1].append(self)
        return self

    def __exit__(self, *exc: Any) -> None:
        stacks, composed_lists = _stacks()
        compose_stack = stacks[-1]
        composed = compose_stack.pop()
        if compose_stack:
            compose_stack[-1]._pending_children.append(composed)
        else:
            composed_lists[-1].append(composed)

    def compose_add_child(self, widget: Widget) -> None:
        self._pending_children.append(widget)

    def __repr__(self) -> str:
        parts = [type(self).__name__]
        if self._id:
            parts.append(f"id={self._id!r}")
        return f"{parts[0]}({', '.join(parts[1:])})"

    # -- identity & tree -----------------------------------------------------------------------

    @property
    def id(self) -> str | None:
        if self._rs_proxy and self._node is not None:
            return native.css_id(self._node)
        return self._id

    @id.setter
    def id(self, value: str | None) -> None:
        if self.is_mounted:
            raise ValueError("can't change the id of a mounted widget")
        self._id = value

    @property
    def name(self) -> str | None:
        return self._name

    @property
    def is_mounted(self) -> bool:
        if self._node is None:
            return False
        try:
            return bool(native.is_mounted(self._node))
        except RuntimeError:
            return False

    @property
    def is_attached(self) -> bool:
        return self.is_mounted

    @property
    def app(self) -> App:
        return _context.active_app()

    @property
    def parent(self) -> Widget | None:
        if not self.is_mounted:
            return None
        return _widget_for(native.parent(self._node))

    @property
    def children(self) -> list[Widget]:
        if not self.is_mounted:
            return list(self._pending_children)
        return [_widget_for(n) for n in native.children(self._node)]

    @property
    def screen(self) -> Screen:
        if not self.is_mounted:
            raise NoMatches(f"{self!r} is not mounted")
        node = native.screen_of(self._node)
        if node is None:
            return self.app.screen
        return _widget_for(node)

    @property
    def ancestors_with_self(self) -> list[Widget]:
        result: list[Widget] = [self]
        node = self.parent
        while node is not None:
            result.append(node)
            node = node.parent
        return result

    @property
    def ancestors(self) -> list[Widget]:
        return self.ancestors_with_self[1:]

    def walk_children(self, filter_type: type[WidgetType] | None = None, *, with_self: bool = False) -> list[Any]:
        if not self.is_mounted:
            return []
        nodes = native.query(self._node, "*")
        if not with_self:
            nodes = [n for n in nodes if n != self._node]
        widgets = [_widget_for(n) for n in nodes]
        return [w for w in widgets if filter_type is None or isinstance(w, filter_type)]

    # -- queries ---------------------------------------------------------------------------------

    def _query_nodes(self, selector: str) -> list[Any]:
        if not self.is_mounted:
            return []
        nodes = native.query(self._node, selector)
        return [_widget_for(n) for n in nodes if n != self._node]

    def query(self, selector: str | type[WidgetType] | None = None) -> DOMQuery[Any]:
        sel = _selector(selector)
        return DOMQuery(self._query_nodes(sel), sel)

    def query_children(self, selector: str | type[WidgetType] | None = None) -> DOMQuery[Any]:
        sel = _selector(selector)
        kids = self.children
        return DOMQuery([k for k in kids if k.match(sel)], sel)

    def query_one(self, selector: str | type[WidgetType], expect_type: type[Any] | None = None) -> Any:
        if expect_type is None and isinstance(selector, type):
            expect_type = selector
        sel = _selector(selector)
        nodes = self._query_nodes(sel)
        if not nodes:
            raise NoMatches(f"No nodes match {sel!r} on {self!r}")
        node = nodes[0]
        if expect_type is not None and not isinstance(node, expect_type):
            raise WrongType(f"Node matching {sel!r} is the wrong type; expected {expect_type.__name__}, got {type(node).__name__}")
        return node

    def query_exactly_one(self, selector: str | type[WidgetType], expect_type: type[Any] | None = None) -> Any:
        sel = _selector(selector)
        nodes = self._query_nodes(sel)
        if len(nodes) > 1:
            raise TooManyMatches(f"Expected one node matching {sel!r}, found {len(nodes)}")
        return self.query_one(selector, expect_type)

    def get_child_by_id(self, id: str, expect_type: type[Any] | None = None) -> Any:
        for child in self.children:
            if child.id == id:
                if expect_type is not None and not isinstance(child, expect_type):
                    raise WrongType(f"child {id!r} is {type(child).__name__}, expected {expect_type.__name__}")
                return child
        raise NoMatches(f"No child with id {id!r}")

    def get_widget_by_id(self, id: str, expect_type: type[Any] | None = None) -> Any:
        return self.query_one(f"#{id}", expect_type)

    def get_child_by_type(self, expect_type: type[WidgetType]) -> WidgetType:
        for child in self.children:
            if type(child) is expect_type:
                return child  # type: ignore[return-value]
        raise NoMatches(f"No immediate child of type {expect_type.__name__}")

    def match(self, selector: str) -> bool:
        if not self.is_mounted:
            return False
        return bool(native.matches(self._node, selector))

    # -- classes -----------------------------------------------------------------------------

    @property
    def classes(self) -> frozenset[str]:
        if self.is_mounted:
            return frozenset(native.classes(self._node))
        return frozenset(self._init_classes)

    @classes.setter
    def classes(self, value: str | Iterable[str]) -> None:
        self.set_classes(value)

    def has_class(self, *class_names: str) -> bool:
        current = self.classes
        return all(c in current for c in class_names)

    def add_class(self: WidgetType, *class_names: str, update: bool = True) -> WidgetType:
        names = [n for c in class_names for n in c.split()]
        if self.is_mounted:
            native.update_classes(self._node, "add", names)
        else:
            self._init_classes = tuple(dict.fromkeys(self._init_classes + tuple(names)))
        return self

    def remove_class(self: WidgetType, *class_names: str, update: bool = True) -> WidgetType:
        names = [n for c in class_names for n in c.split()]
        if self.is_mounted:
            native.update_classes(self._node, "remove", names)
        else:
            self._init_classes = tuple(c for c in self._init_classes if c not in names)
        return self

    def toggle_class(self: WidgetType, *class_names: str) -> WidgetType:
        names = [n for c in class_names for n in c.split()]
        if self.is_mounted:
            native.update_classes(self._node, "toggle", names)
        else:
            current = list(self._init_classes)
            for n in names:
                if n in current:
                    current.remove(n)
                else:
                    current.append(n)
            self._init_classes = tuple(current)
        return self

    def set_class(self: WidgetType, add: bool, *class_names: str, update: bool = True) -> WidgetType:
        return self.add_class(*class_names) if add else self.remove_class(*class_names)

    def set_classes(self: WidgetType, classes: str | Iterable[str]) -> WidgetType:
        names = classes.split() if isinstance(classes, str) else list(classes)
        if self.is_mounted:
            native.set_classes(self._node, names)
        else:
            self._init_classes = tuple(names)
        return self

    # -- styles & state ------------------------------------------------------------------------

    @property
    def styles(self) -> Styles:
        s = self.__dict__.get("_styles")
        if s is None:
            s = self.__dict__["_styles"] = Styles(self)
        return s

    @property
    def display(self) -> bool:
        if self.is_mounted:
            return native.computed_style(self._node, "display") != "none"
        return self._inline.get("display") != "none"

    @display.setter
    def display(self, value: bool | str) -> None:
        if isinstance(value, str):
            value = value != "none"
        if self.is_mounted:
            native.set_display(self._node, bool(value))
        else:
            self.styles.display = "block" if value else "none"

    @property
    def visible(self) -> bool:
        if self.is_mounted:
            return native.computed_style(self._node, "visibility") != "hidden"
        return self._inline.get("visibility") != "hidden"

    @visible.setter
    def visible(self, value: bool | str) -> None:
        if isinstance(value, str):
            value = value != "hidden"
        self.styles.visibility = "visible" if value else "hidden"

    @property
    def disabled(self) -> bool:
        if self.is_mounted:
            return bool(native.is_disabled(self._node))
        return self._init_disabled

    @disabled.setter
    def disabled(self, value: bool) -> None:
        if self.is_mounted:
            native.set_disabled(self._node, bool(value))
        else:
            self._init_disabled = bool(value)

    @property
    def loading(self) -> bool:
        return bool(self.__dict__.get("_loading", False))

    @loading.setter
    def loading(self, value: bool) -> None:
        self.__dict__["_loading"] = bool(value)
        if self.is_mounted:
            native.set_loading(self._node, bool(value))

    def set_loading(self, loading: bool) -> AwaitComplete:
        self.loading = loading
        return AwaitComplete()

    def _set_node_text(self, which: str, attr: str, value: str | None) -> None:
        setattr(self, attr, value)
        if self.is_mounted:
            native.set_node_text(self._node, which, value)

    @property
    def border_title(self) -> str | None:
        return self._border_title

    @border_title.setter
    def border_title(self, value: str | None) -> None:
        self._set_node_text("title", "_border_title", None if value is None else str(value))

    @property
    def border_subtitle(self) -> str | None:
        return self._border_subtitle

    @border_subtitle.setter
    def border_subtitle(self, value: str | None) -> None:
        self._set_node_text("subtitle", "_border_subtitle", None if value is None else str(value))

    @property
    def tooltip(self) -> str | None:
        return self._tooltip

    @tooltip.setter
    def tooltip(self, value: str | None) -> None:
        self._set_node_text("tooltip", "_tooltip", None if value is None else str(value))

    # -- geometry --------------------------------------------------------------------------------

    @property
    def region(self) -> Region:
        if not self.is_mounted:
            return Region()
        return Region(*native.region(self._node, "outer"))

    @property
    def content_region(self) -> Region:
        if not self.is_mounted:
            return Region()
        return Region(*native.region(self._node, "content"))

    @property
    def size(self) -> Size:
        """Size of the content region."""
        return self.content_region.size

    @property
    def content_size(self) -> Size:
        return self.content_region.size

    @property
    def outer_size(self) -> Size:
        return self.region.size

    @property
    def container_size(self) -> Size:
        return self.content_region.size

    # -- scrolling -----------------------------------------------------------------------------

    def _scroll_state(self) -> tuple[int, int, int, int]:
        if not self.is_mounted:
            return (0, 0, 0, 0)
        return native.scroll_offset(self._node)

    @property
    def scroll_offset(self) -> Offset:
        x, y, _, _ = self._scroll_state()
        return Offset(x, y)

    @property
    def scroll_x(self) -> int:
        return self._scroll_state()[0]

    @scroll_x.setter
    def scroll_x(self, value: float) -> None:
        self.scroll_to(x=value, animate=False)

    @property
    def scroll_y(self) -> int:
        return self._scroll_state()[1]

    @scroll_y.setter
    def scroll_y(self, value: float) -> None:
        self.scroll_to(y=value, animate=False)

    @property
    def max_scroll_x(self) -> int:
        return self._scroll_state()[2]

    @property
    def max_scroll_y(self) -> int:
        return self._scroll_state()[3]

    @property
    def is_vertical_scroll_end(self) -> bool:
        x, y, mx, my = self._scroll_state()
        return y >= my

    def scroll_to(self, x: float | None = None, y: float | None = None, *, animate: bool = True, **kwargs: Any) -> None:
        cx, cy, _, _ = self._scroll_state()
        native.scroll_to(self._node, int(cx if x is None else x), int(cy if y is None else y))

    def scroll_relative(self, x: float | None = None, y: float | None = None, **kwargs: Any) -> None:
        cx, cy, _, _ = self._scroll_state()
        self.scroll_to(cx + (x or 0), cy + (y or 0))

    def scroll_home(self, **kwargs: Any) -> None:
        self.scroll_to(0, 0)

    def scroll_end(self, **kwargs: Any) -> None:
        _, _, mx, my = self._scroll_state()
        self.scroll_to(0, my)

    def scroll_up(self, **kwargs: Any) -> None:
        self.scroll_relative(y=-1)

    def scroll_down(self, **kwargs: Any) -> None:
        self.scroll_relative(y=1)

    def scroll_left(self, **kwargs: Any) -> None:
        self.scroll_relative(x=-1)

    def scroll_right(self, **kwargs: Any) -> None:
        self.scroll_relative(x=1)

    def scroll_page_up(self, **kwargs: Any) -> None:
        self.scroll_relative(y=-max(1, self.size.height))

    def scroll_page_down(self, **kwargs: Any) -> None:
        self.scroll_relative(y=max(1, self.size.height))

    def scroll_visible(self, *args: Any, **kwargs: Any) -> None:
        if self.is_mounted:
            native.scroll_visible(self._node)

    def scroll_to_widget(self, widget: Widget, **kwargs: Any) -> bool:
        widget.scroll_visible()
        return True

    def scroll_to_center(self, widget: Widget, **kwargs: Any) -> None:
        widget.scroll_visible()

    # -- focus --------------------------------------------------------------------------------

    @property
    def has_focus(self) -> bool:
        return self.is_mounted and native.focused() == self._node

    @property
    def has_focus_within(self) -> bool:
        if not self.is_mounted:
            return False
        focused = native.focused()
        return focused is not None and focused in native.query(self._node, "*")

    @property
    def is_mouse_over(self) -> bool:
        return self.is_mounted and native.hovered() == self._node

    def focus(self: WidgetType, scroll_visible: bool = True) -> WidgetType:
        if self.is_mounted:
            native.focus(self._node)
        return self

    def blur(self: WidgetType) -> WidgetType:
        if self.has_focus:
            native.blur()
        return self

    # -- mounting --------------------------------------------------------------------------------

    def mount(
        self,
        *widgets: Widget,
        before: int | str | Widget | None = None,
        after: int | str | Widget | None = None,
    ) -> AwaitMount:
        if not widgets:
            return AwaitMount([])
        if not self.is_mounted:
            self._pending_children.extend(widgets)
            return AwaitMount(list(widgets))
        index: int | None = None
        if before is not None or after is not None:
            kids = native.children(self._node)
            ref = before if before is not None else after
            if isinstance(ref, int):
                index = ref if ref >= 0 else len(kids) + ref
            else:
                target = ref if isinstance(ref, Widget) else self.query_one(ref)
                if target._node not in kids:
                    raise MountError(f"{target!r} is not a child of {self!r}")
                index = kids.index(target._node)
            if after is not None:
                index += 1
        native.mount(self._node, list(widgets), index)
        return AwaitMount(list(widgets))

    def mount_all(self, widgets: Iterable[Widget], **kwargs: Any) -> AwaitMount:
        return self.mount(*widgets, **kwargs)

    def mount_compose(self, compose_result: Iterable[Widget], **kwargs: Any) -> AwaitMount:
        return self.mount(*compose_result, **kwargs)

    def remove(self) -> AwaitComplete:
        if self.is_mounted:
            native.remove(self._node)
        return AwaitComplete()

    def remove_children(self, selector: str | type[Widget] | Iterable[Widget] = "*") -> AwaitComplete:
        if not self.is_mounted:
            self._pending_children.clear()
            return AwaitComplete()
        if selector == "*":
            native.remove_children(self._node)
        else:
            if isinstance(selector, (str, type)):
                targets = self.query_children(selector)
            else:
                targets = list(selector)
            for child in targets:
                child.remove()
        return AwaitComplete()

    def recompose(self) -> AwaitComplete:
        if not self.is_mounted:
            return AwaitComplete()
        native.remove_children(self._node)
        native.mount(self._node, compose(self), None)
        return AwaitComplete()

    def refresh(self: WidgetType, *regions: Any, repaint: bool = True, layout: bool = False, recompose: bool = False) -> WidgetType:
        if not self.is_mounted:
            return self
        if recompose:
            self.recompose()
        else:
            native.refresh()
        return self

    # -- messages, timers, workers ---------------------------------------------------------------

    def post_message(self, message: Message) -> bool:
        if not self.is_mounted:
            return False
        if message._sender is None:
            message._sender = self
        native.post_message(self._node, self._node, message, type(message).bubble)
        return True

    def run_action(self, action: str) -> AwaitComplete:
        if self.is_mounted:
            native.run_action(self._node, action)
        return AwaitComplete()

    def set_timer(self, delay: float, callback: Callable[..., Any] | None = None, *, name: str | None = None, pause: bool = False) -> Timer:
        return Timer(self, delay, callback, repeat=False, name=name, pause=pause)

    def set_interval(
        self,
        interval: float,
        callback: Callable[..., Any] | None = None,
        *,
        name: str | None = None,
        repeat: int = 0,
        pause: bool = False,
    ) -> Timer:
        return Timer(self, interval, callback, repeat=True, name=name, pause=pause, count=repeat)

    def call_later(self, callback: Callable[..., Any], *args: Any, **kwargs: Any) -> bool:
        if not self.is_mounted:
            return False
        Timer(self, 0, lambda: callback(*args, **kwargs), repeat=False)
        return True

    call_after_refresh = call_later
    call_next = call_later

    def run_worker(self, work: Any, name: str | None = "", group: str = "default", description: str = "", exit_on_error: bool = True, start: bool = True, exclusive: bool = False, thread: bool = False) -> Worker[Any]:
        from .worker import run_worker

        return run_worker(self, work, name=name, group=group, description=description, exit_on_error=exit_on_error, start=start, exclusive=exclusive, thread=thread)

    @property
    def workers(self) -> Any:
        from .worker import WorkerManager

        return WorkerManager.for_node(self)

    def notify(self, message: str, *, title: str = "", severity: str = "information", timeout: float | None = None, markup: bool = True) -> None:
        self.app.notify(message, title=title, severity=severity, timeout=timeout, markup=markup)

    def bell(self) -> None:
        native.bell()

    def animate(
        self,
        attribute: str,
        value: Any,
        *,
        final_value: Any = ...,
        duration: float | None = None,
        speed: float | None = None,
        delay: float = 0.0,
        easing: Any = "in_out_cubic",
        on_complete: Callable[[], Any] | None = None,
        level: str = "full",
    ) -> AwaitComplete:
        """Animate a numeric attribute (e.g. a reactive) to ``value``."""
        _context_animator().animate(
            self,
            attribute,
            value,
            final_value=final_value,
            duration=duration,
            speed=speed,
            delay=delay,
            easing=easing,
            on_complete=on_complete,
        )
        return AwaitComplete()

    def watch(self, obj: Any, attribute_name: str, callback: Callable[..., Any], init: bool = True) -> None:
        from .reactive import watch

        watch(obj, attribute_name, callback, init=init, watcher=self)

    def log(self, *args: Any, **kwargs: Any) -> None:
        """Devtools logging isn't supported; this is a no-op."""

    def prevent(self, *message_types: type[Message]) -> _NullContext:
        return _NullContext()

    def batch_update(self) -> _NullContext:
        return _NullContext()

    def capture_mouse(self, capture: bool = True) -> None:
        """Mouse capture is handled automatically while dragging."""

    def release_mouse(self) -> None:
        pass

    def action_focus(self) -> None:
        self.focus()


class _NullContext:
    def __enter__(self) -> None:
        return None

    def __exit__(self, *exc: Any) -> None:
        return None


class Timer:
    """A timer created by ``set_timer`` / ``set_interval``."""

    def __init__(
        self,
        owner: Widget,
        interval: float,
        callback: Callable[..., Any] | None,
        *,
        repeat: bool,
        name: str | None = None,
        pause: bool = False,
        count: int = 0,
    ) -> None:
        self._owner = owner
        self._interval = interval
        self._callback = callback
        self._repeat = repeat
        self._count = count
        self._fired = 0
        self.name = name
        self._id: int | None = None
        if not pause:
            self._start()

    def _fire(self) -> None:
        self._fired += 1
        if self._count and self._fired >= self._count:
            self.stop()
        elif not self._repeat:
            self._id = None
        if self._callback is not None:
            result = _invoke(self._callback)
            del result

    def _start(self) -> None:
        if self._id is not None:
            return
        node = self._owner._node if self._owner.is_mounted else None
        if node is None:
            node = native.root()
        self._id = native.set_timer(node, self._interval, self._fire, self._repeat)

    def stop(self) -> None:
        if self._id is not None:
            try:
                native.cancel_timer(self._id)
            except RuntimeError:
                pass
            self._id = None

    def pause(self) -> None:
        self.stop()

    def resume(self) -> None:
        self._start()

    def reset(self) -> None:
        self.stop()
        self._start()


def _context_animator() -> Any:
    app = _context.active_app()
    animator = app.__dict__.get("_animator")
    if animator is None:
        from ._animator import Animator

        animator = app.__dict__["_animator"] = Animator()
    return animator


def _flow_bits(message: Message) -> int:
    return (1 if message._stop_propagation else 0) | (2 if message._no_default_action else 0)


def _selectors_match(message: Message, selectors: dict[str, str]) -> bool:
    for attribute, selector in selectors.items():
        node = getattr(message, attribute, None)
        if not isinstance(node, Widget) or not node.match(selector):
            return False
    return True


def _message_subclasses(cls: type) -> list[type]:
    result = [cls]
    for sub in cls.__subclasses__():
        result.extend(_message_subclasses(sub))
    return result


_NATIVE_CLASSES["Widget"] = Widget

"""Reactive attributes (Textual's ``textual.reactive``), implemented in pure Python.

Setting a reactive validates (``validate_<name>``), stores, calls watchers (``watch_<name>``),
re-runs ``compute_<name>`` methods and refreshes the widget (repaint / layout / recompose).
"""

from __future__ import annotations

import inspect
from functools import lru_cache
from typing import Any, Callable, Generic, TypeVar

ReactiveType = TypeVar("ReactiveType")


class ReactiveError(Exception):
    pass


class TooManyComputesError(ReactiveError):
    pass


class Initialize(Generic[ReactiveType]):
    """Initialize a reactive from a method of the object: ``reactive(Initialize(get_default))``."""

    def __init__(self, callback: Callable[[Any], ReactiveType]) -> None:
        self.callback = callback

    def __call__(self, obj: Any) -> ReactiveType:
        return self.callback(obj)


class _Mutated:
    def __init__(self, value: Any) -> None:
        self.value = value


@lru_cache(maxsize=4096)
def _param_count_cached(func: Any) -> int:
    return _param_count(func)


def _param_count(func: Callable[..., Any]) -> int:
    try:
        sig = inspect.signature(func)
    except (TypeError, ValueError):
        return 0
    return sum(
        1
        for p in sig.parameters.values()
        if p.kind in (p.POSITIONAL_ONLY, p.POSITIONAL_OR_KEYWORD) and p.default is p.empty
    ) or (1 if any(p.kind == p.VAR_POSITIONAL for p in sig.parameters.values()) else 0)


def count_parameters(func: Callable[..., Any]) -> int:
    """Number of required positional parameters (bound methods exclude ``self``)."""
    target = getattr(func, "__func__", None)
    if target is not None:
        try:
            return max(0, _param_count_cached(target) - 1)
        except TypeError:
            return _param_count(func)
    try:
        return _param_count_cached(func)
    except TypeError:
        return _param_count(func)


def invoke_watcher(watcher_object: Any, watch_function: Callable[..., Any], old_value: Any, value: Any) -> None:
    count = count_parameters(watch_function)
    if count >= 2:
        result = watch_function(old_value, value)
    elif count == 1:
        result = watch_function(value)
    else:
        result = watch_function()
    if inspect.isawaitable(result):
        from .widget import _run_awaitable

        _run_awaitable(watcher_object, result)


class Reactive(Generic[ReactiveType]):
    """A reactive descriptor."""

    _reactives_registry: dict[type, dict[str, "Reactive[Any]"]] = {}

    def __init__(
        self,
        default: ReactiveType | Callable[[], ReactiveType] | Initialize[ReactiveType],
        *,
        layout: bool = False,
        repaint: bool = True,
        init: bool = False,
        always_update: bool = False,
        compute: bool = True,
        recompose: bool = False,
        bindings: bool = False,
        toggle_class: str | None = None,
    ) -> None:
        self._default = default
        self._layout = layout
        self._repaint = repaint
        self._init = init
        self._always_update = always_update
        self._run_compute = compute
        self._recompose = recompose
        self._bindings = bindings
        self._toggle_class = toggle_class
        self.name = ""
        self.internal_name = ""
        self.compute_name = ""

    def __set_name__(self, owner: type, name: str) -> None:
        self.name = name
        self.internal_name = f"_reactive_{name}"
        private_compute = f"_compute_{name}"
        self.compute_name = private_compute if hasattr(owner, private_compute) else f"compute_{name}"
        if hasattr(owner, self.compute_name):
            computes = owner.__dict__.get("_computes_")
            if computes is None:
                computes = list(getattr(owner, "_computes_", []))
                setattr(owner, "_computes_", computes)
            if name not in computes:
                computes.append(name)

    # -- initialization -------------------------------------------------------------------

    def _initialize_reactive(self, obj: Any, name: str) -> None:
        if self.internal_name in obj.__dict__:
            return
        compute_method = getattr(obj, self.compute_name, None)
        if compute_method is not None and self._init:
            default = compute_method()
        else:
            d = self._default
            if isinstance(d, Initialize):
                default = d(obj)
            elif callable(d):
                default = d()
            else:
                default = d
        obj.__dict__[self.internal_name] = default
        if self._toggle_class is not None:
            obj.set_class(bool(default), *self._toggle_class.split())
        if self._init:
            self._check_watchers(obj, name, default)

    @classmethod
    def _initialize_object(cls, obj: Any) -> None:
        for name, reactive in _reactives_of(type(obj)).items():
            reactive._initialize_reactive(obj, name)

    # -- descriptor protocol --------------------------------------------------------------

    def __get__(self, obj: Any, obj_type: type | None = None) -> Any:
        if obj is None:
            return self
        internal = self.internal_name
        if internal not in obj.__dict__:
            self._initialize_reactive(obj, self.name)
        compute = getattr(obj, self.compute_name, None)
        if compute is not None:
            old_value = obj.__dict__[internal]
            value = compute()
            obj.__dict__[internal] = value
            if old_value != value:
                self._check_watchers(obj, self.name, old_value)
            return value
        return obj.__dict__[internal]

    def __set__(self, obj: Any, value: Any) -> None:
        self._set(obj, value)

    def _set(self, obj: Any, value: Any, always: bool = False) -> None:
        if isinstance(value, _Mutated):
            value = value.value
            always = True
        self._initialize_reactive(obj, self.name)
        if hasattr(obj, self.compute_name):
            raise AttributeError(
                f"Can't set {obj}.{self.name!r}; reactive attributes with a compute method are read-only"
            )
        name = self.name
        current_value = obj.__dict__[self.internal_name]
        private_validate = getattr(obj, f"_validate_{name}", None)
        if callable(private_validate):
            value = private_validate(value)
        public_validate = getattr(obj, f"validate_{name}", None)
        if callable(public_validate):
            value = public_validate(value)
        if self._toggle_class is not None:
            obj.set_class(bool(value), *self._toggle_class.split())
        if always or self._always_update or current_value != value:
            obj.__dict__[self.internal_name] = value
            self._check_watchers(obj, name, current_value)
            if self._run_compute:
                self._compute(obj)
            if self._layout or self._repaint or self._recompose:
                refresh = getattr(obj, "refresh", None)
                if refresh is not None:
                    refresh(repaint=self._repaint, layout=self._layout, recompose=self._recompose)

    @classmethod
    def _check_watchers(cls, obj: Any, name: str, old_value: Any) -> None:
        value = obj.__dict__.get(f"_reactive_{name}")
        private_watch = getattr(obj, f"_watch_{name}", None)
        if callable(private_watch):
            invoke_watcher(obj, private_watch, old_value, value)
        public_watch = getattr(obj, f"watch_{name}", None)
        if callable(public_watch):
            invoke_watcher(obj, public_watch, old_value, value)
        watchers = obj.__dict__.get("_watchers_", {}).get(name, [])
        for watcher_obj, callback in list(watchers):
            invoke_watcher(watcher_obj, callback, old_value, value)

    @classmethod
    def _compute(cls, obj: Any) -> None:
        for name in getattr(type(obj), "_computes_", []):
            reactive = _reactives_of(type(obj)).get(name)
            if reactive is None:
                continue
            compute = getattr(obj, reactive.compute_name, None)
            if compute is None:
                continue
            old_value = obj.__dict__.get(reactive.internal_name)
            value = compute()
            obj.__dict__[reactive.internal_name] = value
            if old_value != value:
                cls._check_watchers(obj, name, old_value)


class reactive(Reactive[ReactiveType]):
    """A reactive attribute that refreshes the widget when changed (``init=True`` by default:
    watchers are called with the default on first access / mount)."""

    def __init__(
        self,
        default: ReactiveType | Callable[[], ReactiveType] | Initialize[ReactiveType],
        *,
        layout: bool = False,
        repaint: bool = True,
        init: bool = True,
        always_update: bool = False,
        recompose: bool = False,
        bindings: bool = False,
        toggle_class: str | None = None,
    ) -> None:
        super().__init__(
            default,
            layout=layout,
            repaint=repaint,
            init=init,
            always_update=always_update,
            recompose=recompose,
            bindings=bindings,
            toggle_class=toggle_class,
        )


class var(Reactive[ReactiveType]):
    """A reactive attribute that doesn't refresh the widget."""

    def __init__(
        self,
        default: ReactiveType | Callable[[], ReactiveType] | Initialize[ReactiveType],
        init: bool = True,
        always_update: bool = False,
        bindings: bool = False,
        toggle_class: str | None = None,
    ) -> None:
        super().__init__(
            default,
            layout=False,
            repaint=False,
            init=init,
            always_update=always_update,
            bindings=bindings,
            toggle_class=toggle_class,
        )


def _reactives_of(cls: type) -> dict[str, Reactive[Any]]:
    cached = cls.__dict__.get("_reactives_cache_")
    if cached is not None:
        return cached
    found: dict[str, Reactive[Any]] = {}
    for klass in reversed(cls.__mro__):
        for name, value in klass.__dict__.items():
            if isinstance(value, Reactive):
                found[name] = value
    setattr(cls, "_reactives_cache_", found)
    return found


def watch(obj: Any, attribute_name: str, callback: Callable[..., Any], init: bool = True, watcher: Any = None) -> None:
    """Call ``callback`` whenever ``obj.<attribute_name>`` changes."""
    watchers = obj.__dict__.setdefault("_watchers_", {})
    watchers.setdefault(attribute_name, []).append((watcher if watcher is not None else obj, callback))
    if init:
        value = getattr(obj, attribute_name)
        invoke_watcher(obj, callback, value, value)

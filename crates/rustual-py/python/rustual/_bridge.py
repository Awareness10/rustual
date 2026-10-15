"""Thread-safe access to the native module.

``native`` exposes every function of :mod:`rustual._native`. A call made from a thread other
than the app's thread (a worker, an asyncio task on the background loop) is transparently
re-run on the app thread via :func:`call_from_thread`, so widget APIs can be used from workers
without crashing or racing the event loop.

The GIL is never held while waiting: the waiting thread blocks on a ``threading.Event``
(which releases the GIL) and the native event loop runs with the GIL released.
"""

from __future__ import annotations

import threading
from typing import Any, Callable, TypeVar

from . import _native

T = TypeVar("T")

WrongThreadError = _native.WrongThreadError
NoActiveAppError = _native.NoActiveAppError


def call_from_thread(callback: Callable[..., T], *args: Any, **kwargs: Any) -> T:
    """Run ``callback`` on the app thread and return its result (re-raising its exception).

    Called on the app thread itself, the callback simply runs.
    """
    if _native.on_app_thread():
        return callback(*args, **kwargs)
    done = threading.Event()
    box: dict[str, Any] = {}

    def run() -> None:
        try:
            box["result"] = callback(*args, **kwargs)
        except BaseException as error:  # noqa: BLE001 - re-raised in the caller's thread
            box["error"] = error
        finally:
            done.set()

    if not _native.post_call(run):
        raise NoActiveAppError("the app is not running")
    while not done.wait(0.05):
        if not _native.app_running():
            raise NoActiveAppError("the app exited before the call completed")
    if "error" in box:
        raise box["error"]
    return box["result"]


class _Native:
    """Proxy for the native module that marshals calls from foreign threads."""

    def __getattr__(self, name: str) -> Any:
        fn = getattr(_native, name)
        if not callable(fn) or isinstance(fn, type):
            return fn

        def call(*args: Any, **kwargs: Any) -> Any:
            try:
                return fn(*args, **kwargs)
            except WrongThreadError:
                return call_from_thread(fn, *args, **kwargs)

        call.__name__ = name
        setattr(self, name, call)  # cache
        return call


native: Any = _Native()

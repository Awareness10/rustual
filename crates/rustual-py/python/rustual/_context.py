"""Process-wide state: the active app and the asyncio integration.

Async handlers, watchers and async workers run as tasks on an asyncio loop that lives on the
app thread, so they may touch widgets directly (as in Textual):

- ``App.run()``: the loop is owned by us and *pumped* by the native event loop (each pump runs
  ready callbacks without blocking; while tasks are pending, a 10 ms native timer keeps
  pumping so ``await asyncio.sleep()`` and I/O make progress).
- ``App.run_test()``: the test's own running loop is used; tasks start eagerly and the
  ``Pilot`` yields to the loop while waiting.
"""

from __future__ import annotations

import asyncio
import sys
from typing import TYPE_CHECKING, Any, Awaitable, Coroutine

if TYPE_CHECKING:
    from .app import App

_active_app: App | None = None


def active_app() -> App:
    if _active_app is None:
        from .css.query import NoMatches  # noqa: F401  (import cycle guard)

        raise RuntimeError("no active app")
    return _active_app


def set_active_app(app: App | None) -> None:
    global _active_app
    _active_app = app


class AsyncRunner:
    """Runs coroutines for the active app on the app thread."""

    PUMP_INTERVAL = 0.01

    def __init__(self, loop: asyncio.AbstractEventLoop | None = None) -> None:
        self.owned = loop is None
        self.loop = loop if loop is not None else asyncio.new_event_loop()
        self.tasks: set[asyncio.Task[Any]] = set()
        self._pump_timer: Any = None

    def run(self, awaitable: Awaitable[Any]) -> asyncio.Task[Any]:
        coro = awaitable if asyncio.iscoroutine(awaitable) else _wrap(awaitable)
        loop = self.loop
        if loop.is_running() and sys.version_info >= (3, 12):
            task = asyncio.Task(coro, loop=loop, eager_start=True)
        else:
            task = loop.create_task(coro)
        if not task.done():
            self.tasks.add(task)
            task.add_done_callback(self._done)
        else:
            self._done(task)
        if self.owned and not loop.is_running():
            self.pump()
        return task

    def _done(self, task: asyncio.Task[Any]) -> None:
        self.tasks.discard(task)
        if task.cancelled():
            return
        error = task.exception()
        if error is not None:
            from ._bridge import _native

            _native.report_error(error)

    def pump(self) -> None:
        """Run ready asyncio callbacks (never blocks). Only for an owned loop."""
        loop = self.loop
        if not self.owned or loop.is_running() or loop.is_closed():
            return
        for _ in range(100):
            loop.call_soon(loop.stop)
            loop.run_forever()
            if not getattr(loop, "_ready", None):
                break
        self._schedule_pump()

    def _schedule_pump(self) -> None:
        from ._bridge import _native

        if self.tasks and self._pump_timer is None and _native.app_running():
            try:
                root = _native.root()
                self._pump_timer = _native.set_timer(root, self.PUMP_INTERVAL, self.pump, True)
            except RuntimeError:
                self._pump_timer = None
        elif not self.tasks and self._pump_timer is not None:
            try:
                _native.cancel_timer(self._pump_timer)
            except RuntimeError:
                pass
            self._pump_timer = None

    def pending(self) -> bool:
        return bool(self.tasks)

    def close(self) -> None:
        for task in list(self.tasks):
            task.cancel()
        if self.owned and not self.loop.is_closed():
            if self.tasks:
                try:
                    self.loop.run_until_complete(asyncio.gather(*self.tasks, return_exceptions=True))
                except RuntimeError:
                    pass
            self.loop.close()
        self.tasks.clear()
        self._pump_timer = None


async def _wrap(awaitable: Awaitable[Any]) -> Any:
    return await awaitable


_runner: AsyncRunner | None = None


def runner() -> AsyncRunner:
    global _runner
    if _runner is None:
        _runner = AsyncRunner()
    return _runner


def set_runner(r: AsyncRunner | None) -> None:
    global _runner
    _runner = r


def run_coroutine(coro: Coroutine[Any, Any, Any] | Awaitable[Any]) -> asyncio.Task[Any]:
    return runner().run(coro)

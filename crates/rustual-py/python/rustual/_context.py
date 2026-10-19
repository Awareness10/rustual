"""Process-wide state: the active app and the asyncio integration.

Async handlers, watchers and async workers run as tasks on an asyncio loop that lives on the
app thread, so they may touch widgets directly (as in Textual):

- ``App.run()``: the loop is owned by us and *pumped* by the native event loop: each pump runs
  ready callbacks without blocking. The app wakes for the loop's next timer exactly when it is
  due (``await asyncio.sleep(1 / 60)`` ticks at 60 Hz), and a watcher thread blocks on the
  loop's selector and wakes the app as soon as I/O is ready (sockets, pipes,
  ``call_soon_threadsafe``). Where there is no pollable selector (Windows' proactor loop, and
  macOS for now), pending tasks are polled every 10 ms instead.
- ``App.run_test()``: the test's own running loop is used; tasks start eagerly and the
  ``Pilot`` yields to the loop while waiting.
"""

from __future__ import annotations

import asyncio
import os
import select
import sys
import threading
from collections.abc import Awaitable, Coroutine
from typing import TYPE_CHECKING, Any

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

    # Fallback polling interval for pending tasks when I/O readiness can't be watched.
    PUMP_INTERVAL = 0.01

    def __init__(self, loop: asyncio.AbstractEventLoop | None = None) -> None:
        self.owned = loop is None
        self.loop = loop if loop is not None else asyncio.new_event_loop()
        self.tasks: set[asyncio.Task[Any]] = set()
        # The native one-shot timer that pumps next, and when (in `loop.time()`) it fires.
        self._timer: int | None = None
        self._timer_due = 0.0
        self._watcher: _SelectorWatcher | None = None
        self._watch_failed = False

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
            self._start_watcher()
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
        try:
            if not self.owned or loop.is_running() or loop.is_closed():
                return
            for _ in range(100):
                loop.call_soon(loop.stop)
                loop.run_forever()
                if not getattr(loop, "_ready", None):
                    break
            self._schedule_pump()
        finally:
            # Let the watcher poll again (I/O still pending is level-triggered, so nothing is lost
            # when this pump returned early).
            if self._watcher is not None:
                self._watcher.pumped()

    def _next_wakeup(self) -> float | None:
        """Seconds until the loop needs to run again (``None``: only when I/O is ready)."""
        loop = self.loop
        delay: float | None = None
        if getattr(loop, "_ready", None):
            delay = 0.0
        scheduled = getattr(loop, "_scheduled", None)
        if scheduled:
            delay = max(0.0, scheduled[0].when() - loop.time()) if delay is None else delay
        elif scheduled is None and self.tasks:
            delay = self.PUMP_INTERVAL  # not a BaseEventLoop: poll
        if self.tasks and self._watcher is None:
            # Nothing tells us when I/O is ready: poll while tasks are pending.
            delay = self.PUMP_INTERVAL if delay is None else min(delay, self.PUMP_INTERVAL)
        return delay

    def _schedule_pump(self) -> None:
        from ._bridge import _native

        delay = self._next_wakeup()
        if delay is None or not _native.app_running():
            self._cancel_timer()
            return
        due = self.loop.time() + delay
        if self._timer is not None and self._timer_due <= due + 0.0005:
            return  # the pending pump comes soon enough
        self._cancel_timer()
        try:
            self._timer = _native.set_timer(_native.root(), delay, self._on_timer, False)
            self._timer_due = due
        except RuntimeError:
            self._timer = None

    def _on_timer(self) -> None:
        self._timer = None
        self.pump()

    def _cancel_timer(self) -> None:
        if self._timer is None:
            return
        from ._bridge import _native

        try:
            _native.cancel_timer(self._timer)
        except RuntimeError:
            pass
        self._timer = None

    def _start_watcher(self) -> None:
        if self._watcher is not None or self._watch_failed:
            return
        try:
            self._watcher = _SelectorWatcher(self)
        except (OSError, ValueError, AttributeError):
            self._watch_failed = True

    def pending(self) -> bool:
        return bool(self.tasks)

    def close(self) -> None:
        if self._watcher is not None:
            self._watcher.stop()
            self._watcher = None
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
        self._timer = None


class _SelectorWatcher:
    """Wakes the app when the owned loop has I/O ready.

    A daemon thread polls the loop's epoll descriptor (readable whenever a registered socket,
    pipe or the loop's self-pipe is ready) and posts a pump to the app thread; it waits for that
    pump before polling again, so it never spins. Linux only for now (``OSError`` elsewhere).
    """

    def __init__(self, runner: AsyncRunner) -> None:
        if not sys.platform.startswith("linux") or not hasattr(select, "poll"):
            raise OSError("no pollable selector on this platform")
        selector = runner.loop._selector  # type: ignore[attr-defined]
        fd = selector.fileno()
        if fd < 0:
            raise OSError("the loop's selector has no file descriptor")
        self._runner = runner
        self._fd = fd
        self._stop_r, self._stop_w = os.pipe()
        self._pumped = threading.Event()
        self._pumped.set()
        self._stopping = False
        self._thread = threading.Thread(target=self._run, name="rustual-asyncio", daemon=True)
        self._thread.start()

    def pumped(self) -> None:
        self._pumped.set()

    def stop(self) -> None:
        self._stopping = True
        self._pumped.set()
        try:
            os.write(self._stop_w, b"x")
        except OSError:
            pass
        self._thread.join(1.0)
        for fd in (self._stop_r, self._stop_w):
            try:
                os.close(fd)
            except OSError:
                pass

    def _run(self) -> None:
        from ._bridge import _native

        poller = select.poll()
        poller.register(self._fd, select.POLLIN)
        poller.register(self._stop_r, select.POLLIN)
        bad = select.POLLERR | select.POLLHUP | select.POLLNVAL
        while True:
            self._pumped.wait()
            if self._stopping:
                return
            try:
                events = poller.poll()
            except OSError:
                events = [(self._fd, select.POLLERR)]
            if self._stopping or any(fd == self._stop_r for fd, _ in events):
                return
            if any(mask & bad for _, mask in events):
                # The descriptor can't be watched after all: fall back to polling.
                runner = self._runner
                runner._watcher = None
                runner._watch_failed = True
                _native.post_call(runner.pump)
                return
            self._pumped.clear()
            if not _native.post_call(self._runner.pump):
                return


async def _wrap(awaitable: Awaitable[Any]) -> Any:
    return await awaitable


_runner: AsyncRunner | None = None

# Native calls attempted where the DOM isn't accessible (e.g. `self.title = ...` in `compose()`),
# applied at the next handler call.
_deferred: list[Any] = []


# Message types suppressed by `Widget.prevent(...)`, innermost scope last.
prevented: list[tuple[type, ...]] = []


def is_prevented(message: Any) -> bool:
    return any(isinstance(message, types) for types in prevented if types)


def defer(call: Any) -> None:
    _deferred.append(call)


def flush_deferred() -> None:
    if not _deferred:
        return
    pending = list(_deferred)
    _deferred.clear()
    for call in pending:
        try:
            call()
        except RuntimeError:
            _deferred.append(call)


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

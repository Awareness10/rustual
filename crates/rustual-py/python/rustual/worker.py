"""Workers (Textual's ``textual.worker``): run work in an async task or a thread.

Async workers run on the app thread's asyncio loop and may update widgets directly. Thread
workers run in a daemon thread; widget APIs called from them are transparently marshalled
to the app thread (``app.call_from_thread`` is also available, as in Textual).
"""

from __future__ import annotations

import asyncio
import contextvars
import inspect
import threading
from enum import Enum
from functools import partial, wraps
from typing import TYPE_CHECKING, Any, Callable, Generic, TypeVar

from . import _context
from ._bridge import _native, native
from .message import Message

if TYPE_CHECKING:
    from .widget import Widget

ResultType = TypeVar("ResultType")

_current_worker: contextvars.ContextVar[Worker[Any] | None] = contextvars.ContextVar("current_worker", default=None)
_thread_worker = threading.local()


class NoActiveWorker(Exception):
    pass


class WorkerError(Exception):
    pass


class WorkerFailed(Exception):
    def __init__(self, error: BaseException) -> None:
        super().__init__(f"Worker raised exception: {error!r}")
        self.error = error


class DeadlockError(Exception):
    pass


class WorkerCancelled(Exception):
    pass


class WorkerState(Enum):
    PENDING = 1
    RUNNING = 2
    CANCELLED = 3
    ERROR = 4
    SUCCESS = 5


def get_current_worker() -> Worker[Any]:
    worker = getattr(_thread_worker, "worker", None) or _current_worker.get()
    if worker is None:
        raise NoActiveWorker("There is no active worker in this task or thread.")
    return worker


class Worker(Generic[ResultType]):
    """A unit of background work."""

    class StateChanged(Message, bubble=False, namespace="worker"):
        """Posted to the worker's node when its state changes (``on_worker_state_changed``)."""

        def __init__(self, worker: Worker[Any], state: WorkerState) -> None:
            super().__init__()
            self.worker = worker
            self.state = state

        def __repr__(self) -> str:
            return f"Worker.StateChanged({self.worker.name!r}, {self.state.name})"

    def __init__(
        self,
        node: Widget,
        work: Any,
        *,
        name: str = "",
        group: str = "default",
        description: str = "",
        exit_on_error: bool = True,
        thread: bool = False,
    ) -> None:
        self._node = node
        self._work = work
        self.name = name
        self.group = group
        self.description = description
        self.exit_on_error = exit_on_error
        self._thread_worker = thread
        self._state = WorkerState.PENDING
        self._result: ResultType | None = None
        self._error: BaseException | None = None
        self._cancelled = False
        self._task: asyncio.Task[Any] | None = None
        self._done = threading.Event()
        # The app that started the worker: errors are only reported to it, while it runs.
        self._app = _context._active_app

    def __repr__(self) -> str:
        return f"<Worker {self._state.name.lower()} name={self.name!r}>"

    @property
    def node(self) -> Widget:
        return self._node

    @property
    def state(self) -> WorkerState:
        return self._state

    @property
    def is_cancelled(self) -> bool:
        return self._cancelled

    @property
    def is_running(self) -> bool:
        return self._state == WorkerState.RUNNING

    @property
    def is_finished(self) -> bool:
        return self._state in (WorkerState.SUCCESS, WorkerState.ERROR, WorkerState.CANCELLED)

    @property
    def result(self) -> ResultType | None:
        return self._result

    @property
    def error(self) -> BaseException | None:
        return self._error

    def _set_state(self, state: WorkerState) -> None:
        self._state = state
        node = self._node
        try:
            # From a thread, `post_message` queues without waiting (and checks the node later).
            if not _native.on_app_thread() or node.is_mounted:
                node.post_message(Worker.StateChanged(self, state))
        except RuntimeError:
            pass
        if self.is_finished:
            self._done.set()
            WorkerManager._remove(self)

    def _app_running(self) -> bool:
        app = self._app
        return app is not None and _context._active_app is app and getattr(app, "_running", False)

    def _fail(self, error: BaseException) -> None:
        self._error = error
        self._set_state(WorkerState.ERROR)
        # A worker that outlives its app (e.g. a thread still running after exit) fails quietly
        # instead of reporting into whatever app runs next.
        if self.exit_on_error and self._app_running():
            _native.report_error(WorkerFailed(error)) if _native.on_app_thread() else _native.post_call(
                partial(_native.report_error, WorkerFailed(error))
            )

    def _call(self) -> Any:
        work = self._work
        if inspect.iscoroutine(work) or inspect.isawaitable(work):
            return work
        return work()

    async def _run_async(self) -> None:
        token = _current_worker.set(self)
        self._set_state(WorkerState.RUNNING)
        try:
            result = self._call()
            if inspect.isawaitable(result):
                result = await result
            self._result = result
            self._set_state(WorkerState.SUCCESS)
        except asyncio.CancelledError:
            self._cancelled = True
            self._set_state(WorkerState.CANCELLED)
        except Exception as error:  # noqa: BLE001
            self._fail(error)
        finally:
            _current_worker.reset(token)

    def _run_thread(self) -> None:
        _thread_worker.worker = self
        try:
            self._set_state(WorkerState.RUNNING)
            result = self._call()
            if inspect.isawaitable(result):
                result = asyncio.run(_await(result))
            self._result = result
            self._set_state(WorkerState.CANCELLED if self._cancelled else WorkerState.SUCCESS)
        except Exception as error:  # noqa: BLE001
            self._fail(error)
        finally:
            _thread_worker.worker = None

    def _start(self) -> None:
        if self._state != WorkerState.PENDING:
            return
        if self._thread_worker:
            threading.Thread(target=self._run_thread, name=f"worker-{self.name}", daemon=True).start()
        else:
            self._task = _context.run_coroutine(self._run_async())

    def start(self) -> None:
        self._start()

    def cancel(self) -> None:
        """Cancel the worker (async: the task is cancelled; thread: check ``is_cancelled``)."""
        self._cancelled = True
        if self._task is not None and not self._task.done():
            self._task.cancel()
        elif self._state == WorkerState.PENDING:
            self._set_state(WorkerState.CANCELLED)

    async def wait(self) -> ResultType:
        while not self._done.is_set():
            await asyncio.sleep(0.005)
        if self._state == WorkerState.ERROR and self._error is not None:
            raise WorkerFailed(self._error)
        if self._state == WorkerState.CANCELLED:
            raise WorkerCancelled("Worker was cancelled")
        return self._result  # type: ignore[return-value]


async def _await(awaitable: Any) -> Any:
    return await awaitable


class WorkerManager:
    """Tracks running workers (``app.workers`` / ``widget.workers``)."""

    _workers: list[Worker[Any]] = []

    def __init__(self, node: Widget | None = None) -> None:
        self._node = node

    @classmethod
    def for_node(cls, node: Widget) -> WorkerManager:
        from .app import App

        return cls(None if isinstance(node, App) else node)

    @classmethod
    def _add(cls, worker: Worker[Any]) -> None:
        cls._workers.append(worker)

    @classmethod
    def _remove(cls, worker: Worker[Any]) -> None:
        if worker in cls._workers:
            cls._workers.remove(worker)

    def _mine(self) -> list[Worker[Any]]:
        return [w for w in list(self._workers) if self._node is None or w.node is self._node]

    def __iter__(self) -> Any:
        return iter(self._mine())

    def __len__(self) -> int:
        return len(self._mine())

    def cancel_all(self) -> list[Worker[Any]]:
        workers = self._mine()
        for w in workers:
            w.cancel()
        return workers

    def cancel_group(self, node: Widget, group: str) -> list[Worker[Any]]:
        workers = [w for w in list(self._workers) if w.node is node and w.group == group]
        for w in workers:
            w.cancel()
        return workers

    @classmethod
    def _cancel_node(cls, node: Widget) -> None:
        for w in [w for w in cls._workers if w.node is node]:
            w.cancel()

    def cancel_node(self, node: Widget) -> list[Worker[Any]]:
        workers = [w for w in list(self._workers) if w.node is node]
        for w in workers:
            w.cancel()
        return workers

    async def wait_for_complete(self, workers: Any = None) -> None:
        for w in list(workers if workers is not None else self._mine()):
            try:
                await w.wait()
            except (WorkerFailed, WorkerCancelled):
                pass


def run_worker(
    node: Widget,
    work: Any,
    *,
    name: str | None = "",
    group: str = "default",
    description: str = "",
    exit_on_error: bool = True,
    start: bool = True,
    exclusive: bool = False,
    thread: bool = False,
) -> Worker[Any]:
    if exclusive:
        WorkerManager().cancel_group(node, group)
    if name is None or name == "":
        name = getattr(work, "__name__", "") or ""
    worker: Worker[Any] = Worker(
        node, work, name=name, group=group, description=description, exit_on_error=exit_on_error, thread=thread
    )
    WorkerManager._add(worker)
    if start:
        worker._start()
    return worker


def work(
    method: Callable[..., Any] | None = None,
    *,
    name: str = "",
    group: str = "default",
    exit_on_error: bool = True,
    exclusive: bool = False,
    description: str | None = None,
    thread: bool = False,
) -> Any:
    """Decorator: calling the method starts it as a worker and returns the ``Worker``."""

    def decorator(func: Callable[..., Any]) -> Callable[..., Worker[Any]]:
        if not thread and not inspect.iscoroutinefunction(func):
            raise WorkerDeclarationError(
                "Can not create a worker from a non-async function unless `thread=True` is set on the work decorator."
            )

        @wraps(func)
        def decorated(self: Widget, *args: Any, **kwargs: Any) -> Worker[Any]:
            call = partial(func, self, *args, **kwargs)
            return run_worker(
                self,
                call,
                name=name or func.__name__,
                group=group,
                description=description or func.__name__,
                exit_on_error=exit_on_error,
                exclusive=exclusive,
                thread=thread,
            )

        return decorated

    if method is None:
        return decorator
    return decorator(method)


class WorkerDeclarationError(Exception):
    pass

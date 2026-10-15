"""``Pilot``: drive an app in tests (Textual's ``textual.pilot``)."""

from __future__ import annotations

import asyncio
import time
from typing import TYPE_CHECKING, Any, Generic, TypeVar

from . import _context
from ._bridge import _native
from .geometry import Offset
from .widget import Widget, _selector

if TYPE_CHECKING:
    from .app import App

ReturnType = TypeVar("ReturnType")


class OutOfBounds(Exception):
    pass


class Pilot(Generic[ReturnType]):
    """Returned by ``App.run_test()``; every method waits until the app has settled."""

    def __init__(self, app: App[ReturnType], handle: Any) -> None:
        self._app = app
        self._handle = handle

    @property
    def app(self) -> App[ReturnType]:
        return self._app

    async def _settle(self, timeout: float = 5.0) -> None:
        """Process pending work until the app is idle and no asyncio callbacks are runnable.

        Tasks blocked on something external (a sleep, a dismissed screen) aren't waited for;
        use ``pause(delay)`` to let time pass, like in Textual.
        """
        deadline = time.monotonic() + timeout
        loop = asyncio.get_running_loop()
        while True:
            self._handle.pause()
            # Let tasks scheduled on the test loop (async handlers, workers) run.
            for _ in range(3):
                await asyncio.sleep(0)
            busy = self._handle.pump()
            runnable = bool(getattr(loop, "_ready", ()))
            if (not busy and not runnable) or time.monotonic() > deadline:
                break
        self._raise_errors()

    def _raise_errors(self) -> None:
        error = _native.take_error()
        if error is not None:
            raise error

    async def press(self, *keys: str) -> None:
        """Press keys (``"a"``, ``"enter"``, ``"ctrl+c"``)."""
        await self._settle()  # apply pending focus changes etc. first
        for key in keys:
            self._handle.press([key])
            await self._settle()

    async def type(self, text: str) -> None:
        await self._settle()
        self._handle.type_text(text)
        await self._settle()

    async def paste(self, text: str) -> None:
        self._handle.paste(text)
        await self._settle()

    def _target(self, widget: Widget | type[Widget] | str | None, offset: tuple[int, int]) -> tuple[int, int]:
        if widget is None:
            x, y = 0, 0
        else:
            target = widget if isinstance(widget, Widget) else self._app.query_one(_selector(widget))
            region = target.region
            x, y = region.x, region.y
        return x + offset[0], y + offset[1]

    async def click(
        self,
        widget: Widget | type[Widget] | str | None = None,
        offset: tuple[int, int] = (0, 0),
        shift: bool = False,
        meta: bool = False,
        control: bool = False,
        times: int = 1,
    ) -> bool:
        """Click a widget (at ``offset`` from its top-left corner) or the screen."""
        await self._settle()
        x, y = self._target(widget, offset)
        for _ in range(times):
            self._handle.click(None, (x, y))
            await self._settle()
        return True

    async def double_click(self, widget: Any = None, offset: tuple[int, int] = (0, 0), **kwargs: Any) -> bool:
        return await self.click(widget, offset, times=2)

    async def hover(self, widget: Widget | type[Widget] | str | None = None, offset: tuple[int, int] = (0, 0)) -> bool:
        """Move the mouse over a widget (at ``offset`` from its top-left corner)."""
        x, y = self._target(widget, offset)
        self._handle.hover_at(x, y)
        await self._settle()
        return True

    async def mouse_down(self, widget: Any = None, offset: tuple[int, int] = (0, 0), **kwargs: Any) -> bool:
        x, y = self._target(widget, offset)
        self._handle.mouse_down(x, y)
        await self._settle()
        return True

    async def mouse_up(self, widget: Any = None, offset: tuple[int, int] = (0, 0), **kwargs: Any) -> bool:
        x, y = self._target(widget, offset)
        self._handle.mouse_up(x, y)
        await self._settle()
        return True

    async def resize_terminal(self, width: int, height: int) -> None:
        self._handle.resize(width, height)
        await self._settle()

    async def pause(self, delay: float | None = None) -> None:
        """Wait for pending work (and ``delay`` seconds, letting timers fire)."""
        if delay:
            end = time.monotonic() + delay
            while time.monotonic() < end:
                step = min(0.01, max(0.0, end - time.monotonic()))
                self._handle.wait(step)
                await asyncio.sleep(0)
        await self._settle()

    async def wait_for_animation(self) -> None:
        await self._settle()

    async def wait_for_scheduled_animations(self) -> None:
        await self._settle()

    async def exit(self, result: ReturnType) -> None:
        self._app.exit(result)
        await self._settle()

    # -- inspection helpers (not in Textual) ------------------------------------------------

    def export_svg(self) -> str:
        """The current screen as an SVG image."""
        return self._handle.export_svg()

    def screen_text(self) -> str:
        """The current screen as plain text (one line per row)."""
        return self._handle.screen_text()

    @property
    def notifications(self) -> list[str]:
        return list(self._handle.notifications())

    @property
    def has_exited(self) -> bool:
        return bool(self._handle.has_exited())

    @property
    def offset(self) -> Offset:
        return Offset()

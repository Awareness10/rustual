"""Compare rustual with Python Textual on the same app code.

    uv run python bench/bench.py rustual
    /path/to/venv-with-textual/bin/python bench/bench.py textual

Each scenario runs headlessly via ``App.run_test()``; times are wall-clock medians.
"""

from __future__ import annotations

import asyncio
import importlib
import statistics
import sys
import time

impl = sys.argv[1] if len(sys.argv) > 1 else "rustual"
app_mod = importlib.import_module(f"{impl}.app")
widgets = importlib.import_module(f"{impl}.widgets")
containers = importlib.import_module(f"{impl}.containers")
reactive_mod = importlib.import_module(f"{impl}.reactive")

App = app_mod.App
Label, Static, Button = widgets.Label, widgets.Static, widgets.Button
VerticalScroll, Horizontal = containers.VerticalScroll, containers.Horizontal
reactive = reactive_mod.reactive

N_WIDGETS = 1000


class ManyApp(App):
    """Startup with many widgets."""

    def compose(self):
        with VerticalScroll():
            for i in range(N_WIDGETS):
                with Horizontal(classes="row"):
                    yield Label(f"row {i}", classes="name")
                    yield Static(f"[b]{i * 3}[/b] value", classes="value")

    CSS = """
    .row { height: 1; }
    .name { width: 20; }
    .value { width: 1fr; color: $accent; }
    """


class Counter(Static):
    count = reactive(0)

    def render(self):
        return f"Count: [b]{self.count}[/b]"


class KeysApp(App):
    """Key presses that update a reactive and re-render."""

    BINDINGS = [("x", "inc", "inc")]

    def compose(self):
        yield Counter()
        for i in range(200):
            yield Label(f"filler {i}")

    def action_inc(self):
        self.query_one(Counter).count += 1


def render(pilot) -> None:
    """Make sure a full frame is composed (Textual's headless driver renders on refresh)."""
    if impl == "rustual":
        pilot.screen_text()


async def scenario_startup() -> float:
    t = time.perf_counter()
    async with ManyApp().run_test(size=(100, 40)) as pilot:
        await pilot.pause()
        render(pilot)
    return time.perf_counter() - t


async def scenario_keys() -> float:
    async with KeysApp().run_test(size=(100, 40)) as pilot:
        await pilot.pause()
        t = time.perf_counter()
        for _ in range(100):
            await pilot.press("x")
            render(pilot)
        elapsed = time.perf_counter() - t
        assert pilot.app.query_one(Counter).count == 100
    return elapsed


async def scenario_scroll() -> float:
    async with ManyApp().run_test(size=(100, 40)) as pilot:
        await pilot.pause()
        scroller = pilot.app.query_one(VerticalScroll)
        t = time.perf_counter()
        for i in range(100):
            scroller.scroll_to(y=i * 5, animate=False)
            await pilot.pause()
            render(pilot)
        return time.perf_counter() - t


async def main() -> None:
    results = {}
    for name, fn in [("startup (1000 rows)", scenario_startup), ("100 key presses", scenario_keys), ("100 scroll steps", scenario_scroll)]:
        runs = [await fn() for _ in range(3)]
        results[name] = statistics.median(runs)
    for name, secs in results.items():
        print(f"{impl:11} {name:22} {secs * 1000:9.1f} ms")


asyncio.run(main())

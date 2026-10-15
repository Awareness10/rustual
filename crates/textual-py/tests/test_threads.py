"""GIL / thread-safety stress: concurrent workers touching widgets; exit while threads wait."""

import threading
import time

from textual_rs.app import App, ComposeResult
from textual_rs.reactive import reactive
from textual_rs.widget import Widget
from textual_rs.widgets import Label


class Count(Widget):
    DEFAULT_CSS = "Count { height: 1; }"

    n = reactive(0)

    def render(self) -> str:
        return f"n={self.n}"


async def test_many_threads_update_widgets_concurrently():
    N_THREADS, N_UPDATES = 8, 50

    class MyApp(App):
        BINDINGS = [("x", "bump", "")]

        def compose(self) -> ComposeResult:
            yield Count()
            for i in range(N_THREADS):
                yield Label("", id=f"l{i}")

        def on_mount(self) -> None:
            for i in range(N_THREADS):
                self.run_worker(lambda i=i: self.hammer(i), thread=True)

        def hammer(self, i: int) -> None:
            label = self.query_one(f"#l{i}", Label)
            for k in range(N_UPDATES):
                label.update(f"t{i}:{k}")
                # Read-modify-write runs on the app thread as two calls; use call_from_thread
                # to make the increment atomic.
                self.call_from_thread(self._inc)

        def _inc(self) -> None:
            self.query_one(Count).n += 1

        def action_bump(self) -> None:
            self.query_one(Count).n += 1000

    app = MyApp()
    async with app.run_test() as pilot:
        for _ in range(5):
            await pilot.press("x")
        deadline = time.monotonic() + 10
        while app.query_one(Count).n < N_THREADS * N_UPDATES + 5000 and time.monotonic() < deadline:
            await pilot.pause(0.01)
        assert app.query_one(Count).n == N_THREADS * N_UPDATES + 5000
        text = pilot.screen_text()
        for i in range(N_THREADS):
            assert f"t{i}:{N_UPDATES - 1}" in text


async def test_exit_while_threads_wait_does_not_deadlock():
    errors: list[BaseException] = []
    started = threading.Event()

    class MyApp(App):
        def compose(self) -> ComposeResult:
            yield Label("", id="l")

        def on_mount(self) -> None:
            threading.Thread(target=self.spin, daemon=True).start()

        def spin(self) -> None:
            started.set()
            try:
                while True:
                    self.query_one("#l", Label).update(str(time.monotonic()))
            except Exception as error:  # NoActiveAppError, NoMatches, WidgetError: any clean failure
                errors.append(error)

    app = MyApp()
    async with app.run_test() as pilot:
        started.wait(2)
        await pilot.pause(0.05)
    t0 = time.monotonic()
    while not errors and time.monotonic() - t0 < 3:
        time.sleep(0.01)
    assert errors, "the worker thread should fail cleanly once the app has exited"

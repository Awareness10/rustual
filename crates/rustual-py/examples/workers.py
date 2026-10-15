"""Workers: an async worker (asyncio.sleep) and a thread worker updating the UI."""

import asyncio
import time

from rustual import work
from rustual.app import App, ComposeResult
from rustual.widgets import Footer, Header, Label


class WorkersApp(App):
    BINDINGS = [("b", "boom", "Raise an error"), ("q", "quit", "Quit")]

    def compose(self) -> ComposeResult:
        yield Header()
        yield Label("async: waiting", id="async")
        yield Label("thread: waiting", id="thread")
        yield Footer()

    def on_mount(self) -> None:
        self.ticker()
        self.crunch()

    @work
    async def ticker(self) -> None:
        for i in range(1, 1000):
            await asyncio.sleep(0.1)
            self.query_one("#async", Label).update(f"async: tick {i}")

    @work(thread=True)
    def crunch(self) -> None:
        for i in range(1, 1000):
            time.sleep(0.15)
            self.query_one("#thread", Label).update(f"thread: step {i}")

    def action_boom(self) -> None:
        raise RuntimeError("boom from a handler")


if __name__ == "__main__":
    WorkersApp().run()

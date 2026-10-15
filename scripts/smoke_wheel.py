"""Smoke test for an installed rustual-py wheel: run a tiny app headlessly."""

import asyncio
import sys

from rustual.app import App, ComposeResult
from rustual.widgets import Button, Label


class Smoke(App[int]):
    def compose(self) -> ComposeResult:
        yield Label("hello", id="label")
        yield Button("OK", id="ok")

    def on_button_pressed(self) -> None:
        self.exit(42)


async def main() -> None:
    app = Smoke()
    async with app.run_test() as pilot:
        assert "hello" in pilot.screen_text()
        await pilot.click("#ok")
    assert app.return_value == 42, app.return_value
    print(f"rustual-py smoke test OK (Python {sys.version.split()[0]}, {sys.platform})")


asyncio.run(main())

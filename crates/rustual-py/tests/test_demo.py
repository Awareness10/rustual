"""The bundled examples (`rustual-py`, `python -m rustual`) and their launcher."""

import pytest

from rustual.cli import main
from rustual.demo import DEMOS
from rustual.demo.launcher import Launcher


@pytest.mark.parametrize("demo", DEMOS, ids=[d.name for d in DEMOS])
async def test_demo_runs(demo):
    app = demo.load()()
    async with app.run_test(size=(100, 30)) as pilot:
        await pilot.pause(0.1)
        assert pilot.screen_text().strip(), f"{demo.name} rendered nothing"


async def test_launcher_returns_the_chosen_demo():
    app = Launcher()
    async with app.run_test(size=(100, 30)) as pilot:
        assert "Stopwatch" in pilot.screen_text()
        await pilot.press("down", "enter")
    assert app.return_value == DEMOS[1].name


def test_cli_list(capsys):
    assert main(["--list"]) == 0
    out = capsys.readouterr().out
    assert all(d.name in out for d in DEMOS)

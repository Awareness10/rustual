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


async def test_tasks_demo(tmp_path):
    from rustual.demo.tasks import TasksApp

    path = tmp_path / "tasks.json"
    app = TasksApp(path)
    async with app.run_test(size=(110, 30)) as pilot:
        footer = pilot.screen_text().splitlines()[-1]
        assert footer.startswith(" ^q Quit  n New  e Edit  space Done  del Delete  / Filter  ^s Save ")
        table = app.query_one("#tasks")
        await pilot.press("n", "enter")
        assert "The title can't be empty." in pilot.screen_text()
        await pilot.press(*"Buy milk", "enter")
        assert table.row_count == 8
        await pilot.press("slash", *"milk")
        assert table.row_count == 1
        # The focused filter input takes printable keys, so their bindings leave the footer.
        assert " n New " not in pilot.screen_text().splitlines()[-1]
        await pilot.press("escape", "delete", "y", "ctrl+s")
        assert table.row_count == 7
    assert len(__import__("json").loads(path.read_text())) == 7

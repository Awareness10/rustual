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
        assert "Task manager" in pilot.screen_text()
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


async def test_monitor_demo():
    from rustual.demo.monitor import MonitorApp

    app = MonitorApp()
    async with app.run_test(size=(110, 36)) as pilot:
        footer = pilot.screen_text().splitlines()[-1]
        assert footer.startswith(" ^q Quit  c CPU  m Memory  n Name  d Details  k Kill  space Pause ")
        table = app.query_one("#procs")
        names = lambda: [table.get_row_at(r)[1] for r in range(table.row_count)]  # noqa: E731
        await pilot.press("n")
        assert names() == sorted(names())
        await pilot.press("space")
        frozen = [table.get_row_at(r) for r in range(table.row_count)]
        await pilot.pause(1.3)
        assert [table.get_row_at(r) for r in range(table.row_count)] == frozen
        await pilot.press("d")
        assert "CPU over the last minute" in pilot.screen_text()
        await pilot.press("escape", "k", "y")
        assert table.row_count == 39
    narrow = MonitorApp()
    async with narrow.run_test(size=(70, 30)):
        assert narrow.screen.has_class("-narrow")
        assert not narrow.query_one("#cards").display or narrow.query_one("#cards").region.height == 0


async def test_chat_demo():
    from rustual.demo.chat import ChatApp, reply, tokens
    from rustual.widgets import Markdown

    text = reply("a table please")
    assert "".join(tokens(text)) == text
    app = ChatApp()
    async with app.run_test(size=(100, 30)) as pilot:
        footer = pilot.screen_text().splitlines()[-1]
        assert footer.startswith(" ^q Quit  esc Stop  ^l Clear  ^n New chat ")
        await pilot.press(*"python", "enter")
        await pilot.pause(3)
        answers = list(app.query(".assistant").results(Markdown))
        assert "def fib(n: int)" in answers[1].source
        assert "3 messages" in app.sub_title
        await pilot.press(*"plan", "enter")
        await pilot.pause(0.05)
        await pilot.press("escape")
        stopped = list(app.query(".assistant").results(Markdown))[2]
        await pilot.pause(0.2)
        assert stopped.source.endswith("*(stopped)*") and "changelog" not in stopped.source
        await pilot.press("up")
        assert app.query_one("#prompt").value == "plan"
        await pilot.press("up")
        assert app.query_one("#prompt").value == "python"
        await pilot.press("ctrl+l")
        assert not list(app.query(".assistant"))

"""Textual's stopwatch tutorial, run against rustual."""

from rustual.demo.stopwatch import Stopwatch, StopwatchApp, TimeDisplay


async def test_stopwatch_tutorial():
    app = StopwatchApp()
    async with app.run_test(size=(80, 30)) as pilot:
        assert len(app.query(Stopwatch)) == 3
        first = app.query(Stopwatch).first()
        await pilot.click(first.query_one("#start"))
        assert first.has_class("started")
        await pilot.pause(0.2)
        await pilot.click(first.query_one("#stop"))
        assert not first.has_class("started")
        elapsed = first.query_one(TimeDisplay).time
        assert 0.1 < elapsed < 1.0
        assert first.query_one(TimeDisplay).value != "00:00:00.00"
        await pilot.click(first.query_one("#reset"))
        assert first.query_one(TimeDisplay).time == 0
        await pilot.press("a")
        assert len(app.query(Stopwatch)) == 4
        await pilot.press("r", "r")
        assert len(app.query(Stopwatch)) == 2
        await pilot.press("d")
        assert app.theme == "textual-light"

from rustual import on
from rustual.app import App, ComposeResult
from rustual.containers import Horizontal, Vertical
from rustual.reactive import reactive
from rustual.widgets import Button, Footer, Header, Input, Label, Static


class Counter(Static):
    count = reactive(0)

    def render(self) -> str:
        return f"Count: [b]{self.count}[/b]"


class CounterApp(App):
    CSS = """
    Counter { height: 3; border: round $accent; }
    """
    BINDINGS = [("+", "increment", "Add one")]

    def compose(self) -> ComposeResult:
        yield Header()
        with Vertical():
            yield Counter(id="counter")
            with Horizontal():
                yield Button("Add", id="add", variant="primary")
                yield Button("Reset", id="reset")
            yield Input(placeholder="type", id="name")
            yield Label("", id="echo")
        yield Footer()

    def action_increment(self) -> None:
        self.query_one(Counter).count += 1

    @on(Button.Pressed, "#add")
    def add(self) -> None:
        self.query_one("#counter", Counter).count += 1

    def on_button_pressed(self, event: Button.Pressed) -> None:
        if event.button.id == "reset":
            self.query_one(Counter).count = 0

    def on_input_changed(self, event: Input.Changed) -> None:
        self.query_one("#echo", Label).update(f"Hello {event.value}")


async def test_counter_app():
    app = CounterApp()
    async with app.run_test(size=(60, 20)) as pilot:
        counter = app.query_one(Counter)
        assert counter.count == 0
        await pilot.press("+")
        assert counter.count == 1
        await pilot.click("#add")
        assert counter.count == 2
        assert "Count: 2" in pilot.screen_text()
        await pilot.click("#reset")
        assert counter.count == 0
        app.query_one(Input).focus()
        await pilot.press("h", "i")
        assert app.query_one(Input).value == "hi"
        assert "Hello hi" in pilot.screen_text()


async def test_header_clock_local_time_and_format():
    import re
    from datetime import datetime

    class ClockApp(App):
        def compose(self) -> ComposeResult:
            yield Header(show_clock=True, time_format="%H:%M", id="h")

    app = ClockApp()
    before = datetime.now().strftime("%H:%M")
    async with app.run_test(size=(40, 3)) as pilot:
        header = app.query_one("#h", Header)
        assert header.time_format == "%H:%M"
        first = pilot.screen_text().splitlines()[0].rstrip()
        # Local time like Textual's datetime.now() (either side of a minute boundary).
        assert first[-5:] in {before, datetime.now().strftime("%H:%M")}, first
        header.time_format = "<%S>"
        await pilot.pause()
        assert header.time_format == "<%S>"
        assert re.search(r"<\d\d>$", pilot.screen_text().splitlines()[0].rstrip())

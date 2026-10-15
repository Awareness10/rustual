"""The Python example shown in the README (kept passing)."""

from rustual import on
from rustual.app import App, ComposeResult
from rustual.reactive import reactive
from rustual.widgets import Button, Footer, Header, Static


class Counter(Static):
    count = reactive(0)

    def render(self) -> str:
        return f"Count: [b]{self.count}[/b]"


class CounterApp(App):
    CSS = "Counter { height: 3; border: round $accent; }"
    BINDINGS = [("+", "increment", "Add one")]

    def compose(self) -> ComposeResult:
        yield Header()
        yield Counter()
        yield Button("Add", id="add", variant="primary")
        yield Footer()

    def action_increment(self) -> None:
        self.query_one(Counter).count += 1

    @on(Button.Pressed, "#add")
    def add(self) -> None:
        self.action_increment()


async def test_counter():
    app = CounterApp()
    async with app.run_test() as pilot:
        await pilot.press("+")
        assert app.query_one(Counter).count == 1
        assert "Count: 1" in pilot.screen_text()
        await pilot.click("#add")
        assert app.query_one(Counter).count == 2

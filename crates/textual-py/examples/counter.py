"""A counter app: reactive attributes, bindings, buttons, @on and an input."""

from textual_rs import on
from textual_rs.app import App, ComposeResult
from textual_rs.containers import Horizontal, Vertical
from textual_rs.reactive import reactive
from textual_rs.widgets import Button, Footer, Header, Input, Label, Static


class Counter(Static):
    count = reactive(0)

    def render(self) -> str:
        return f"Count: [b]{self.count}[/b]"


class CounterApp(App[int]):
    CSS = """
    Counter { height: 3; border: round $accent; padding: 0 1; }
    #buttons { height: auto; }
    #echo { margin: 1 2; }
    """
    BINDINGS = [("+", "increment", "Add one"), ("q", "quit", "Quit")]

    def compose(self) -> ComposeResult:
        yield Header(show_clock=True)
        with Vertical():
            yield Counter()
            with Horizontal(id="buttons"):
                yield Button("Add", id="add", variant="primary")
                yield Button("Reset", id="reset", variant="error")
            yield Input(placeholder="What's your name?")
            yield Label("", id="echo")
        yield Footer()

    def action_increment(self) -> None:
        self.query_one(Counter).count += 1

    def action_quit(self) -> None:
        self.exit(self.query_one(Counter).count)

    @on(Button.Pressed, "#add")
    def add(self) -> None:
        self.action_increment()

    @on(Button.Pressed, "#reset")
    def reset(self) -> None:
        self.query_one(Counter).count = 0

    def on_input_changed(self, event: Input.Changed) -> None:
        self.query_one("#echo", Label).update(f"Hello, [b]{event.value}[/b]!")


if __name__ == "__main__":
    print("final count:", CounterApp().run())

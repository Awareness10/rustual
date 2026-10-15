"""Rich renderables in Static and render()."""

from rich.panel import Panel
from rich.table import Table
from rich.text import Text

from rustual.app import App, ComposeResult
from rustual.widget import Widget
from rustual.widgets import Static


def make_table() -> Table:
    table = Table(title="Planets")
    table.add_column("Name", style="cyan")
    table.add_column("Moons", justify="right")
    table.add_row("Earth", "1")
    table.add_row("Mars", "2")
    return table


class PanelWidget(Widget):
    DEFAULT_CSS = "PanelWidget { height: auto; }"

    def render(self) -> Panel:
        return Panel("inside a panel", title="P")


async def test_rich_table_in_static_and_update():
    class MyApp(App):
        def compose(self) -> ComposeResult:
            yield Static(make_table(), id="t")
            yield PanelWidget()
            yield Static(Text("styled text", style="bold red"), id="txt")

    app = MyApp()
    async with app.run_test(size=(60, 20)) as pilot:
        text = pilot.screen_text()
        assert "Planets" in text and "Earth" in text and "Mars" in text
        assert "inside a panel" in text
        assert "styled text" in text
        static = app.query_one("#t", Static)
        assert static.size.height >= 6  # auto height measured from the rendered table
        static.update("plain now")
        await pilot.pause()
        assert "plain now" in pilot.screen_text()
        assert "Planets" not in pilot.screen_text()
        static.update(make_table())
        await pilot.pause()
        assert "Planets" in pilot.screen_text()

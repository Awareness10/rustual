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


def test_rich_text_theme_styles_become_markup():
    """Rich theme style names (``repr.str``) resolve to real styles, as in Textual."""
    from rich.highlighter import ReprHighlighter

    from rustual.content import to_markup

    label = Text.assemble(Text.from_markup("[b]code[/b]="), ReprHighlighter()(repr("x")))
    markup, is_markup = to_markup(label)
    assert is_markup
    assert "repr." not in markup
    assert "[bold]code[/bold]=" in markup.replace("not bold ", "")
    assert "ansi_green" in markup
    assert Text("a [b] c").markup and to_markup(Text("a [b] c"))[0] == r"a \[b] c"


async def test_tree_label_from_highlighted_rich_text():
    """Textual's json_tree example: labels built with ReprHighlighter are styled, not literal."""
    from rich.highlighter import ReprHighlighter

    from rustual.widgets import Tree

    class TreeApp(App):
        def compose(self) -> ComposeResult:
            yield Tree("Root")

        def on_mount(self) -> None:
            tree = self.query_one(Tree)
            label = Text.assemble(Text.from_markup("[b]n[/b]="), ReprHighlighter()(repr(2)))
            tree.root.add_leaf(label)
            tree.root.expand()

    app = TreeApp()
    async with app.run_test(size=(40, 5)) as pilot:
        text = pilot.screen_text()
        assert "n=2" in text
        assert "[" not in text.replace("[]", "")

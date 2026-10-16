from rustual.app import App, ComposeResult
from rustual.theme import Theme
from rustual.widgets import Label


async def test_register_and_switch_theme():
    class MyApp(App):
        def compose(self) -> ComposeResult:
            yield Label("x")

        def on_mount(self) -> None:
            self.register_theme(Theme(name="arctic", primary="#88C0D0", dark=True))
            self.theme = "arctic"

    app = MyApp()
    app.register_theme(Theme(name="early", primary="#ff0000", dark=False))
    async with app.run_test() as pilot:
        assert app.theme == "arctic"
        assert "early" in app.available_themes
        app.theme = "early"
        assert app.theme == "early"
        app.theme = "textual-light"
        assert app.theme == "textual-light"


async def test_ansi_themes_and_ansi_color():
    from rustual.app import App
    from rustual.widgets import Static

    class AnsiApp(App):
        CSS = "#s { width: 1; } #s:ansi { width: 2; }"

        def compose(self):
            yield Static("x", id="s")

    app = AnsiApp()
    app.theme = "ansi-dark"
    assert app.native_ansi_color
    assert app.get_theme("ansi-light").ansi
    async with app.run_test() as pilot:
        assert "ansi-light" in app.available_themes
        assert app.native_ansi_color
        assert app.query_one("#s").region.width == 2
        app.ansi_color = False
        await pilot.pause()
        assert not app.native_ansi_color
        assert app.query_one("#s").region.width == 1
        app.ansi_color = None
        await pilot.pause()
        assert app.query_one("#s").region.width == 2

    # A truecolor theme unless ansi_color=True.
    app = AnsiApp(ansi_color=True)
    async with app.run_test():
        assert app.native_ansi_color
        assert app.query_one("#s").region.width == 2

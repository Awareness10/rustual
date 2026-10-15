from textual_rs.app import App, ComposeResult
from textual_rs.theme import Theme
from textual_rs.widgets import Label


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

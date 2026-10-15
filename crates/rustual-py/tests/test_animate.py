from rustual.app import App, ComposeResult
from rustual.reactive import reactive
from rustual.widget import Widget
from rustual.widgets import Label


class Bar(Widget):
    value = reactive(0.0)

    def render(self) -> str:
        return f"value={self.value:.0f}"


async def test_animate_reactive_and_style():
    done = []

    class MyApp(App):
        def compose(self) -> ComposeResult:
            yield Bar()
            yield Label("x", id="l")

    app = MyApp()
    async with app.run_test() as pilot:
        bar = app.query_one(Bar)
        bar.animate("value", 100, duration=0.1, easing="linear", on_complete=lambda: done.append(1))
        await pilot.pause(0.05)
        assert 0 < bar.value < 100
        await pilot.pause(0.1)
        assert bar.value == 100 and done == [1]
        assert "value=100" in pilot.screen_text()
        label = app.query_one("#l")
        label.styles.animate("width", 20, duration=0.05)
        await pilot.pause(0.12)
        assert label.region.width == 20

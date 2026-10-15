"""Events on custom widgets, focus, and widget-level bindings."""

from textual_rs import events
from textual_rs.app import App, ComposeResult
from textual_rs.binding import Binding
from textual_rs.widget import Widget
from textual_rs.widgets import Label


class Pad(Widget, can_focus=True):
    DEFAULT_CSS = "Pad { height: 5; width: 20; }"
    BINDINGS = [Binding("space", "poke", "Poke"), ("r", "reset", "Reset")]

    def __init__(self, **kwargs) -> None:
        super().__init__(**kwargs)
        self.log_: list[str] = []

    def on_click(self, event: events.Click) -> None:
        self.log_.append(f"click {event.x},{event.y} b{event.button}")

    def on_mouse_down(self, event: events.MouseDown) -> None:
        self.log_.append("down")

    def on_enter(self) -> None:
        self.log_.append("enter")

    def on_leave(self) -> None:
        self.log_.append("leave")

    def on_focus(self) -> None:
        self.log_.append("focus")

    def on_blur(self) -> None:
        self.log_.append("blur")

    def on_paste(self, event: events.Paste) -> None:
        self.log_.append(f"paste {event.text}")

    def action_poke(self) -> None:
        self.log_.append("poke")

    def action_reset(self) -> None:
        self.log_.clear()

    def render(self) -> str:
        return "pad"


async def test_mouse_focus_bindings_paste_resize():
    resized = []

    class MyApp(App):
        def compose(self) -> ComposeResult:
            yield Pad(id="pad")
            yield Label("other", id="other")

        def on_resize(self, event: events.Resize) -> None:
            resized.append(tuple(event.size))

    app = MyApp()
    async with app.run_test(size=(40, 12)) as pilot:
        pad = app.query_one(Pad)
        assert pad.has_focus  # auto-focus
        await pilot.click(pad, offset=(2, 1))
        assert "down" in pad.log_
        assert "click 2,1 b1" in pad.log_
        await pilot.press("space")
        assert pad.log_[-1] == "poke"
        await pilot.press("r")
        assert pad.log_ == []
        await pilot.paste("hello")
        assert pad.log_ == ["paste hello"]
        await pilot.hover("#other")
        assert "leave" in pad.log_
        await pilot.hover(pad)
        assert pad.log_[-1] == "enter"
        pad.blur()
        await pilot.pause()
        assert pad.log_[-1] == "blur"
        pad.focus()
        await pilot.pause()
        assert pad.log_[-1] == "focus"
        await pilot.resize_terminal(50, 14)
        assert resized and resized[-1] == (50, 14)

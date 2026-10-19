"""Textual APIs that real apps import (found by scanning popular Textual apps): shims and
small features that must exist for those apps to start."""

import logging

import pytest

from rustual.app import App, ComposeResult, InvalidThemeError
from rustual.containers import Vertical
from rustual.widgets import Input, Label


def test_imports_that_apps_use():
    from rustual import log  # noqa: F401
    from rustual.await_complete import AwaitComplete  # noqa: F401
    from rustual.cache import FIFOCache, LRUCache  # noqa: F401
    from rustual.design import ColorSystem  # noqa: F401
    from rustual.dom import DOMNode, NoScreen  # noqa: F401
    from rustual.errors import NoWidget  # noqa: F401
    from rustual.events import DescendantBlur, DescendantFocus, ScreenResume, ScreenSuspend  # noqa: F401
    from rustual.geometry import clamp
    from rustual.lazy import Lazy, Reveal  # noqa: F401
    from rustual.markup import escape
    from rustual.notifications import SeverityLevel  # noqa: F401
    from rustual.theme import BUILTIN_THEMES
    from rustual.timer import Timer  # noqa: F401
    from rustual.types import AnimationLevel, CSSPathType, IgnoreReturnCallbackType, InputValidationOn  # noqa: F401

    assert clamp(5, 10, 0) == 5 and clamp(-1, 0, 10) == 0 and clamp(11, 0, 10) == 10
    assert escape("[b]bold[/b] [not a tag") == r"\[b]bold\[/b] [not a tag"
    assert escape("trailing\\") == "trailing\\\\"
    assert {"textual-dark", "nord", "ansi-dark"} <= set(BUILTIN_THEMES)
    assert BUILTIN_THEMES["ansi-dark"].ansi


def test_lru_cache():
    from rustual.cache import LRUCache

    cache: LRUCache[str, int] = LRUCache(2)
    cache["a"] = 1
    cache["b"] = 2
    assert cache.get("a") == 1  # "a" is now the most recent
    cache["c"] = 3
    assert "b" not in cache and "a" in cache and len(cache) == 2


def test_color_system_matches_theme_variables():
    from rustual.design import ColorSystem
    from rustual.theme import BUILTIN_THEMES

    variables = ColorSystem("#0178D4", dark=True).generate()
    assert variables["primary"].upper() == "#0178D4"
    assert "text-muted" in variables and "primary-darken-3" in variables
    nord = BUILTIN_THEMES["nord"].to_color_system().generate()
    assert nord["primary"].upper() == BUILTIN_THEMES["nord"].primary.upper()


def test_log(tmp_path, monkeypatch, caplog):
    from rustual import log

    path = tmp_path / "log.txt"
    monkeypatch.setenv("TEXTUAL_LOG", str(path))
    with caplog.at_level(logging.DEBUG, logger="rustual"):
        log("hello", answer=42)
        log.warning("careful")
        Label("x").log.info("from a widget")
    assert path.read_text().splitlines() == ["hello answer=42", "careful", "from a widget"]
    assert [r.levelname for r in caplog.records] == ["INFO", "WARNING", "INFO"]


async def test_lazy_mounts_the_widget():
    from rustual.lazy import Lazy

    class LazyApp(App):
        def compose(self) -> ComposeResult:
            with Lazy(Vertical(id="v")):
                yield Label("inside", id="l")

    app = LazyApp()
    async with app.run_test():
        assert app.query_one("#l").parent.id == "v"


async def test_invalid_theme_error():
    app = App()
    async with app.run_test():
        with pytest.raises(InvalidThemeError):
            app.theme = "no-such-theme"
        with pytest.raises(ValueError):  # it is also a ValueError
            app.theme = "no-such-theme"


async def test_descendant_focus_and_screen_resume():
    from rustual.screen import Screen

    seen = []

    class Box(Vertical):
        def on_descendant_focus(self, event) -> None:
            seen.append(("focus", event.widget.id))

        def on_descendant_blur(self, event) -> None:
            seen.append(("blur", event.widget.id))

    class Other(Screen):
        def on_screen_resume(self) -> None:
            seen.append(("resume", "other"))

        def on_screen_suspend(self) -> None:
            seen.append(("suspend", "other"))

    class FocusApp(App):
        def compose(self) -> ComposeResult:
            with Box():
                yield Input(id="a")
                yield Input(id="b")

    app = FocusApp()
    async with app.run_test() as pilot:
        await pilot.press("tab")
        await app.push_screen(Other())
        await pilot.pause()
        app.pop_screen()
        await pilot.pause()
    assert ("focus", "a") in seen and ("blur", "a") in seen and ("focus", "b") in seen, seen
    assert seen.index(("resume", "other")) < seen.index(("suspend", "other")), seen
    # Focus returns to #b when the screen is popped.
    assert seen[-1] == ("focus", "b"), seen


async def test_screen_title_overrides_app_title():
    from rustual.screen import Screen
    from rustual.widgets import Header

    class Titled(Screen):
        TITLE = "Screen title"

        def compose(self) -> ComposeResult:
            yield Header()

    class TitleApp(App):
        TITLE = "App title"

        def compose(self) -> ComposeResult:
            yield Header()

    app = TitleApp()
    async with app.run_test() as pilot:
        assert "App title" in pilot.screen_text().splitlines()[0]
        screen = Titled()
        await app.push_screen(screen)
        await pilot.pause()
        assert "Screen title" in pilot.screen_text().splitlines()[0]
        screen.sub_title = "sub"
        await pilot.pause()
        assert "Screen title — sub" in pilot.screen_text().splitlines()[0]
        screen.title = None
        await pilot.pause()
        assert "App title — sub" in pilot.screen_text().splitlines()[0]
        assert screen.is_active


async def test_rich_styles():
    from rustual.widget import Widget

    class Styled(Widget):
        COMPONENT_CLASSES = {"styled--title"}
        DEFAULT_CSS = """
        Styled { color: #ff0000; background: #000080; text-style: italic; height: 1; }
        Styled > .styled--title { color: #00ff00; text-style: bold; }
        """

    class StyleApp(App):
        def compose(self) -> ComposeResult:
            yield Styled(id="s")

    app = StyleApp()
    async with app.run_test():
        w = app.query_one("#s")
        style = w.rich_style
        assert style.color.name == "#ff0000" and style.bgcolor.name == "#000080" and style.italic
        title = w.get_component_rich_style("styled--title")
        assert title.color.name == "#00ff00" and title.bold and title.bgcolor.name == "#000080"
        partial = w.get_component_rich_style("styled--title", partial=True)
        assert partial.color.name == "#00ff00" and partial.bold and not partial.italic
        with pytest.raises(KeyError):
            w.get_component_rich_style("nope")
        assert w.get_component_rich_style("nope", default=style) is style


async def test_data_bind_set_reactive_bind_and_misc():
    from rustual.reactive import reactive
    from rustual.widget import Widget

    class Child(Widget):
        count = reactive(0)

    class Parent(Widget):
        total = reactive(5)
        hits: list = []

        def compose(self) -> ComposeResult:
            yield Child(id="c1").data_bind(count=Parent.total)

        def watch_total(self, value: int) -> None:
            Parent.hits.append(value)

    class BindApp(App):
        def compose(self) -> ComposeResult:
            yield Parent(id="p")

        def on_mount(self) -> None:
            self.bind("x", "bell", description="Bell")

        def action_bell(self) -> None:
            self.rang = True

    app = BindApp()
    async with app.run_test() as pilot:
        parent, child = app.query_one("#p"), app.query_one("#c1")
        assert child.count == 5
        parent.total = 7
        await pilot.pause()
        assert child.count == 7
        before = list(Parent.hits)
        parent.set_reactive(Parent.total, 9)  # no watcher
        assert parent.total == 9 and Parent.hits == before
        await pilot.press("x")
        assert app.rang
        app.notify("hello")
        await pilot.pause()
        assert "hello" in pilot.screen_text()
        app.clear_notifications()
        await pilot.pause()
        assert "hello" not in pilot.screen_text()
        from rustual.binding import Binding

        assert app.get_key_display(Binding("ctrl+s", "save")) == "^s"
        assert app.get_key_display(Binding("f1", "help", key_display="F1!")) == "F1!"
        assert child.is_running and child.scrollable_content_region == child.content_region


async def test_dataclass_messages():
    from dataclasses import dataclass

    from rustual.message import Message

    @dataclass
    class Picked(Message):
        value: str

    got = []

    class MsgApp(App):
        def on_picked(self, message: Picked) -> None:
            got.append((message.value, message.sender is self))

    app = MsgApp()
    async with app.run_test() as pilot:
        app.post_message(Picked("x"))
        await pilot.pause()
    assert got == [("x", True)]


def test_color_constants():
    from rustual._color_constants import ANSI_COLORS, COLOR_NAME_TO_RGB

    assert COLOR_NAME_TO_RGB["red"] == (255, 0, 0)
    assert "ansi_bright_blue" in COLOR_NAME_TO_RGB and ANSI_COLORS[4] == "blue"


async def test_breakpoints():
    from rustual.containers import Grid
    from rustual.screen import Screen
    from rustual.widgets import Placeholder

    # Textual's examples/breakpoints.py (imports swapped).
    class BreakpointApp(App):
        HORIZONTAL_BREAKPOINTS = [(0, "-narrow"), (40, "-normal"), (80, "-wide"), (120, "-very-wide")]
        CSS = """
        Screen {
            Placeholder { padding: 2; }
            Grid { grid-rows: auto; height: auto; }
            &.-narrow { Grid { grid-size: 1; } }
            &.-normal { Grid { grid-size: 2; } }
            &.-wide { Grid { grid-size: 4; } }
            &.-very-wide { Grid { grid-size: 6; } }
        }
        """

        def compose(self) -> ComposeResult:
            with Grid():
                for n in range(16):
                    yield Placeholder(f"Placeholder {n + 1}", id=f"p{n}")

    def columns(app) -> int:
        y = app.query_one("#p0").region.y
        return sum(1 for n in range(16) if app.query_one(f"#p{n}").region.y == y)

    app = BreakpointApp()
    async with app.run_test(size=(100, 40)) as pilot:
        assert app.screen.has_class("-wide") and columns(app) == 4
        await pilot.resize_terminal(30, 40)
        assert app.screen.has_class("-narrow") and not app.screen.has_class("-wide")
        assert columns(app) == 1
        await pilot.resize_terminal(130, 40)
        assert columns(app) == 6

        class Fixed(Screen):
            HORIZONTAL_BREAKPOINTS = [(0, "-fixed")]

        await app.push_screen(Fixed())
        await pilot.pause()
        assert app.screen.has_class("-fixed") and not app.screen.has_class("-very-wide")


async def test_getters():
    from rustual import getters
    from rustual.css.query import NoMatches
    from rustual.widgets import Input

    class GetterApp(App):
        field = getters.query_one("#field", Input)
        first_input = getters.query_one(Input)
        direct = getters.child_by_id("field", Input)
        missing = getters.child_by_id("nope")

        def compose(self) -> ComposeResult:
            yield Input(id="field")

    class Probe(Label):
        app = getters.app(GetterApp)

    app = GetterApp()
    async with app.run_test():
        assert app.field.id == "field" and app.first_input is app.field
        assert app.direct is app.field or app.direct.id == "field"
        with pytest.raises(NoMatches):
            _ = app.missing
        assert Probe("x").app is app


async def test_footer_shows_custom_bindings_like_textual():
    from rustual.binding import Binding
    from rustual.widgets import Footer

    class KeysApp(App):
        BINDINGS = [
            Binding("ctrl+q", "quit", "Quit"),
            Binding("ctrl+s", "save", "Save"),
            Binding("escape", "back", "Back"),
            Binding("question_mark", "about", "About"),
            Binding("f2", "hidden", "Hidden", show=False),
        ]

        def compose(self) -> ComposeResult:
            yield Footer()

        def action_save(self) -> None:
            pass

    app = KeysApp()
    async with app.run_test(size=(100, 3)) as pilot:
        footer = pilot.screen_text().splitlines()[-1]
        assert footer.startswith(" ^q Quit  ^s Save  esc Back  ? About "), footer
        assert "Hidden" not in footer
        await pilot.press("ctrl+q")
    assert app.return_code == 0 or app.return_code is None

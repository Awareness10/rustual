"""Core behaviour: lifecycle, reactivity, messages, screens, timers, workers, threads, errors."""

import asyncio
import threading

import pytest

from textual_rs import on, work
from textual_rs.app import App, ComposeResult
from textual_rs.containers import Container, Vertical
from textual_rs.css.query import NoMatches, WrongType
from textual_rs.events import Key, Mount
from textual_rs.message import Message
from textual_rs.reactive import reactive, var
from textual_rs.screen import ModalScreen, Screen
from textual_rs.widget import Widget
from textual_rs.widgets import Button, Label, Static
from textual_rs.worker import Worker, WorkerState, get_current_worker


async def test_compose_mount_order_and_query():
    events = []

    class Child(Static):
        def on_mount(self) -> None:
            events.append(("child", self.id))

    class MyApp(App):
        def compose(self) -> ComposeResult:
            with Vertical(id="outer"):
                yield Child("a", id="a")
                yield Child("b", id="b", classes="x y")

        def on_mount(self, event: Mount) -> None:
            events.append(("app", type(event).__name__))

    app = MyApp()
    async with app.run_test() as pilot:
        assert ("child", "a") in events and ("app", "Mount") in events
        assert [w.id for w in app.query(Child)] == ["a", "b"]
        b = app.query_one("#b", Child)
        assert b.has_class("x", "y")
        assert b.parent.id == "outer"
        assert b.parent.parent is app.screen
        assert app.query_one("#outer").children == [app.query_one("#a"), b]
        assert app.query_one(Child) is app.query_one("#a")
        with pytest.raises(NoMatches):
            app.query_one("#nope")
        with pytest.raises(WrongType):
            app.query_one("#a", Button)
        b.add_class("z")
        assert "z" in b.classes
        b.remove_class("x")
        assert not b.has_class("x")
        b.toggle_class("q")
        assert b.has_class("q")
        assert b.match(".q")
        assert len(app.query(".y")) == 1


async def test_reactive_watch_validate_compute():
    calls = []

    class W(Widget):
        value = reactive(0)
        doubled = reactive(0)
        name_ = var("x")

        def validate_value(self, v: int) -> int:
            return max(0, min(10, v))

        def watch_value(self, old: int, new: int) -> None:
            calls.append((old, new))

        def compute_doubled(self) -> int:
            return self.value * 2

        def render(self) -> str:
            return f"v={self.value} d={self.doubled}"

    class MyApp(App):
        def compose(self) -> ComposeResult:
            yield W()

    app = MyApp()
    async with app.run_test() as pilot:
        w = app.query_one(W)
        w.value = 5
        assert w.doubled == 10
        w.value = 50
        assert w.value == 10
        assert (5, 10) in calls
        await pilot.pause()
        assert "v=10 d=20" in pilot.screen_text()


async def test_custom_message_bubbles_and_stops():
    seen = []

    class Ping(Message):
        def __init__(self, n: int) -> None:
            super().__init__()
            self.n = n

    class Inner(Widget):
        class Done(Message):
            pass

        def on_mount(self) -> None:
            self.post_message(Ping(1))
            self.post_message(self.Done())

    class Middle(Container):
        def on_ping(self, message: Ping) -> None:
            seen.append(("middle", message.n, message.sender.id))

        def on_inner_done(self, message: Inner.Done) -> None:
            seen.append(("middle-done",))
            message.stop()

    class MyApp(App):
        def compose(self) -> ComposeResult:
            with Middle():
                yield Inner(id="inner")

        def on_ping(self, message: Ping) -> None:
            seen.append(("app", message.n))

        def on_inner_done(self, message: Inner.Done) -> None:
            seen.append(("app-done",))

    assert Inner.Done.handler_name == "on_inner_done"
    app = MyApp()
    async with app.run_test():
        pass
    assert ("middle", 1, "inner") in seen
    assert ("app", 1) in seen
    assert ("middle-done",) in seen
    assert ("app-done",) not in seen


async def test_on_decorator_selectors_and_key_methods():
    pressed = []

    class MyApp(App):
        def compose(self) -> ComposeResult:
            yield Button("One", id="one")
            yield Button("Two", id="two", classes="special")

        @on(Button.Pressed, "#one")
        def one(self) -> None:
            pressed.append("one")

        @on(Button.Pressed, ".special")
        def two(self, event: Button.Pressed) -> None:
            pressed.append(event.button.id)

        def key_x(self) -> None:
            pressed.append("key-x")

        def on_key(self, event: Key) -> None:
            pressed.append(f"on_key:{event.key}")

    app = MyApp()
    async with app.run_test() as pilot:
        app.query_one("#one", Button).press()
        app.query_one("#two", Button).press()
        await pilot.pause()
        await pilot.press("x")
    assert "one" in pressed and "two" in pressed
    assert "on_key:x" in pressed and "key-x" in pressed


async def test_mount_remove_dynamic():
    class MyApp(App):
        def compose(self) -> ComposeResult:
            yield Vertical(id="list")

        async def on_mount(self) -> None:
            box = self.query_one("#list")
            await box.mount(Label("first", id="first"))
            box.mount(Label("zero", id="zero"), before=0)
            box.mount(Label("last", id="last"))

    app = MyApp()
    async with app.run_test() as pilot:
        box = app.query_one("#list")
        assert [c.id for c in box.children] == ["zero", "first", "last"]
        await app.query_one("#first").remove()
        await pilot.pause()
        assert [c.id for c in box.children] == ["zero", "last"]
        await box.remove_children()
        await pilot.pause()
        assert box.children == []


async def test_screens_push_dismiss_callback_and_modal():
    results = []

    class Ask(ModalScreen[str]):
        def compose(self) -> ComposeResult:
            yield Button("Yes", id="yes")

        def on_button_pressed(self, event: Button.Pressed) -> None:
            self.dismiss("yes!")

    class Other(Screen):
        CSS = "Other { background: red; }"

        def compose(self) -> ComposeResult:
            yield Label("other screen", id="other-label")

    class MyApp(App):
        SCREENS = {"other": Other}

        def compose(self) -> ComposeResult:
            yield Label("main")

    app = MyApp()
    async with app.run_test() as pilot:
        base = app.screen
        app.push_screen(Ask(), results.append)
        await pilot.pause()
        assert isinstance(app.screen, Ask)
        assert app.screen.is_modal
        await pilot.click("#yes")
        assert results == ["yes!"]
        assert app.screen is base
        app.push_screen("other")
        await pilot.pause()
        assert app.query_one("#other-label").screen is app.screen
        assert "other screen" in pilot.screen_text()
        app.pop_screen()
        await pilot.pause()
        assert app.screen is base


async def test_timers_and_intervals():
    ticks = []

    class MyApp(App):
        def on_mount(self) -> None:
            self.timer = self.set_interval(0.02, lambda: ticks.append("i"))
            self.set_timer(0.03, lambda: ticks.append("t"))

    app = MyApp()
    async with app.run_test() as pilot:
        await pilot.pause(0.15)
        app.timer.stop()
        n = ticks.count("i")
        await pilot.pause(0.06)
        assert ticks.count("i") == n
    assert ticks.count("t") == 1
    assert n >= 3


async def test_async_handler_and_async_worker():
    log = []

    class MyApp(App):
        def compose(self) -> ComposeResult:
            yield Label("", id="out")

        async def on_mount(self) -> None:
            await asyncio.sleep(0.01)
            log.append("mounted-after-sleep")
            self.fetch()

        @work(exclusive=True)
        async def fetch(self) -> str:
            assert get_current_worker().name == "fetch"
            await asyncio.sleep(0.01)
            self.query_one("#out", Label).update("fetched")
            return "data"

        def on_worker_state_changed(self, event: Worker.StateChanged) -> None:
            log.append(event.state)

    app = MyApp()
    async with app.run_test() as pilot:
        await pilot.pause(0.1)
        assert "fetched" in pilot.screen_text()
    assert "mounted-after-sleep" in log
    assert WorkerState.SUCCESS in log


async def test_thread_worker_updates_ui_from_thread():
    class MyApp(App):
        def compose(self) -> ComposeResult:
            yield Label("", id="out")

        def on_mount(self) -> None:
            self.run_worker(self.crunch, thread=True)

        def crunch(self) -> int:
            assert threading.current_thread() is not threading.main_thread()
            total = sum(range(10_000))
            # Direct widget calls from a thread are marshalled to the app thread.
            self.query_one("#out", Label).update(f"total={total}")
            self.call_from_thread(self.notify, "done")
            return total

    app = MyApp()
    async with app.run_test() as pilot:
        for _ in range(100):
            await pilot.pause(0.01)
            if "total=" in pilot.screen_text():
                break
        assert "total=49995000" in pilot.screen_text()


async def test_exception_in_handler_propagates():
    class Boom(Exception):
        pass

    class MyApp(App):
        def compose(self) -> ComposeResult:
            yield Button("x", id="x")

        def on_button_pressed(self) -> None:
            raise Boom("bad")

    app = MyApp()
    with pytest.raises(Boom):
        async with app.run_test() as pilot:
            await pilot.click("#x")


async def test_exit_return_value_and_styles():
    class MyApp(App[int]):
        def compose(self) -> ComposeResult:
            yield Label("hi", id="l")

        def on_mount(self) -> None:
            label = self.query_one("#l")
            label.styles.background = "red"
            label.styles.width = 10
            label.display = False

    app = MyApp()
    async with app.run_test() as pilot:
        label = app.query_one("#l")
        assert label.display is False
        label.display = True
        await pilot.pause()
        assert label.display is True
        assert label.region.width == 10
        await pilot.exit(42)
        assert pilot.has_exited
    assert app.return_value == 42


async def test_pre_mount_configuration_and_border_title():
    class MyApp(App):
        def compose(self) -> ComposeResult:
            s = Static("boxed", id="s")
            s.border_title = "Title"
            s.styles.border = ("round", "green")
            s.add_class("extra")
            yield s

    app = MyApp()
    async with app.run_test() as pilot:
        s = app.query_one("#s")
        assert s.has_class("extra")
        assert "Title" in pilot.screen_text()


async def test_push_screen_wait_from_worker():
    got = []

    class Ask(ModalScreen[bool]):
        def compose(self) -> ComposeResult:
            yield Button("OK", id="ok")

        def on_button_pressed(self) -> None:
            self.dismiss(True)

    class MyApp(App):
        def on_mount(self) -> None:
            self.ask()

        @work
        async def ask(self) -> None:
            got.append(await self.push_screen_wait(Ask()))

    app = MyApp()
    async with app.run_test() as pilot:
        await pilot.pause()
        assert isinstance(app.screen, Ask)
        await pilot.click("#ok")
        await pilot.pause()
    assert got == [True]

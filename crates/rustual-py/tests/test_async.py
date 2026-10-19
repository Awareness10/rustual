"""Messages and calls across threads, the asyncio integration, AUTO_FOCUS and worker cleanup."""

from __future__ import annotations

import asyncio
import json
import os
import subprocess
import sys
import textwrap
import threading
import time
from pathlib import Path

import pytest

from rustual import work
from rustual.app import App, ComposeResult
from rustual.containers import Vertical
from rustual.message import Message
from rustual.screen import Screen
from rustual.widgets import Button, Input, Label
from rustual.worker import WorkerState


class Tick(Message):
    def __init__(self, n: int) -> None:
        super().__init__()
        self.n = n


async def test_post_message_from_a_thread_never_waits_for_the_app():
    seen: list[int] = []

    class MyApp(App):
        def compose(self) -> ComposeResult:
            yield Label("")

        def on_tick(self, message: Tick) -> None:
            seen.append(message.n)

    app = MyApp()
    async with app.run_test() as pilot:
        # Under run_test the app only runs while the test awaits, so a blocking post_message
        # would never return here.
        thread = threading.Thread(target=lambda: [app.post_message(Tick(n)) for n in range(500)])
        thread.start()
        thread.join(2)
        assert not thread.is_alive(), "post_message from a thread blocked"
        assert seen == []
        await pilot.pause()
        assert seen == list(range(500))


async def test_post_message_from_a_thread_to_a_removed_widget_is_dropped():
    seen: list[int] = []

    class Child(Label):
        def on_tick(self, message: Tick) -> None:
            seen.append(message.n)

    class MyApp(App):
        def compose(self) -> ComposeResult:
            yield Child("x")

    app = MyApp()
    async with app.run_test() as pilot:
        child = app.query_one(Child)
        result: list[bool] = []
        thread = threading.Thread(target=lambda: result.append(child.post_message(Tick(1))))
        thread.start()
        thread.join(2)
        assert result == [True]
        await pilot.pause()
        assert seen == [1]
        await child.remove()
        await pilot.pause()
        thread = threading.Thread(target=lambda: child.post_message(Tick(2)))
        thread.start()
        thread.join(2)
        await pilot.pause()
        assert seen == [1]


async def test_call_from_thread_awaits_async_callbacks_and_reraises():
    results: list[object] = []

    class MyApp(App):
        def compose(self) -> ComposeResult:
            yield Input(id="name")

        async def read_value(self, suffix: str) -> str:
            await asyncio.sleep(0.01)
            return self.query_one(Input).value + suffix

        def fail(self) -> None:
            raise KeyError("boom")

        @work(thread=True)
        def in_thread(self) -> None:
            self.call_from_thread(setattr, self.query_one(Input), "value", "abc")
            results.append(self.call_from_thread(self.read_value, "!"))
            try:
                self.call_from_thread(self.fail)
            except KeyError as error:
                results.append(error)

    app = MyApp()
    async with app.run_test() as pilot:
        with pytest.raises(RuntimeError, match="different thread"):
            app.call_from_thread(lambda: None)
        app.in_thread()
        for _ in range(200):
            await pilot.pause(0.01)
            if len(results) == 2:
                break
        assert results[0] == "abc!"
        assert isinstance(results[1], KeyError)


async def test_removing_a_widget_cancels_its_workers():
    class Busy(Label):
        def on_mount(self) -> None:
            self.worker = self.run_worker(self.forever())

        async def forever(self) -> None:
            await asyncio.sleep(60)

    class MyApp(App):
        def compose(self) -> ComposeResult:
            yield Busy("busy")

    app = MyApp()
    async with app.run_test() as pilot:
        busy = app.query_one(Busy)
        await pilot.pause()
        assert busy.worker.state == WorkerState.RUNNING
        await busy.remove()
        await pilot.pause()
        assert busy.worker.state == WorkerState.CANCELLED


async def test_auto_focus_none_focuses_nothing():
    class MyApp(App):
        AUTO_FOCUS = None

        def compose(self) -> ComposeResult:
            yield Input()

    app = MyApp()
    async with app.run_test():
        assert app.focused is None


async def test_auto_focus_selector_and_screen_override():
    class Other(Screen):
        AUTO_FOCUS = "#second"

        def compose(self) -> ComposeResult:
            yield Button("first", id="first")
            yield Button("second", id="second")

    class Nothing(Screen):
        AUTO_FOCUS = ""

        def compose(self) -> ComposeResult:
            yield Button("only")

    class MyApp(App):
        AUTO_FOCUS = "#b"

        def compose(self) -> ComposeResult:
            with Vertical():
                yield Input(id="a")
                yield Input(id="b")

    app = MyApp()
    async with app.run_test() as pilot:
        assert app.focused is not None and app.focused.id == "b"
        app.push_screen(Other())
        await pilot.pause()
        assert app.focused is not None and app.focused.id == "second"
        app.push_screen(Nothing())
        await pilot.pause()
        assert app.focused is None


# -- the asyncio loop under App.run() (in a pseudo-terminal) ---------------------------------------

ROOT = Path(__file__).resolve().parent


def run_in_terminal(code: str, tmp_path: Path, timeout: float = 30) -> dict:
    """Run ``code`` (which writes JSON to ``sys.argv[1]``) in a pseudo-terminal."""
    import pty

    out = tmp_path / "result.json"
    script = tmp_path / "app.py"
    script.write_text(textwrap.dedent(code))
    master, slave = pty.openpty()
    import fcntl
    import struct
    import termios

    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 24, 80, 0, 0))
    proc = subprocess.Popen(
        [sys.executable, str(script), str(out)],
        stdin=slave,
        stdout=slave,
        stderr=slave,
        start_new_session=True,
    )
    os.close(slave)
    output = b""
    deadline = time.monotonic() + timeout
    try:
        while proc.poll() is None and time.monotonic() < deadline:
            import select

            if select.select([master], [], [], 0.05)[0]:
                try:
                    output += os.read(master, 65536)
                except OSError:
                    break
        proc.wait(5)
    finally:
        if proc.poll() is None:
            proc.kill()
        os.close(master)
    assert out.exists(), output.decode(errors="replace")[-2000:]
    return json.loads(out.read_text())


pty_only = pytest.mark.skipif(not sys.platform.startswith("linux"), reason="pseudo-terminal test (Linux)")


@pty_only
def test_asyncio_sleep_wakes_on_time(tmp_path):
    # The loop used to be polled every 10 ms, so sleep(1/60) took 20 ms (50 Hz).
    result = run_in_terminal(
        """
        import asyncio, json, sys, time
        from rustual.app import App
        from rustual.widgets import Label

        class A(App):
            def compose(self):
                yield Label("")

            def on_mount(self):
                self.ticks = 0
                self.run_worker(self.loop())

            async def loop(self):
                await asyncio.sleep(0.1)
                start = time.monotonic()
                while time.monotonic() - start < 1.5:
                    await asyncio.sleep(1 / 60)
                    self.ticks += 1
                self.exit(self.ticks / (time.monotonic() - start))

        rate = A().run()
        json.dump({"rate": rate}, open(sys.argv[1], "w"))
        """,
        tmp_path,
    )
    assert result["rate"] > 54, result


@pty_only
def test_asyncio_socket_reads_wake_the_app_immediately(tmp_path):
    result = run_in_terminal(
        """
        import asyncio, json, socket, statistics, sys, threading, time
        from rustual.app import App
        from rustual.widgets import Label

        class A(App):
            def compose(self):
                yield Label("")

            def on_mount(self):
                self.latencies = []
                self.run_worker(self.serve())

            async def serve(self):
                server = await asyncio.start_server(self.client, "127.0.0.1", 0)
                port = server.sockets[0].getsockname()[1]
                threading.Thread(target=self.send, args=(port,), daemon=True).start()
                async with server:
                    await server.serve_forever()

            def send(self, port):
                with socket.create_connection(("127.0.0.1", port)) as sock:
                    for _ in range(100):
                        sock.sendall(f"{time.perf_counter()}\\n".encode())
                        time.sleep(0.007)

            async def client(self, reader, writer):
                while line := await reader.readline():
                    self.latencies.append(time.perf_counter() - float(line))
                    if len(self.latencies) == 100:
                        self.exit(statistics.median(self.latencies) * 1000)

        median_ms = A().run()
        json.dump({"median_ms": median_ms}, open(sys.argv[1], "w"))
        """,
        tmp_path,
    )
    # Polling every 10 ms gave ~5 ms; woken by the selector it is well under a millisecond.
    assert result["median_ms"] < 2.5, result

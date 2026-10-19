"""Live JSON form: JSON objects arriving over TCP are written into input fields in real time.

``textual_app.py`` runs on Python Textual and ``rustual_app.py`` on rustual; the two files
differ only in their imports. Both take the same options::

    python rustual_app.py                 # listen on 127.0.0.1:7878 and feed itself at 60 Hz
    python rustual_app.py --rate 240      # a faster internal feed
    python rustual_app.py --no-feed       # only listen; send from elsewhere with feed.py
    python rustual_app.py --mode async    # receive with asyncio instead of a thread
    python rustual_app.py --duration 10   # quit after 10 seconds and print the stats

A worker receives newline-delimited JSON (in a thread with blocking sockets, or as an asyncio
server) and hands every object to the UI with ``post_message``, which is thread-safe. The
handler writes each value into the input whose id matches the key. Focus a field to hold its
value while you edit it.
"""

from __future__ import annotations

import argparse
import asyncio
import json
import socket
import threading
import time
from collections import deque
from typing import Any

from rustual import work
from rustual.app import App, ComposeResult
from rustual.containers import Center, Grid, Vertical
from rustual.message import Message
from rustual.widgets import Footer, Header, Input, Label, Static
from rustual.worker import Worker, get_current_worker

from feed import HOST, PORT, RATE, start_feeder

FIELDS = ["vehicle", "status", "lat", "lon", "altitude", "speed", "heading", "battery", "temperature", "seq"]


class JsonReceived(Message):
    """One JSON object from a client (``data`` is ``None`` if the line wasn't a JSON object)."""

    def __init__(self, data: dict[str, Any] | None, received: float) -> None:
        super().__init__()
        self.data = data
        self.received = received


class ClientsChanged(Message):
    """A client connected (``delta`` 1) or disconnected (-1)."""

    def __init__(self, delta: int) -> None:
        super().__init__()
        self.delta = delta


class Stats:
    """Receive rate and latency (sender's ``ts`` to the moment the values are applied)."""

    def __init__(self) -> None:
        self.reset()

    def reset(self) -> None:
        self.total = 0
        self.errors = 0
        self.latency_sum = 0.0
        self.latency_max = 0.0
        self.first: float | None = None
        self.last: float | None = None
        self.recent: deque[tuple[float, float]] = deque()  # (time, latency ms), last second

    def record(self, data: dict[str, Any]) -> None:
        now = time.time()
        ts = data.get("ts")
        latency = max(0.0, (now - ts) * 1000) if isinstance(ts, (int, float)) else 0.0
        self.total += 1
        self.latency_sum += latency
        self.latency_max = max(self.latency_max, latency)
        self.first = self.first or now
        self.last = now
        self.recent.append((now, latency))
        self.prune(now)

    def prune(self, now: float) -> None:
        while self.recent and self.recent[0][0] < now - 1.0:
            self.recent.popleft()

    def line(self, clients: int, paused: bool) -> str:
        self.prune(time.time())
        rate = len(self.recent)
        avg = sum(lat for _, lat in self.recent) / rate if rate else 0.0
        peak = max((lat for _, lat in self.recent), default=0.0)
        text = (
            f"{rate:3d} Hz · latency {avg:4.1f} ms (max {peak:4.1f}) · "
            f"{self.total} received · {self.errors} bad · {clients} client{'s' if clients != 1 else ''}"
        )
        return text + (" · PAUSED" if paused else "")

    def summary(self) -> str:
        span = (self.last - self.first) if self.first and self.last else 0.0
        rate = (self.total - 1) / span if span > 0 else 0.0
        avg = self.latency_sum / self.total if self.total else 0.0
        return (
            f"received {self.total} objects ({self.errors} bad) at {rate:.1f} Hz; "
            f"latency avg {avg:.2f} ms, max {self.latency_max:.2f} ms"
        )


class LiveForm(App[str]):
    TITLE = "Live JSON form"
    AUTO_FOCUS = None  # a focused field holds its value, so start with none focused
    CSS = """
    #main { align: center middle; }
    #form {
        grid-size: 2;
        grid-columns: 13 1fr;
        grid-gutter: 0 1;
        width: 64;
        height: auto;
        border: round $primary;
        border-title-color: $text-primary;
        padding: 0 1;
    }
    #form Label { width: 100%; text-align: right; color: $text-muted; }
    #form Input { border: none; height: 1; padding: 0 1; }
    #form Input:focus { background: $primary 30%; }
    #stats { width: 100%; height: 1; margin-top: 1; color: $text-muted; text-align: center; }
    """
    BINDINGS = [("f2", "toggle_pause", "Pause"), ("f3", "reset_stats", "Reset stats")]

    def __init__(
        self,
        host: str = HOST,
        port: int = PORT,
        mode: str = "thread",
        feed_rate: float | None = RATE,
        duration: float | None = None,
    ) -> None:
        super().__init__()
        self.host = host
        self.port = port
        self.mode = mode
        self.feed_rate = feed_rate
        self.duration = duration
        self.stats = Stats()
        self.clients = 0
        self.paused = False
        self.bound_port: int | None = None
        self._stop_feed: threading.Event | None = None

    def compose(self) -> ComposeResult:
        yield Header()
        with Vertical(id="main"):
            with Center():
                with Grid(id="form"):
                    for name in FIELDS:
                        yield Label(name)
                        yield Input(id=name)
            yield Static(id="stats")
        yield Footer()

    def on_mount(self) -> None:
        self.inputs = {name: self.query_one(f"#{name}", Input) for name in FIELDS}
        # Bind here so a busy port fails at once and port 0 (any free port) is known right away.
        server = socket.create_server((self.host, self.port))
        self.bound_port = server.getsockname()[1]
        self.query_one("#form").border_title = f"{self.mode} receiver · {self.host}:{self.bound_port}"
        if self.mode == "async":
            self.serve_async(server)
        else:
            self.serve_thread(server)
        if self.feed_rate:
            self.sub_title = f"fed at {self.feed_rate:g} Hz"
            self._stop_feed = start_feeder(self.host, self.bound_port, self.feed_rate)
        self.set_interval(0.25, self.show_stats)
        self.show_stats()
        if self.duration:
            self.set_timer(self.duration, self.finish)

    # -- receiving (worker side) -----------------------------------------------------------------

    @work(thread=True, exclusive=True, group="receiver")
    def serve_thread(self, server: socket.socket) -> None:
        """Accept clients in a thread; each client is read by a thread of its own."""
        worker = get_current_worker()
        server.settimeout(0.2)
        with server:
            while not worker.is_cancelled:
                try:
                    conn, _ = server.accept()
                except TimeoutError:
                    continue
                except OSError:
                    return
                threading.Thread(target=self.read_client, args=(conn, worker), daemon=True).start()

    def read_client(self, conn: socket.socket, worker: Worker[None]) -> None:
        conn.settimeout(None)
        self.post_message(ClientsChanged(1))
        with conn, conn.makefile("rb") as lines:
            for line in lines:
                if worker.is_cancelled:
                    break
                self.receive(line)
        self.post_message(ClientsChanged(-1))

    @work(exclusive=True, group="receiver")
    async def serve_async(self, server: socket.socket) -> None:
        """Accept and read clients with asyncio on the app's event loop."""
        async with await asyncio.start_server(self.read_client_async, sock=server) as aserver:
            await aserver.serve_forever()

    async def read_client_async(self, reader: asyncio.StreamReader, writer: asyncio.StreamWriter) -> None:
        self.post_message(ClientsChanged(1))
        try:
            while line := await reader.readline():
                self.receive(line)
        finally:
            writer.close()
            self.post_message(ClientsChanged(-1))

    def receive(self, line: bytes) -> None:
        """Parse one line and post it to the UI (called from any thread)."""
        received = time.time()
        try:
            data = json.loads(line)
        except ValueError:
            data = None
        self.post_message(JsonReceived(data if isinstance(data, dict) else None, received))

    # -- the UI (app thread) ---------------------------------------------------------------------

    def on_json_received(self, message: JsonReceived) -> None:
        if message.data is None:
            self.stats.errors += 1
            return
        self.stats.record(message.data)
        if self.paused:
            return
        for key, value in message.data.items():
            field = self.inputs.get(key)
            if field is not None and not field.has_focus:
                field.value = value if isinstance(value, str) else json.dumps(value)

    def on_clients_changed(self, message: ClientsChanged) -> None:
        self.clients += message.delta
        self.show_stats()

    def show_stats(self) -> None:
        self.query_one("#stats", Static).update(self.stats.line(self.clients, self.paused))

    def action_toggle_pause(self) -> None:
        self.paused = not self.paused
        self.show_stats()

    def action_reset_stats(self) -> None:
        self.stats.reset()
        self.show_stats()

    def finish(self) -> None:
        self.exit(self.stats.summary())

    def on_unmount(self) -> None:
        if self._stop_feed is not None:
            self._stop_feed.set()


def main() -> None:
    parser = argparse.ArgumentParser(description="Write JSON received over TCP into input fields, live.")
    parser.add_argument("--host", default=HOST)
    parser.add_argument("--port", type=int, default=PORT, help=f"port to listen on (default {PORT}; 0: any)")
    parser.add_argument("--mode", choices=["thread", "async"], default="thread", help="how to receive")
    parser.add_argument("--rate", type=float, default=RATE, help="internal feed rate in Hz (default 60)")
    parser.add_argument("--no-feed", action="store_true", help="don't feed yourself; wait for clients")
    parser.add_argument("--duration", type=float, default=None, help="quit after this many seconds")
    args = parser.parse_args()
    app = LiveForm(args.host, args.port, args.mode, None if args.no_feed else args.rate, args.duration)
    summary = app.run()
    if summary:
        print(summary)


if __name__ == "__main__":
    main()

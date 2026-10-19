"""Send newline-delimited JSON telemetry over TCP at a fixed rate (stdlib only).

Used by the live JSON form apps (which run a feeder like this one internally unless started
with ``--no-feed``), and runnable on its own to feed an app from another terminal::

    python feed.py                      # 60 Hz to 127.0.0.1:7878 until interrupted
    python feed.py --rate 240 --port 9000 --duration 10

Each line is one JSON object whose keys match the apps' input ids, plus ``seq`` and ``ts``
(the sender's ``time.time()``, so the receiver can show latency)::

    {"seq": 1, "ts": 1760000000.0, "vehicle": "rover-7", "lat": 59.913901, ...}
"""

from __future__ import annotations

import argparse
import json
import math
import socket
import threading
import time
from typing import Any

HOST = "127.0.0.1"
PORT = 7878
RATE = 60.0

STATUSES = ("nominal", "nominal", "nominal", "climbing", "descending", "low signal")


def sample(seq: int, t: float) -> dict[str, Any]:
    """One telemetry sample at time ``t`` (seconds since the feed started)."""
    return {
        "seq": seq,
        "ts": time.time(),
        "vehicle": "rover-7",
        "lat": round(59.9139 + 0.01 * math.sin(t / 7), 6),
        "lon": round(10.7522 + 0.01 * math.cos(t / 7), 6),
        "altitude": round(120 + 30 * math.sin(t / 3), 2),
        "speed": round(12 + 4 * math.sin(t * 1.3), 2),
        "heading": round((t * 24) % 360, 1),
        "battery": round(100 - (t / 6) % 100, 1),
        "temperature": round(21 + 3 * math.sin(t / 11), 2),
        "status": STATUSES[int(t / 2) % len(STATUSES)],
    }


def feed(
    host: str = HOST,
    port: int = PORT,
    rate: float = RATE,
    duration: float | None = None,
    stop: threading.Event | None = None,
    connect_timeout: float = 5.0,
) -> int:
    """Connect to ``host:port`` and send ``rate`` samples a second; returns how many were sent.

    Sends on a fixed schedule (no drift), retries the connection for ``connect_timeout``
    seconds, and stops after ``duration`` seconds, when ``stop`` is set or when the receiver
    closes the connection.
    """
    stop = stop or threading.Event()
    deadline = time.monotonic() + connect_timeout
    while True:
        try:
            sock = socket.create_connection((host, port), timeout=1.0)
            break
        except OSError:
            if time.monotonic() > deadline or stop.wait(0.05):
                return 0
    sock.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
    period = 1.0 / rate
    start = next_send = time.monotonic()
    seq = 0
    with sock:
        while not stop.is_set():
            now = time.monotonic()
            if duration is not None and now - start >= duration:
                break
            seq += 1
            line = json.dumps(sample(seq, now - start), separators=(",", ":")) + "\n"
            try:
                sock.sendall(line.encode())
            except OSError:
                break
            next_send += period
            delay = next_send - time.monotonic()
            if delay > 0:
                stop.wait(delay)
            else:
                next_send = time.monotonic()  # fell behind: don't burst to catch up
    return seq


def start_feeder(host: str, port: int, rate: float) -> threading.Event:
    """Run :func:`feed` in a daemon thread; set the returned event to stop it."""
    stop = threading.Event()
    threading.Thread(target=feed, args=(host, port, rate, None, stop), name="feeder", daemon=True).start()
    return stop


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--host", default=HOST)
    parser.add_argument("--port", type=int, default=PORT)
    parser.add_argument("--rate", type=float, default=RATE, help="samples per second (default 60)")
    parser.add_argument("--duration", type=float, default=None, help="seconds to send for (default: forever)")
    args = parser.parse_args()
    print(f"sending {args.rate:g} Hz to {args.host}:{args.port} (ctrl+c stops)")
    try:
        sent = feed(args.host, args.port, args.rate, args.duration, connect_timeout=30.0)
    except KeyboardInterrupt:
        return
    print(f"sent {sent} samples")


if __name__ == "__main__":
    main()

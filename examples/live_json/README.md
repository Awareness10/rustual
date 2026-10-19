# Live JSON form

JSON objects arrive over TCP (one per line) and are written into a form of input fields as they
come in, 60 times a second by default. The same app exists three times:

| File | Runs on |
|---|---|
| [`textual_app.py`](textual_app.py) | Python [Textual](https://github.com/Textualize/textual) 8.2.8 |
| [`rustual_app.py`](rustual_app.py) | rustual-py: the same file with only the imports swapped |
| [`../../crates/rustual/examples/live_json.rs`](../../crates/rustual/examples/live_json.rs) | rustual in Rust |

[`feed.py`](feed.py) (stdlib only) sends the data. Each app runs one internally unless started
with `--no-feed`.

```sh
# rustual-py (from crates/rustual-py, which has the package installed)
cd crates/rustual-py && uv run python ../../examples/live_json/rustual_app.py

# Python Textual
uv run --with textual==8.2.8 python examples/live_json/textual_app.py

# Rust
cargo run --release -p rustual --example live_json
```

Options (the same for all three):

| Option | |
|---|---|
| `--mode thread` | (default) a thread accepts clients; one thread per client reads lines |
| `--mode async` | an asyncio server (Python) / a tokio worker (Rust) |
| `--rate 240` | rate of the internal feed in Hz (default 60) |
| `--no-feed` | only listen; feed it from another terminal: `python examples/live_json/feed.py --rate 60` |
| `--port 9000`, `--host` | where to listen (default `127.0.0.1:7878`; port 0 picks a free one) |
| `--duration 10` | quit after 10 seconds and print the stats |

`f2` pauses the updates, `f3` resets the stats and `ctrl+q` quits. Focusing a field holds its
value while you edit it.

## How it works

```
feeder ──TCP──▶ receiver (thread or asyncio/tokio) ──post_message──▶ app thread ──▶ Input.value
```

The receiver never touches widgets: it parses each line and posts a `JsonReceived` message,
which is thread-safe and never blocks (Textual's `post_message`; `AppHandle::post_message` in
Rust). The handler on the app thread writes each value into the input whose id matches the key.
The stats line shows the receive rate and the latency from the sender's `ts` to the moment the
values are applied.

Each line is an object like:

```json
{"seq": 1, "ts": 1760000000.0, "vehicle": "rover-7", "lat": 59.913901, "lon": 10.762200,
 "altitude": 120.0, "speed": 12.0, "heading": 0.0, "battery": 100.0, "temperature": 21.0,
 "status": "nominal"}
```

Unknown keys are ignored and lines that aren't JSON objects are counted as bad.

## Numbers

5-second runs with `--duration 5 --port 0` in a terminal (Linux, one machine, `ts` to applied):

| | 60 Hz: latency (avg) · CPU | 1000 Hz: objects received of 5000 · CPU |
|---|---|---|
| Textual (thread / async) | 0.18 / 0.15 ms · 0.96 s | 4833 / 4872 · 2.6 s |
| rustual-py (thread / async) | 0.16 / 0.23 ms · 0.20 s | 5000 / 5000 · 0.42 / 0.69 s |
| Rust (thread / async) | 0.10 / 0.11 ms · 0.07 s | 5000 / 5000 · 0.11 s |

At 1000 Hz, Textual's in-process feeder falls behind because the UI holds the GIL; rustual
draws at most 60 frames a second (`TEXTUAL_FPS`, as in Textual) and handles all the messages
that arrived in between before each frame.

The Python tests are in
[`crates/rustual-py/tests/test_live_json.py`](../../crates/rustual-py/tests/test_live_json.py)
(they also check that the two Python files differ only in their imports); the Rust ones run with
`cargo test -p rustual --example live_json`.

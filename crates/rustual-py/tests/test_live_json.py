"""The live JSON form (tests/live_json), a regression test: TCP → worker → post_message → inputs."""

from __future__ import annotations

import importlib
import json
import socket
import sys
from pathlib import Path

import pytest

EXAMPLE = Path(__file__).resolve().parent / "live_json"
sys.path.insert(0, str(EXAMPLE))
live = importlib.import_module("rustual_app")
feed = importlib.import_module("feed")


def test_only_the_imports_differ_from_the_textual_version():
    def body(name: str) -> list[str]:
        lines = (EXAMPLE / name).read_text().splitlines()
        return [line for line in lines if not line.startswith(("from textual", "from rustual"))]

    assert body("textual_app.py") == body("rustual_app.py")
    imports = [
        line for line in (EXAMPLE / "rustual_app.py").read_text().splitlines() if line.startswith("from rustual")
    ]
    assert len(imports) == 6


async def wait_for(pilot, condition, timeout: float = 5.0) -> None:
    for _ in range(int(timeout / 0.01)):
        if condition():
            return
        await pilot.pause(0.01)
    raise AssertionError("timed out")


@pytest.mark.parametrize("mode", ["thread", "async"])
async def test_received_json_is_written_into_the_inputs(mode):
    app = live.LiveForm(port=0, mode=mode, feed_rate=None)
    async with app.run_test(size=(80, 24)) as pilot:
        assert app.focused is None
        port = app.bound_port
        assert port
        with socket.create_connection(("127.0.0.1", port)) as sock:
            sock.sendall((json.dumps(feed.sample(1, 0.0)) + "\n").encode())
            sock.sendall(b"not json\n")
            sock.sendall(b'{"speed": 99.5, "status": "racing", "extra": 1}\n')
            await wait_for(pilot, lambda: app.stats.total == 2 and app.stats.errors == 1)
            values = {name: field.value for name, field in app.inputs.items()}
            assert values["vehicle"] == "rover-7"
            assert values["seq"] == "1"
            assert values["speed"] == "99.5"
            assert values["status"] == "racing"
            await wait_for(pilot, lambda: app.clients == 1)
        await wait_for(pilot, lambda: app.clients == 0)
        await pilot.pause(0.3)
        text = pilot.screen_text()
        assert "2 received · 1 bad · 0 clients" in text
        assert f"{mode} receiver · 127.0.0.1:{port}" in text


async def test_focus_and_pause_hold_values():
    app = live.LiveForm(port=0, feed_rate=None)
    async with app.run_test(size=(80, 24)) as pilot:
        with socket.create_connection(("127.0.0.1", app.bound_port)) as sock:
            sock.sendall(b'{"speed": 1, "heading": 1}\n')
            await wait_for(pilot, lambda: app.stats.total == 1)
            await pilot.click("#speed")
            sock.sendall(b'{"speed": 2, "heading": 2}\n')
            await wait_for(pilot, lambda: app.stats.total == 2)
            assert (app.inputs["speed"].value, app.inputs["heading"].value) == ("1", "2")
            await pilot.press("f2")
            sock.sendall(b'{"heading": 3}\n')
            await wait_for(pilot, lambda: app.stats.total == 3)
            assert app.inputs["heading"].value == "2"
            assert "PAUSED" in pilot.screen_text()


async def test_feeds_itself():
    app = live.LiveForm(port=0, mode="async", feed_rate=200)
    async with app.run_test(size=(80, 24)) as pilot:
        await wait_for(pilot, lambda: app.stats.total >= 20)
        assert int(app.inputs["seq"].value) >= 20
        assert app.inputs["vehicle"].value == "rover-7"

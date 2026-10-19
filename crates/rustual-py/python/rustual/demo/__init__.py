"""Bundled example apps, runnable with ``uvx rustual-py`` / ``python -m rustual``."""

from __future__ import annotations

import importlib
from dataclasses import dataclass
from typing import Any


@dataclass(frozen=True)
class Demo:
    name: str
    title: str
    app: str
    description: str

    def load(self) -> Any:
        """The demo's App class."""
        module = importlib.import_module(f"{__name__}.{self.name}")
        return getattr(module, self.app)


DEMOS: list[Demo] = [
    Demo(
        "tasks",
        "Task manager",
        "TasksApp",
        "A realistic app: projects in a `ListView`, tasks in a `DataTable`, a modal form with "
        "validation (`n` new, `e` edit), a delete confirmation, a filter (`/`), palette commands "
        "and saving to JSON (`ctrl+s`).\n\n"
        "*Shows:* `ModalScreen` + `push_screen` callbacks, `Binding`s shown in the `Footer`, "
        "a command `Provider`. Runs unchanged under Textual.",
    ),
    Demo(
        "monitor",
        "System monitor",
        "MonitorApp",
        "A live (simulated) process monitor: `Digits`, `Sparkline` and `ProgressBar` cards and a "
        "process table updated every second. Sort with `c` / `m` / `n`, `d` shows details, `k` "
        "kills (with confirmation), space pauses; narrow terminals drop the cards.\n\n"
        "*Shows:* `set_interval`, `HORIZONTAL_BREAKPOINTS`, `ModalScreen`. Runs unchanged under Textual.",
    ),
    Demo(
        "chat",
        "Chat",
        "ChatApp",
        "A chat client with a simulated assistant (try *python*, *table* or *plan*): replies are "
        "`Markdown` streamed in word by word into a `layout: stream` log. Up / down recall earlier "
        "prompts, escape stops a reply, `ctrl+l` clears.\n\n"
        "*Shows:* `Markdown.append`, mounting messages, timers. Runs unchanged under Textual.",
    ),
    Demo(
        "waves",
        "Waves",
        "WavesApp",
        "Four waves streaming through `Sparkline`s at 60 frames a second, colored from the theme: "
        "`t` switches theme (or pick one with `ctrl+p`), `+` / `-` change the speed, space "
        "pauses.\n\n"
        "*Shows:* a 60 Hz `set_interval`, component classes (`.sparkline--max-color`) styled with "
        "theme variables. Runs unchanged under Textual.",
    ),
]

BY_NAME = {demo.name: demo for demo in DEMOS}

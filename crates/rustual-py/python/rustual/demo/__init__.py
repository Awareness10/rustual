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
        "stopwatch",
        "Stopwatch",
        "StopwatchApp",
        "Textual's own **stopwatch tutorial**, unchanged except for its imports.\n\n"
        "Start, stop and reset timers; `a` adds a stopwatch, `r` removes one, `d` toggles dark mode.\n\n"
        "*Shows:* `Digits`, reactive attributes, `set_interval`, CSS classes, a `.tcss` file.",
    ),
    Demo(
        "counter",
        "Counter",
        "CounterApp",
        "A counter with buttons, a key binding (`+`) and a text input that greets you.\n\n"
        "*Shows:* `reactive` + `render()`, `@on(Button.Pressed, \"#id\")`, `Input.Changed`, `Header` / `Footer`.",
    ),
    Demo(
        "forms",
        "Form controls",
        "FormsApp",
        "Every form widget: `Checkbox`, `Switch`, `RadioSet`, `Select`, `OptionList`, `SelectionList` "
        "and a `MaskedInput` (credit card number).\n\nTab between them; changes are reported in the footer.",
    ),
    Demo(
        "data",
        "Data widgets",
        "DataApp",
        "A sortable `DataTable` (press `s`), a `Tree`, a `RichLog` of your selections, plus "
        "`ProgressBar` (`p` advances it), `Sparkline` and `Digits`.",
    ),
    Demo(
        "content",
        "Tabs, Markdown & editor",
        "ContentApp",
        "`TabbedContent` with a `Markdown` document, a `TextArea` code editor with syntax "
        "highlighting, and `Collapsible`, `Rule`, `Placeholder` and `Link`.",
    ),
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
        "workers",
        "Workers",
        "WorkersApp",
        "An async worker (`asyncio.sleep`) and a thread worker updating the UI at the same time; "
        "press `b` to raise an error and see it reported after the app exits cleanly.\n\n"
        "*Shows:* `@work`, `@work(thread=True)`, calls into the UI from another thread.",
    ),
]

BY_NAME = {demo.name: demo for demo in DEMOS}

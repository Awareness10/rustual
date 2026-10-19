"""Streaming waves: four sparklines animated at 60 frames a second, new samples flowing in from
the right. Every color comes from the theme (``$primary``, ``$secondary``, ``$accent``,
``$success`` → ``$warning``), so switching themes recolors the whole app.

``t`` switches to the next theme (or pick one from the command palette, ``ctrl+p``), ``+`` / ``-``
change the speed, space pauses. The same app in Rust: ``crates/rustual/examples/waves.rs``.
Written against Textual's API: replace ``rustual`` with ``textual`` in the imports and it runs
under Textual."""

from __future__ import annotations

import math
from collections.abc import Callable

from rustual.app import App, ComposeResult
from rustual.binding import Binding
from rustual.containers import Vertical
from rustual.widgets import Footer, Header, Sparkline

FPS = 60
SPEEDS = [3, 6, 12, 24, 48, 96]  # columns per second
DEFAULT_SPEED = 2
TAU = math.tau


def _packets(x: float, t: float) -> float:
    d = x % 48 - 24
    return math.cos(TAU * x / 6) * math.exp(-d * d / 40)


#: (name, title, formula, f(x, t)): the height at position x (columns) at time t (seconds).
WAVES: list[tuple[str, str, str, Callable[[float, float], float]]] = [
    ("sine", "Sine", "sin(2πx/24)", lambda x, t: math.sin(TAU * x / 24)),
    ("beats", "Beats", "sin(2πx/11) + sin(2πx/13)", lambda x, t: math.sin(TAU * x / 11) + math.sin(TAU * x / 13)),
    ("packets", "Wave packets", "cos(2πx/6) · e^(−d²/40)", _packets),
    (
        "tide",
        "Tide",
        "sin(2πx/40) + ½sin(2πx/17 + t) + ¼sin(2πx/7)",
        lambda x, t: math.sin(TAU * x / 40) + 0.5 * math.sin(TAU * x / 17 + t) + 0.25 * math.sin(TAU * x / 7),
    ),
]


def samples(f: Callable[[float, float], float], offset: float, time: float, width: int) -> list[float]:
    """The samples shown in a sparkline ``width`` columns wide when the stream is at ``offset``."""
    return [f(offset + i, time) for i in range(width)]


def theme_cycle(names: list[str]) -> list[str]:
    """The themes ``t`` cycles through (the ANSI themes use the terminal's own colors)."""
    return sorted(name for name in names if "ansi" not in name)


class WavesApp(App):
    TITLE = "Waves"
    CSS = """
    #waves { padding: 0 1; }
    .wave { height: 1fr; padding: 0 1; border-subtitle-color: $text-muted; }
    .wave Sparkline { height: 1fr; }

    .sine { border: round $primary; border-title-color: $primary; }
    .sine Sparkline > .sparkline--min-color { color: $primary 20%; }
    .sine Sparkline > .sparkline--max-color { color: $primary; }

    .beats { border: round $secondary; border-title-color: $secondary; }
    .beats Sparkline > .sparkline--min-color { color: $secondary 20%; }
    .beats Sparkline > .sparkline--max-color { color: $secondary; }

    .packets { border: round $accent; border-title-color: $accent; }
    .packets Sparkline > .sparkline--min-color { color: $accent 20%; }
    .packets Sparkline > .sparkline--max-color { color: $accent; }

    .tide { border: round $success; border-title-color: $warning; }
    .tide Sparkline > .sparkline--min-color { color: $success; }
    .tide Sparkline > .sparkline--max-color { color: $warning; }
    """
    BINDINGS = [
        Binding("t", "next_theme", "Theme"),
        Binding("plus", "speed(1)", "Faster"),
        Binding("minus", "speed(-1)", "Slower"),
        Binding("space", "pause", "Pause"),
        Binding("ctrl+q", "quit", "Quit", priority=True),
    ]

    def __init__(self) -> None:
        super().__init__()
        self.time = 0.0  # seconds of animation (frozen while paused)
        self.offset = 0.0  # how far the stream has moved, in columns
        self.speed = DEFAULT_SPEED
        self.paused = False
        self.sparklines: dict[str, Sparkline] = {}

    def compose(self) -> ComposeResult:
        yield Header()
        with Vertical(id="waves"):
            for name, title, formula, _ in WAVES:
                panel = Vertical(Sparkline([], id=name), classes=f"wave {name}")
                panel.border_title = title
                panel.border_subtitle = formula
                yield panel
        yield Footer()

    def on_mount(self) -> None:
        self.sparklines = {name: self.query_one(f"#{name}", Sparkline) for name, *_ in WAVES}
        self.refresh_waves()
        self.set_interval(1 / FPS, self.tick)

    def tick(self) -> None:
        if not self.paused:
            self.time += 1 / FPS
            self.offset += SPEEDS[self.speed] / FPS
        self.refresh_waves()

    def refresh_waves(self) -> None:
        for name, _, _, f in WAVES:
            sparkline = self.sparklines[name]
            # Zero before the first layout: the next frame fills it.
            sparkline.data = samples(f, self.offset, self.time, sparkline.size.width)
        state = " · paused" if self.paused else ""
        self.sub_title = f"{self.theme} · {SPEEDS[self.speed]} columns/s{state}"

    def action_next_theme(self) -> None:
        names = theme_cycle(list(self.available_themes))
        current = names.index(self.theme) if self.theme in names else -1
        self.theme = names[(current + 1) % len(names)]
        self.refresh_waves()

    def action_speed(self, step: int) -> None:
        self.speed = max(0, min(len(SPEEDS) - 1, self.speed + step))
        self.refresh_waves()

    def action_pause(self) -> None:
        self.paused = not self.paused
        self.refresh_waves()


if __name__ == "__main__":
    WavesApp().run()

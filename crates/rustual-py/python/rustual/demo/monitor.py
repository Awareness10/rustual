"""A live system monitor: CPU / memory / network cards with sparklines, a process table that
updates every second (sortable, with a details dialog and "kill" with confirmation), and a
layout that drops the cards on narrow terminals (breakpoints).

The processes are simulated (deterministically), so the demo runs anywhere. Written against
Textual's API: replace ``rustual`` with ``textual`` in the imports and it runs under Textual."""

from __future__ import annotations

from dataclasses import dataclass, field

from rustual.app import App, ComposeResult
from rustual.binding import Binding
from rustual.containers import Horizontal, Vertical
from rustual.screen import ModalScreen
from rustual.widgets import (
    Button,
    DataTable,
    Digits,
    Footer,
    Header,
    Label,
    ProgressBar,
    Sparkline,
    Static,
)

HISTORY = 60
CORES = 8
MEMORY_MB = 16_384
NAMES = [
    "postgres", "nginx", "rust-analyzer", "cargo", "firefox", "code", "dockerd", "sshd",
    "redis-server", "python3", "node", "systemd", "pipewire", "Xwayland", "kworker", "rustual",
]  # fmt: skip


class Random:
    """Deterministic pseudo-random numbers in ``0.0..1.0`` (xorshift), so runs are reproducible."""

    def __init__(self, seed: int) -> None:
        self.state = seed

    def __call__(self) -> float:
        x = self.state
        x ^= (x << 13) & 0xFFFFFFFFFFFFFFFF
        x ^= x >> 7
        x ^= (x << 17) & 0xFFFFFFFFFFFFFFFF
        self.state = x
        return (x % 10_000) / 10_000


@dataclass
class Process:
    pid: int
    name: str
    cpu: float
    memory: float
    threads: int
    history: list[float] = field(default_factory=list)


def push(history: list[float], value: float) -> None:
    history.append(value)
    del history[:-HISTORY]


class Details(ModalScreen[None]):
    """Details of one process, with its CPU history."""

    DEFAULT_CSS = """
    Details { align: center middle; }
    #details { width: 60; height: auto; padding: 1 2; border: thick $accent; background: $surface; }
    #details Sparkline { height: 4; margin-top: 1; }
    """
    BINDINGS = [Binding("escape,enter", "dismiss", "Close")]

    def __init__(self, process: Process) -> None:
        super().__init__()
        self.process = process

    def compose(self) -> ComposeResult:
        p = self.process
        with Vertical(id="details"):
            yield Static(
                f"[b]{p.name}[/] (pid {p.pid})\n\n"
                f"CPU     [b]{p.cpu:.1f}%[/]\nMemory  [b]{p.memory:.0f} MB[/]\nThreads {p.threads}\n\n"
                "[$text-muted]CPU over the last minute:[/]"
            )
            yield Sparkline(p.history)


class ConfirmKill(ModalScreen[bool]):
    DEFAULT_CSS = """
    ConfirmKill { align: center middle; }
    #confirm { width: 52; height: auto; padding: 1 2; border: thick $error; background: $surface; }
    #confirm Horizontal { height: auto; align-horizontal: right; margin-top: 1; }
    #confirm Button { margin-left: 2; }
    """
    BINDINGS = [
        Binding("y", "answer(True)", "Kill"),
        Binding("n,escape", "answer(False)", "Cancel"),
    ]

    def __init__(self, process: Process) -> None:
        super().__init__()
        self.process = process

    def compose(self) -> ComposeResult:
        with Vertical(id="confirm"):
            yield Label(f"Send SIGTERM to [b]{self.process.name}[/] (pid {self.process.pid})?")
            with Horizontal():
                yield Button("Cancel", id="no")
                yield Button("Kill", variant="error", id="yes")

    def action_answer(self, yes: bool) -> None:
        self.dismiss(yes)

    def on_button_pressed(self, event: Button.Pressed) -> None:
        self.dismiss(event.button.id == "yes")


class Card(Vertical):
    """A metric: big digits and a sparkline (and optionally a bar)."""

    def __init__(self, title: str, id: str, bar: bool = False) -> None:
        super().__init__(id=id, classes="card")
        self.border_title = title
        self.bar = bar

    def compose(self) -> ComposeResult:
        yield Digits("0")
        yield Sparkline([])
        if self.bar:
            yield ProgressBar(total=MEMORY_MB, show_eta=False)

    def update(self, value: str, history: list[float], progress: float | None = None) -> None:
        self.query_one(Digits).update(value)
        self.query_one(Sparkline).data = list(history)
        if progress is not None:
            self.query_one(ProgressBar).update(progress=progress)


class MonitorApp(App):
    TITLE = "Monitor"
    CSS = """
    #cards { height: 9; }
    .card { width: 1fr; border: round $primary; padding: 0 1; }
    .card Digits { width: auto; }
    .card Sparkline { height: 2; margin-top: 1; }
    #procs { height: 1fr; border: round $secondary; }
    Screen.-narrow #cards { display: none; }
    """
    HORIZONTAL_BREAKPOINTS = [(0, "-narrow"), (90, "-wide")]
    BINDINGS = [
        Binding("c", "sort('cpu')", "CPU"),
        Binding("m", "sort('memory')", "Memory"),
        Binding("n", "sort('name')", "Name"),
        # Enter also opens details (the table's own enter binding selects the row).
        Binding("d", "details", "Details"),
        Binding("k", "kill", "Kill"),
        Binding("space", "pause", "Pause"),
        Binding("ctrl+q", "quit", "Quit", priority=True),
    ]

    def __init__(self) -> None:
        super().__init__()
        self.random = Random(0x9E3779B97F4A7C15)
        rnd = self.random
        self.processes = []
        for i in range(40):
            cpu = rnd() * 15
            self.processes.append(
                Process(300 + i * 37, NAMES[i % len(NAMES)], cpu, 20 + rnd() * 380, 1 + int(rnd() * 40), [cpu])
            )
        self.cpu: list[float] = []
        self.memory: list[float] = []
        self.network: list[float] = []
        self.sort_key = "cpu"
        self.paused = False
        for _ in range(HISTORY // 2):
            self.step()

    def compose(self) -> ComposeResult:
        yield Header(show_clock=True)
        with Horizontal(id="cards"):
            yield Card("CPU", "cpu")
            yield Card("Memory", "memory", bar=True)
            yield Card("Network", "network")
        table = DataTable(cursor_type="row", zebra_stripes=True, id="procs")
        table.border_title = "Processes"
        yield table
        yield Footer()

    def on_mount(self) -> None:
        table = self.query_one("#procs", DataTable)
        for label, key in [("PID", "pid"), ("Name", "name"), ("CPU %", "cpu"), ("Mem MB", "mem"), ("Threads", "threads")]:
            table.add_column(label, key=key)
        self.refresh_view()
        table.focus()
        self.set_interval(1, self.tick)

    def step(self) -> None:
        """Advance the simulation by one second."""
        rnd = self.random
        for p in self.processes:
            # Mostly small changes, with an occasional spike.
            spike = 35 if rnd() > 0.97 else 0
            p.cpu = min(max(p.cpu * 0.9 + (rnd() - 0.52) * 6 + spike, 0), 100)
            p.memory = min(max(p.memory + (rnd() - 0.5) * 12, 5), 1024)
            push(p.history, p.cpu)
        push(self.cpu, min(sum(p.cpu for p in self.processes) / CORES, 100))
        push(self.memory, sum(p.memory for p in self.processes))
        push(self.network, 20 + rnd() * 80)

    def tick(self) -> None:
        if not self.paused:
            self.step()
            self.refresh_view()

    @property
    def sorted_processes(self) -> list[Process]:
        if self.sort_key == "name":
            return sorted(self.processes, key=lambda p: (p.name, p.pid))
        return sorted(self.processes, key=lambda p: p.memory if self.sort_key == "memory" else p.cpu, reverse=True)

    @property
    def current(self) -> Process | None:
        table = self.query_one("#procs", DataTable)
        if not table.row_count:
            return None
        key = table.coordinate_to_cell_key(table.cursor_coordinate).row_key.value
        return next((p for p in self.processes if str(p.pid) == key), None)

    def refresh_view(self) -> None:
        self.query_one("#cpu", Card).update(f"{self.cpu[-1]:.0f}%", self.cpu)
        memory = self.memory[-1]
        self.query_one("#memory", Card).update(f"{memory / 1024:.1f}G", self.memory, min(memory, MEMORY_MB))
        self.query_one("#network", Card).update(f"{self.network[-1]:.0f}M", self.network)

        table = self.query_one("#procs", DataTable)
        # Keep the cursor on the same process while the order changes.
        selected = self.current
        table.clear()
        cursor = 0
        # DataTable cells use Rich markup (as in Textual), so named colors rather than $variables.
        for row, p in enumerate(self.sorted_processes):
            if p is selected:
                cursor = row
            cpu = f"{p.cpu:.1f}"
            if p.cpu > 50:
                cpu = f"[bold red]{cpu}[/]"
            elif p.cpu > 25:
                cpu = f"[yellow]{cpu}[/]"
            table.add_row(str(p.pid), p.name, cpu, f"{p.memory:.0f}", str(p.threads), key=str(p.pid))
        table.move_cursor(row=cursor)
        state = " · paused" if self.paused else ""
        self.sub_title = f"{len(self.processes)} processes · sorted by {self.sort_key}{state}"

    def action_sort(self, key: str) -> None:
        self.sort_key = key
        self.refresh_view()

    def action_pause(self) -> None:
        self.paused = not self.paused
        self.refresh_view()

    def action_details(self) -> None:
        if (process := self.current) is not None:
            self.push_screen(Details(process))

    def on_data_table_row_selected(self) -> None:
        self.action_details()

    def action_kill(self) -> None:
        process = self.current
        if process is None:
            return

        def done(yes: bool | None) -> None:
            if yes and process in self.processes:
                self.processes.remove(process)
                self.notify(f"Sent SIGTERM to {process.name} ({process.pid})")
                self.refresh_view()

        self.push_screen(ConfirmKill(process), done)


if __name__ == "__main__":
    MonitorApp().run()

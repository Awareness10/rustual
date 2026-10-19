#!/usr/bin/env python3
"""Regenerate the README gallery (docs/assets/screenshots/) from the examples.

Each shot runs an example in screenshot mode (``TEXTUAL_SCREENSHOT``): headless at
``TEXTUAL_SIZE``, pressing ``TEXTUAL_PRESS``, waiting ``TEXTUAL_SCREENSHOT_DELAY`` ms, in the theme
from ``TEXTUAL_THEME``. Rust examples are built in release mode first; Python ones run with the
rustual-py package from ``crates/rustual-py`` (``uv sync --reinstall-package rustual-py`` after
changing Rust code).

    python scripts/screenshots.py                # all shots
    python scripts/screenshots.py chat monitor   # only these
    python scripts/screenshots.py --list
    python scripts/screenshots.py --out /tmp/shots --png   # elsewhere, plus PNGs (rsvg-convert)
"""

from __future__ import annotations

import argparse
import os
import shutil
import subprocess
import sys
import time
from dataclasses import dataclass
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
PY = ROOT / "crates" / "rustual-py"
OUT = ROOT / "docs" / "assets" / "screenshots"


@dataclass(frozen=True)
class Shot:
    name: str
    #: ``rust:<example>`` or a Python command (run with ``uv run`` in crates/rustual-py).
    app: str
    size: str = "100x30"
    press: str = ""
    delay: int = 500
    theme: str = "textual-dark"
    args: tuple[str, ...] = ()

    def command(self) -> list[str]:
        if self.app.startswith("rust:"):
            return [str(ROOT / "target" / "release" / "examples" / self.app[5:]), *self.args]
        return ["uv", "run", "--quiet", "python", *self.app.split(), *self.args]


# Typing in screenshot mode: one key per character.
def typed(text: str) -> str:
    names = {" ": "space"}
    return ",".join(names.get(c, c) for c in text)


SHOTS = [
    # The README's first image. The tasks table has focus on start.
    Shot("py_tasks", "-m rustual.demo.tasks", size="100x28", press="down,down"),
    Shot("tasks", "rust:tasks", size="100x28", press="down,down"),
    Shot("tasks_form", "rust:tasks", size="100x34", press="e", theme="catppuccin-latte"),
    Shot("monitor", "rust:monitor", size="100x30", delay=2500, theme="nord"),
    Shot("chat", "rust:chat", size="100x30", press=typed("table") + ",enter", delay=4000),
    # The same app in two themes: every color comes from theme variables.
    Shot("waves", "rust:waves", size="100x30", delay=800, theme="tokyo-night"),
    Shot("waves_light", "rust:waves", size="100x30", delay=800, theme="solarized-light"),
]


def build_rust(shots: list[Shot]) -> None:
    examples = sorted({s.app[5:] for s in shots if s.app.startswith("rust:")})
    if not examples:
        return
    cmd = ["cargo", "build", "--release", "-p", "rustual"]
    for name in examples:
        cmd += ["--example", name]
    subprocess.run(cmd, cwd=ROOT, check=True)


def take(shot: Shot, out: Path, png: bool) -> Path:
    path = out / f"{shot.name}.svg"
    env = {
        **os.environ,
        "TEXTUAL_SCREENSHOT": str(path),
        "TEXTUAL_SIZE": shot.size,
        "TEXTUAL_PRESS": shot.press,
        "TEXTUAL_SCREENSHOT_DELAY": str(shot.delay),
        "TEXTUAL_THEME": shot.theme,
    }
    env.pop("NO_COLOR", None)
    if path.exists():
        path.unlink()
    start = time.monotonic()
    subprocess.run(
        shot.command(),
        cwd=ROOT if shot.app.startswith("rust:") else PY,
        env=env,
        check=True,
        stdin=subprocess.DEVNULL,
        stdout=subprocess.DEVNULL,
        timeout=120,
    )
    if not path.exists():
        raise SystemExit(f"error: {shot.name}: no screenshot written")
    print(
        f"{path.relative_to(ROOT) if path.is_relative_to(ROOT) else path}  "
        f"({shot.size}, {shot.theme}, {time.monotonic() - start:.1f}s)"
    )
    if png:
        subprocess.run(["rsvg-convert", "-w", "1000", str(path), "-o", str(path.with_suffix(".png"))], check=True)
    return path


def main() -> None:
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("names", nargs="*", help="shots to take (default: all)")
    p.add_argument("--out", type=Path, default=OUT, help=f"output directory (default: {OUT.relative_to(ROOT)})")
    p.add_argument("--png", action="store_true", help="also write PNGs (needs rsvg-convert)")
    p.add_argument("--list", action="store_true", help="list the shots and exit")
    args = p.parse_args()

    if args.list:
        for s in SHOTS:
            print(f"{s.name:12} {s.app:40} {s.size:7} {s.theme:12} {s.press}")
        return
    by_name = {s.name: s for s in SHOTS}
    unknown = [n for n in args.names if n not in by_name]
    if unknown:
        raise SystemExit(f"error: unknown shot(s) {', '.join(unknown)}; known: {', '.join(by_name)}")
    shots = [by_name[n] for n in args.names] if args.names else SHOTS
    if args.png and not shutil.which("rsvg-convert"):
        raise SystemExit("error: --png needs rsvg-convert")
    args.out.mkdir(parents=True, exist_ok=True)
    build_rust(shots)
    for shot in shots:
        take(shot, args.out.resolve(), args.png)


if __name__ == "__main__":
    sys.exit(main())

"""Measure how much of Textual's public Python API exists in rustual (used for features.md).

    cd crates/rustual-py && uv run --with textual==8.2.8 python ../../scripts/api_coverage.py

Counts public names only: names defined in each Textual module (not re-imported ones), and the
public attributes of each widget class beyond the ``Widget`` base. Handler hooks (``on_*``,
``watch_*``, ``validate_*``, ``compute_*``) and ``action_*`` methods are left out, since rustual
runs those natively. A name existing doesn't prove it behaves the same: this measures API
surface, the behaviour percentages in features.md stay estimates.
"""

from __future__ import annotations

import importlib
import inspect
import sys

MODULES = [
    "app", "screen", "widget", "containers", "binding", "reactive", "message", "events", "color",
    "command", "content", "geometry", "validation", "worker", "theme", "markup", "pilot",
    "css.query", "dom", "timer", "signal", "fuzzy", "highlight", "coordinate", "strip", "style",
    "suggester", "keys", "actions", "await_complete", "cache", "getters", "lazy", "design",
    "notifications", "system_commands", "types", "errors",
]
HOOKS = ("_", "on_", "watch_", "validate_", "compute_", "action_")


def public(obj: object) -> set[str]:
    return {n for n in dir(obj) if not n.startswith("_")}


def api(names: set[str]) -> set[str]:
    return {n for n in names if not n.startswith(HOOKS)}


def module_names(module: object) -> set[str]:
    """Public names defined in ``module`` itself (classes, functions, UPPER_CASE constants)."""
    source = inspect.getsource(module)
    names = set()
    for name in public(module):
        obj = getattr(module, name)
        if inspect.ismodule(obj) or name in ("annotations", "TYPE_CHECKING"):
            continue
        owner = getattr(obj, "__module__", None)
        if owner is None:
            if name.isupper() and (f"\n{name} =" in source or f"\n{name}:" in source):
                names.add(name)
        elif owner == module.__name__:
            names.add(name)
    return names


def row(label: str, have: set[str], want: set[str]) -> tuple[int, int]:
    missing = sorted(want - have)
    pct = 100 * len(want & have) / len(want) if want else 100
    shown = ", ".join(missing[:10]) + (", …" if len(missing) > 10 else "")
    print(f"| {label} | {len(want & have)}/{len(want)} | {pct:.0f}% | {shown} |")
    return len(want & have), len(want)


def total(label: str, counts: list[tuple[int, int]]) -> None:
    have, want = sum(c[0] for c in counts), sum(c[1] for c in counts)
    print(f"| **{label}** | **{have}/{want}** | **{100 * have / want:.0f}%** | |\n")


def main() -> int:
    import textual.widgets
    from textual.app import App as TApp
    from textual.screen import Screen as TScreen
    from textual.widget import Widget as TWidget

    import rustual.widgets
    from rustual.app import App as RApp
    from rustual.screen import Screen as RScreen
    from rustual.widget import Widget as RWidget

    print("| Module | Names | % | Missing |\n|---|---|---|---|")
    counts = []
    for name in MODULES:
        textual_module = importlib.import_module(f"textual.{name}")
        try:
            have = public(importlib.import_module(f"rustual.{name}"))
        except ImportError:
            have = set()
        counts.append(row(f"`textual.{name}`", have, module_names(textual_module)))
    total("Modules", counts)

    print("| Class | Names | % | Missing |\n|---|---|---|---|")
    base = public(TWidget)
    counts = [
        row("`Widget` (base)", public(RWidget), api(base)),
        row("`App`", public(RApp), api(public(TApp))),
        row("`Screen`", public(RScreen), api(public(TScreen))),
    ]
    for name in textual.widgets.__all__:
        cls = getattr(textual.widgets, name)
        if not inspect.isclass(cls):
            continue
        ours = getattr(rustual.widgets, name, None)
        counts.append(row(f"`{name}`", public(ours) if ours else set(), api(public(cls) - base)))
    total("Classes", counts)
    return 0


if __name__ == "__main__":
    sys.exit(main())

"""Render renderable scenarios with real rich and print Rust golden constants.

Each scenario is rendered with Console.render_lines(pad=True) at the given widths, as plain
text. Output: Rust `const` items for crates/rustual-render/src/renderables/golden.rs:

    uv run --no-project --with rich==15.0.0 python scripts/rich_golden.py \\
        > crates/rustual-render/src/renderables/golden.rs
    uv run --no-project --with rich==15.0.0 python scripts/rich_golden.py syntax > crates/rustual/tests/golden/syntax.rs

Syntax scenarios use the plain "text" lexer: rustual highlights with its own keyword
highlighter, so only the layout (gutter, wrapping, padding, line handling) is compared.
"""

import json

from rich import box
from rich.columns import Columns
from rich.console import Console, Group
from rich.padding import Padding
from rich.panel import Panel
from rich.syntax import Syntax
from rich.table import Table
from rich.text import Text

WORDS = (
    "alpha bravo charlie delta echo foxtrot golf hotel india juliett kilo lima mike "
    "november oscar papa quebec romeo sierra tango"
).split()


def star_wars(**kw):
    t = Table(title="Star Wars Movies", caption="Rich example table", **kw)
    t.add_column("Released", no_wrap=True)
    t.add_column("Title")
    t.add_column("Box Office", justify="right")
    t.add_row("Dec 20, 2019", "Star Wars: The Rise of Skywalker", "$952,110,690")
    t.add_row("May 25, 2018", "Solo: A Star Wars Story", "$393,151,347")
    t.add_row("Dec 15, 2017", "Star Wars Ep. V111: The Last Jedi", "$1,332,539,889", end_section=True)
    t.add_row("Dec 16, 2016", "Rogue One: A Star Wars Story", "$1,332,439,889")
    return t


def ratio_table():
    t = Table(expand=True)
    t.add_column("A", ratio=1)
    t.add_column("B", ratio=2)
    t.add_column("Fixed")
    t.add_row("one", "two", "three")
    return t


def lines_table():
    t = Table(show_lines=True)
    t.add_column("Key")
    t.add_column("Value", vertical="middle")
    t.add_row("a", "1")
    t.add_row("multi\nline", "2")
    t.add_row("c", "3")
    return t


def leading_table():
    t = Table(leading=1, box=box.SQUARE)
    t.add_column("x")
    t.add_column("y")
    t.add_row("1", "2")
    t.add_row("3", "4")
    return t


def plain_table():
    t = Table(show_header=False, box=box.ROUNDED, show_edge=False)
    t.add_column()
    t.add_column()
    t.add_row("left", "right")
    t.add_row("more text here", "x")
    return t


def footer_table():
    t = Table(show_footer=True, box=box.MINIMAL_HEAVY_HEAD)
    t.add_column("Item", footer="Total")
    t.add_column("Price", footer="$3", justify="right")
    t.add_row("apple", "$1")
    t.add_row("pear", "$2")
    return t


def grid():
    t = Table.grid(padding=(0, 2))
    t.add_column()
    t.add_column(justify="right")
    t.add_row("Name:", "rustual")
    t.add_row("Version:", "0.4")
    return t


def min_width_table():
    t = Table(min_width=40)
    t.add_column("a")
    t.add_column("b")
    t.add_row("1", "2")
    return t


def nested():
    t = Table(box=box.ASCII)
    t.add_column("Outer")
    t.add_column("Inner")
    t.add_row("cell", Panel.fit("boxed"))
    return t


def wide_cells():
    t = Table()
    t.add_column("Name", overflow="fold")
    t.add_column("Description")
    t.add_column("Code", no_wrap=True)
    t.add_row("supercalifragilistic", "a long description that will need to wrap", "abc-123")
    return t


SCENARIOS = {
    "TABLE_STAR_WARS": (star_wars, [80, 60, 40, 30]),
    "TABLE_STAR_WARS_EXPAND": (lambda: star_wars(expand=True), [90]),
    "TABLE_STAR_WARS_WIDTH": (lambda: star_wars(width=50), [80]),
    "TABLE_RATIO": (ratio_table, [50]),
    "TABLE_LINES": (lines_table, [40]),
    "TABLE_LEADING": (leading_table, [40]),
    "TABLE_PLAIN": (plain_table, [40]),
    "TABLE_FOOTER": (footer_table, [40]),
    "TABLE_GRID": (grid, [40]),
    "TABLE_MIN_WIDTH": (min_width_table, [80]),
    "TABLE_NESTED": (nested, [40]),
    "TABLE_WIDE_CELLS": (wide_cells, [40, 24]),
    "PANEL_BASIC": (lambda: Panel("Hello, World!"), [30]),
    "PANEL_FIT_TITLES": (
        lambda: Panel.fit("Hello", title="Title", subtitle="sub", subtitle_align="right"),
        [40],
    ),
    "PANEL_LONG_TITLE": (lambda: Panel.fit("x", title="a much longer title"), [40, 12]),
    "PANEL_WRAP": (
        lambda: Panel("The quick brown fox jumps over the lazy dog", box=box.DOUBLE, padding=(1, 2)),
        [24],
    ),
    "PANEL_HEIGHT": (lambda: Panel("x", height=5, title="t", title_align="left"), [12]),
    "COLUMNS_WORDS": (lambda: Columns(WORDS), [40, 25]),
    "COLUMNS_COLUMN_FIRST": (lambda: Columns(WORDS, column_first=True), [40]),
    "COLUMNS_EQUAL_EXPAND": (lambda: Columns(WORDS[:7], equal=True, expand=True), [50]),
    "COLUMNS_ALIGN_RIGHT": (lambda: Columns(WORDS[:6], equal=True, align="right"), [30]),
    "COLUMNS_WIDTH": (lambda: Columns(WORDS[:5], width=8), [30]),
    "PADDING": (lambda: Padding("Hi", (1, 2)), [10]),
    "GROUP": (lambda: Group(Panel.fit("a"), "text"), [20]),
}

CODE = "def hello(name):\n    print(f'hello {name}, this line is fairly long')\n\nhello('world')"

SYNTAX = {
    "SYNTAX_PLAIN": (lambda: Syntax(CODE, "text"), [30]),
    "SYNTAX_NUMBERS": (lambda: Syntax(CODE, "text", line_numbers=True), [40]),
    "SYNTAX_WRAP": (lambda: Syntax(CODE, "text", line_numbers=True, word_wrap=True), [32]),
    "SYNTAX_HIGHLIGHT": (
        lambda: Syntax(CODE, "text", line_numbers=True, highlight_lines={10}, start_line=9),
        [40],
    ),
    "SYNTAX_TRAILING_NEWLINE": (lambda: Syntax("a\nb\n", "text", line_numbers=True), [10]),
    "SYNTAX_TRAILING_NEWLINE_PLAIN": (lambda: Syntax("a\nb\n", "text"), [10]),
    "SYNTAX_PADDING": (lambda: Syntax("x = 1\ty", "text", padding=(1, 2)), [16]),
    "SYNTAX_DEDENT": (lambda: Syntax("    a\n      b\n    c", "text", dedent=True), [10]),
    "SYNTAX_IN_PANEL": (lambda: Panel.fit(Syntax("x = 1", "text", line_numbers=True)), [30]),
}

MEASURE = {
    "MEASURE_STAR_WARS": (star_wars, 80),
    "MEASURE_PANEL_FIT": (lambda: Panel.fit("Hello", title="Title"), 80),
    "MEASURE_TEXT": (lambda: Text("hello wonderful\nworld"), 80),
    "MEASURE_GRID": (grid, 80),
}


def main() -> None:
    import sys

    syntax = sys.argv[1:] == ["syntax"]
    console = Console(width=200, color_system=None, legacy_windows=False, force_terminal=False)
    print("// Generated by scripts/rich_golden.py from rich 15.0.0 — do not edit by hand.")
    for name, (make, widths) in (SYNTAX if syntax else SCENARIOS).items():
        for width in widths:
            options = console.options.update_width(width)
            lines = console.render_lines(make(), options, pad=True)
            plain = ["".join(seg.text for seg in line) for line in lines]
            items = "".join(f"    {json.dumps(line, ensure_ascii=False)},\n" for line in plain)
            print(f"const {name}_{width}: &[&str] = &[\n{items}];")
    if syntax:
        return
    from rich.measure import Measurement

    for name, (make, width) in MEASURE.items():
        m = Measurement.get(console, console.options.update_width(width), make())
        print(f"const {name}: (i32, i32) = ({m.minimum}, {m.maximum});")


main()

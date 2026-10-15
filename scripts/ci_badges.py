#!/usr/bin/env python3
"""Write shields.io endpoint badges (JSON) from CI test logs and coverage reports.

    python scripts/ci_badges.py --rust-log rust.log --rust-cov rust-cov.json \
        --py-log py.log --py-cov py-cov.json --out badges/

Inputs:
- ``rust.log``: output of ``cargo llvm-cov`` / ``cargo test`` ("test result: ok. N passed; ...").
- ``rust-cov.json``: ``cargo llvm-cov --json --summary-only`` report.
- ``py.log``: pytest output (last line "N passed in ...").
- ``py-cov.json``: ``pytest --cov-report=json`` report.

The README displays the files through https://img.shields.io/endpoint?url=..., so the numbers
always come from the latest CI run on master.
"""

from __future__ import annotations

import argparse
import json
import re
from pathlib import Path


def coverage_color(percent: float) -> str:
    for threshold, color in ((90, "brightgreen"), (80, "green"), (70, "yellowgreen"), (60, "yellow"), (50, "orange")):
        if percent >= threshold:
            return color
    return "red"


def rust_tests(log: str) -> tuple[int, int]:
    """(passed, failed) summed over every "test result:" line."""
    passed = failed = 0
    for m in re.finditer(r"test result: \w+\. (\d+) passed; (\d+) failed", log):
        passed += int(m.group(1))
        failed += int(m.group(2))
    return passed, failed


def python_tests(log: str) -> tuple[int, int]:
    """(passed, failed) from pytest's summary line."""
    passed = sum(int(n) for n in re.findall(r"(\d+) passed", log[-2000:]))
    failed = sum(int(n) for n in re.findall(r"(\d+) failed", log[-2000:]))
    return passed, failed


def badge(label: str, message: str, color: str, logo: str | None = None) -> dict[str, object]:
    data: dict[str, object] = {"schemaVersion": 1, "label": label, "message": message, "color": color}
    if logo:
        data["namedLogo"] = logo
        data["logoColor"] = "white"
    return data


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--rust-log", type=Path, required=True)
    parser.add_argument("--rust-cov", type=Path, required=True)
    parser.add_argument("--py-log", type=Path, required=True)
    parser.add_argument("--py-cov", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()

    rust_cov = json.loads(args.rust_cov.read_text())["data"][0]["totals"]["lines"]["percent"]
    py_cov = json.loads(args.py_cov.read_text())["totals"]["percent_covered"]
    rust_passed, rust_failed = rust_tests(args.rust_log.read_text())
    py_passed, py_failed = python_tests(args.py_log.read_text())
    if rust_passed == 0 or py_passed == 0:
        raise SystemExit(f"no test results found (rust={rust_passed}, python={py_passed})")

    def tests_badge(label: str, passed: int, failed: int, logo: str | None = None) -> dict[str, object]:
        if failed:
            return badge(label, f"{failed} failed, {passed} passed", "red", logo)
        return badge(label, f"{passed} passing", "brightgreen", logo)

    badges = {
        "rust-coverage.json": badge("rust coverage", f"{rust_cov:.0f}%", coverage_color(rust_cov), "rust"),
        "python-coverage.json": badge("python coverage", f"{py_cov:.0f}%", coverage_color(py_cov), "python"),
        "rust-tests.json": tests_badge("rust tests", rust_passed, rust_failed, "rust"),
        "python-tests.json": tests_badge("python tests", py_passed, py_failed, "python"),
        "tests.json": tests_badge("tests", rust_passed + py_passed, rust_failed + py_failed),
    }
    args.out.mkdir(parents=True, exist_ok=True)
    for name, data in badges.items():
        (args.out / name).write_text(json.dumps(data) + "\n")
        print(f"{name}: {data['message']}")


if __name__ == "__main__":
    main()

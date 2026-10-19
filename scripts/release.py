#!/usr/bin/env python3
"""Semver release flow for rustual (Rust crates + the rustual-py Python package).

Branches
    dev   development; the version is the *next* release with a dev suffix:
          Cargo ``X.Y.Z-dev.0``, Python ``X.Y.Z.dev0``.
    main  releases only; every commit on main is a tagged release ``vX.Y.Z``.

Commands (run from the repository root)
    python scripts/release.py status
        Current version, last release, branch.
    python scripts/release.py plan {patch,minor,major}
        On dev: set the planned next version from the last release
        (e.g. after v0.2.0: patch → 0.2.1.dev0, minor → 0.3.0.dev0, major → 1.0.0.dev0).
    python scripts/release.py release [--dry-run] [--no-push] [--skip-ci-check]
        On dev: release the planned version X.Y.Z:
          1. set the version to X.Y.Z, date the "Unreleased" CHANGELOG section, commit
          2. merge dev into main (--no-ff) and tag vX.Y.Z
          3. fast-forward dev to main, start X.Y.(Z+1).dev0
          4. push main, dev and the tag (the tag triggers .github/workflows/release.yml,
             which publishes the GitHub release, PyPI and crates.io)
    python scripts/release.py check
        CI: all version files agree; dev carries a dev version, main and tags a release
        version, and a tag matches the version.
    python scripts/release.py notes X.Y.Z
        Print the CHANGELOG section for a version (used for the GitHub release notes).

The workspace version in Cargo.toml is the source of truth; the script also updates the
internal crate dependency versions, crates/rustual-py/pyproject.toml, Cargo.lock and uv.lock.
"""

from __future__ import annotations

import argparse
import datetime as _dt
import json
import os
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CARGO = ROOT / "Cargo.toml"
PYPROJECT = ROOT / "crates" / "rustual-py" / "pyproject.toml"
CHANGELOG = ROOT / "CHANGELOG.md"
INTERNAL_CRATES = ("rustual-render", "rustual-css")
MAIN, DEV = "main", "dev"

SEMVER = re.compile(r"^(\d+)\.(\d+)\.(\d+)(?:-dev\.0)?$")


# ------------------------------------------------------------------------------- versions


class Version(tuple):
    """(major, minor, patch, dev)."""

    def __new__(cls, major: int, minor: int, patch: int, dev: bool = False) -> Version:
        return super().__new__(cls, (major, minor, patch, dev))

    @classmethod
    def parse(cls, text: str) -> Version:
        text = text.strip().removeprefix("v")
        m = SEMVER.match(text) or re.match(r"^(\d+)\.(\d+)\.(\d+)\.dev0$", text)
        if not m:
            raise SystemExit(f"error: not a release or dev version: {text!r}")
        return cls(int(m[1]), int(m[2]), int(m[3]), text.endswith(("-dev.0", ".dev0")))

    @property
    def release(self) -> Version:
        return Version(self[0], self[1], self[2])

    @property
    def is_dev(self) -> bool:
        return self[3]

    def bump(self, part: str) -> Version:
        major, minor, patch = self[:3]
        if part == "major":
            return Version(major + 1, 0, 0)
        if part == "minor":
            return Version(major, minor + 1, 0)
        if part == "patch":
            return Version(major, minor, patch + 1)
        raise SystemExit(f"error: unknown bump {part!r} (patch, minor or major)")

    def dev(self) -> Version:
        return Version(self[0], self[1], self[2], True)

    @property
    def cargo(self) -> str:
        return f"{self[0]}.{self[1]}.{self[2]}" + ("-dev.0" if self.is_dev else "")

    @property
    def python(self) -> str:
        return f"{self[0]}.{self[1]}.{self[2]}" + (".dev0" if self.is_dev else "")

    @property
    def tag(self) -> str:
        return f"v{self[0]}.{self[1]}.{self[2]}"

    def __str__(self) -> str:
        return self.python


def run(*cmd: str, capture: bool = False, cwd: Path = ROOT, check: bool = True) -> str:
    result = subprocess.run(cmd, cwd=cwd, text=True, capture_output=capture)
    if check and result.returncode != 0:
        if capture:
            sys.stderr.write(result.stdout + result.stderr)
        raise SystemExit(f"error: command failed: {' '.join(cmd)}")
    return result.stdout.strip() if capture else ""


def git(*args: str) -> str:
    return run("git", *args, capture=True)


def cargo_version() -> Version:
    m = re.search(r'^\[workspace\.package\]\n(?:.*\n)*?version = "([^"]+)"', CARGO.read_text(), re.M)
    if not m:
        raise SystemExit("error: no [workspace.package] version in Cargo.toml")
    return Version.parse(m[1])


def python_version() -> Version:
    m = re.search(r'^version = "([^"]+)"', PYPROJECT.read_text(), re.M)
    if not m:
        raise SystemExit(f"error: no version in {PYPROJECT}")
    return Version.parse(m[1])


def internal_dependency_versions() -> dict[str, str]:
    text = CARGO.read_text()
    out = {}
    for crate in INTERNAL_CRATES:
        m = re.search(rf'^{crate} = \{{[^}}]*version = "([^"]+)"', text, re.M)
        out[crate] = m[1] if m else "<missing>"
    return out


def last_release() -> Version:
    tags = [t for t in git("tag", "--list", "v*").split() if SEMVER.match(t[1:]) and "-" not in t]
    if not tags:
        return Version(0, 0, 0)
    return max(Version.parse(t) for t in tags)


def set_version(v: Version) -> None:
    text = CARGO.read_text()
    text = re.sub(
        r'(^\[workspace\.package\]\n(?:.*\n)*?version = ")[^"]+(")', rf"\g<1>{v.cargo}\g<2>", text, count=1, flags=re.M
    )
    for crate in INTERNAL_CRATES:
        text = re.sub(rf'(^{crate} = \{{[^}}]*version = ")[^"]+(")', rf"\g<1>{v.cargo}\g<2>", text, count=1, flags=re.M)
    CARGO.write_text(text)
    py = PYPROJECT.read_text()
    PYPROJECT.write_text(re.sub(r'^version = "[^"]+"', f'version = "{v.python}"', py, count=1, flags=re.M))
    # Lock files.
    run("cargo", "update", "--workspace", "--quiet")
    run("uv", "lock", "--quiet", cwd=PYPROJECT.parent)


# ------------------------------------------------------------------------------- changelog


def changelog_section(version: str) -> str:
    text = CHANGELOG.read_text()
    m = re.search(rf"^## \[?{re.escape(version)}\]?.*?\n(.*?)(?=^## |\Z)", text, re.M | re.S)
    return m[1].strip() if m else ""


def date_changelog(v: Version) -> None:
    text = CHANGELOG.read_text()
    if "## Unreleased" not in text:
        raise SystemExit("error: CHANGELOG.md has no '## Unreleased' section")
    unreleased = re.search(r"^## Unreleased\n(.*?)(?=^## |\Z)", text, re.M | re.S)
    if unreleased is None or not unreleased[1].strip():
        raise SystemExit("error: the 'Unreleased' section of CHANGELOG.md is empty; describe the release first")
    today = _dt.date.today().isoformat()
    text = text.replace("## Unreleased\n", f"## Unreleased\n\n## {v.python} - {today}\n", 1)
    CHANGELOG.write_text(text)


# ------------------------------------------------------------------------------- commands


def cmd_status(_: argparse.Namespace) -> None:
    v = cargo_version()
    print(f"branch:        {git('branch', '--show-current') or '(detached)'}")
    print(f"version:       {v.python}  (Cargo {v.cargo}, pyproject {python_version().python})")
    print(f"last release:  {last_release().tag}")
    if v.is_dev:
        print(f"next release:  {v.release.tag}   (python scripts/release.py release)")


def require_branch(name: str) -> None:
    current = git("branch", "--show-current")
    if current != name:
        raise SystemExit(f"error: run this on the {name!r} branch (currently on {current!r})")


def require_clean() -> None:
    if git("status", "--porcelain", "--untracked-files=no"):
        raise SystemExit("error: the working tree has uncommitted changes")


def cmd_plan(args: argparse.Namespace) -> None:
    require_branch(DEV)
    require_clean()
    planned = last_release().bump(args.part).dev()
    current = cargo_version()
    if planned == current:
        print(f"already planning {planned.python}")
        return
    set_version(planned)
    run("git", "commit", "-qam", f"Plan {planned.release.python} ({args.part} release)")
    print(f"dev now builds {planned.python} (Cargo {planned.cargo}); next release: {planned.release.tag}")


def ci_succeeded(sha: str) -> bool | None:
    """Result of the latest CI run for `sha` (None if gh isn't available or no run exists)."""
    try:
        out = run(
            "gh",
            "run",
            "list",
            "--workflow",
            "ci.yml",
            "--commit",
            sha,
            "--limit",
            "1",
            "--json",
            "status,conclusion",
            capture=True,
            check=False,
        )
        runs = json.loads(out or "[]")
    except (OSError, json.JSONDecodeError):
        return None
    if not runs:
        return None
    return runs[0]["status"] == "completed" and runs[0]["conclusion"] == "success"


def cmd_release(args: argparse.Namespace) -> None:
    require_branch(DEV)
    require_clean()
    current = cargo_version()
    if not current.is_dev:
        raise SystemExit(f"error: dev should carry a .dev0 version, found {current.python}")
    if python_version() != current:
        raise SystemExit("error: Cargo.toml and pyproject.toml versions differ; run `release.py check`")
    version = current.release
    if version <= last_release():
        raise SystemExit(f"error: {version.tag} isn't newer than the last release {last_release().tag}")
    if git("tag", "--list", version.tag):
        raise SystemExit(f"error: tag {version.tag} already exists")

    run("git", "fetch", "--quiet", "origin")
    head = git("rev-parse", "HEAD")
    if git("rev-parse", f"origin/{DEV}") != head:
        raise SystemExit(f"error: {DEV} isn't in sync with origin/{DEV}; push or pull first")
    if not args.skip_ci_check:
        ok = ci_succeeded(head)
        if ok is not True:
            state = "hasn't finished or doesn't exist" if ok is None else "failed"
            raise SystemExit(f"error: CI for {head[:8]} {state} (use --skip-ci-check to override)")

    next_dev = version.bump("patch").dev()
    print(f"Releasing {version.tag}; dev continues with {next_dev.python}")
    if args.dry_run:
        print("(dry run: nothing changed)")
        return

    set_version(version)
    date_changelog(version)
    run("git", "commit", "-qam", f"Release {version.tag}")
    run("git", "checkout", "-q", MAIN)
    run("git", "merge", "--quiet", "--ff-only", f"origin/{MAIN}")
    run("git", "merge", "--quiet", "--no-ff", DEV, "-m", f"Merge {DEV} for {version.tag}")
    run("git", "tag", "-a", version.tag, "-m", f"rustual {version.python}")
    run("git", "checkout", "-q", DEV)
    run("git", "merge", "--quiet", "--ff-only", MAIN)
    set_version(next_dev)
    run("git", "commit", "-qam", f"Start {next_dev.release.python} development")
    if args.no_push:
        print(f"Done locally. Push with: git push --atomic origin {MAIN} {DEV} {version.tag}")
        return
    run("git", "push", "--quiet", "--atomic", "origin", MAIN, DEV, version.tag)
    print(f"Pushed {version.tag}: the Release workflow now builds and publishes it.")


def cmd_check(_: argparse.Namespace) -> None:
    v = cargo_version()
    problems = []
    if python_version() != v:
        problems.append(f"pyproject.toml has {python_version().python}, Cargo.toml has {v.python}")
    for crate, dep in internal_dependency_versions().items():
        if dep != v.cargo:
            problems.append(f"workspace dependency {crate} = {dep!r}, expected {v.cargo!r}")
    ref = os.environ.get("GITHUB_REF", "")
    branch = os.environ.get("GITHUB_HEAD_REF") or (
        ref.removeprefix("refs/heads/") if ref.startswith("refs/heads/") else ""
    )
    if ref.startswith("refs/tags/"):
        tag = ref.removeprefix("refs/tags/")
        if tag != v.tag or v.is_dev:
            problems.append(f"tag {tag} doesn't match the version {v.python}")
    elif branch == MAIN and v.is_dev:
        problems.append(f"{MAIN} must carry a release version, found {v.python}")
    elif branch == DEV and not v.is_dev:
        problems.append(f"{DEV} must carry a .dev0 version, found {v.python}")
    if problems:
        raise SystemExit("version check failed:\n  " + "\n  ".join(problems))
    print(f"version check ok: {v.python} (Cargo {v.cargo})")


def cmd_notes(args: argparse.Namespace) -> None:
    print(changelog_section(Version.parse(args.version).python))


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = parser.add_subparsers(dest="command", required=True)
    sub.add_parser("status").set_defaults(func=cmd_status)
    p = sub.add_parser("plan")
    p.add_argument("part", choices=["patch", "minor", "major"])
    p.set_defaults(func=cmd_plan)
    p = sub.add_parser("release")
    p.add_argument("--dry-run", action="store_true")
    p.add_argument("--no-push", action="store_true")
    p.add_argument("--skip-ci-check", action="store_true")
    p.set_defaults(func=cmd_release)
    sub.add_parser("check").set_defaults(func=cmd_check)
    p = sub.add_parser("notes")
    p.add_argument("version")
    p.set_defaults(func=cmd_notes)
    args = parser.parse_args()
    args.func(args)


if __name__ == "__main__":
    main()

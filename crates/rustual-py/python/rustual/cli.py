"""``rustual-py`` / ``python -m rustual``: run the bundled examples.

rustual-py              open the launcher
rustual-py tasks        run one example
rustual-py --list       list the examples
"""

from __future__ import annotations

import argparse
import sys

from . import __version__
from .demo import BY_NAME, DEMOS


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        prog="rustual-py",
        description="Run the example apps bundled with rustual (Textual's API on a Rust engine).",
    )
    parser.add_argument("example", nargs="?", choices=sorted(BY_NAME), metavar="EXAMPLE", help="example to run")
    parser.add_argument("--list", action="store_true", help="list the examples and exit")
    parser.add_argument("--version", action="version", version=f"rustual-py {__version__}")
    args = parser.parse_args(argv)

    if args.list:
        width = max(len(d.name) for d in DEMOS)
        for demo in DEMOS:
            print(f"{demo.name:<{width}}  {demo.title}")
        return 0
    if args.example:
        BY_NAME[args.example].load()().run()
        return 0

    from ._bridge import _native
    from .demo.launcher import Launcher

    # Stay on the alternate screen while switching between the launcher and the examples, so
    # the shell doesn't flash up in between.
    _native.keep_terminal(True)
    try:
        while True:
            choice = Launcher().run()
            if not choice:
                break
            BY_NAME[choice].load()().run()
    finally:
        _native.keep_terminal(False)
    print("rustual: https://github.com/Awareness10/rustual")
    return 0


if __name__ == "__main__":
    sys.exit(main())

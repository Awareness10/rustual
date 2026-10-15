"""``python -m rustual``: the example launcher (like ``python -m textual``)."""

import sys

from .cli import main

sys.exit(main())

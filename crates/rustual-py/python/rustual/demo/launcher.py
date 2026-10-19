"""The ``rustual-py`` launcher: pick a bundled example and run it."""

from __future__ import annotations

from ..app import App, ComposeResult
from ..containers import Horizontal, Vertical
from ..widgets import Footer, Header, Markdown, OptionList
from ..widgets.option_list import Option
from . import BY_NAME, DEMOS

INTRO = """\
# rustual

Textual's Python API, running on a Rust engine.

Pick an example on the left and press **enter** to run it; quit it (`ctrl+q`) to come back here.
"""


class Launcher(App[str]):
    """Lists the bundled demos; exits with the name of the one to run."""

    TITLE = "rustual"
    SUB_TITLE = "examples"
    CSS = """
    #demos { width: 34; height: 1fr; border: round $accent; border-title-color: $accent; }
    #about { width: 1fr; height: 1fr; padding: 0 2; }
    """
    BINDINGS = [("q", "quit", "Quit")]

    def compose(self) -> ComposeResult:
        yield Header()
        with Horizontal():
            demos = OptionList(*[Option(demo.title, id=demo.name) for demo in DEMOS], id="demos")
            demos.border_title = "Examples"
            yield demos
            with Vertical(id="about"):
                yield Markdown(INTRO, id="description")
        yield Footer()

    def on_option_list_option_highlighted(self, event: OptionList.OptionHighlighted) -> None:
        demo = BY_NAME.get(event.option_id or "")
        if demo is not None:
            self.query_one("#description", Markdown).update(
                f"{INTRO}\n## {demo.title}\n\n{demo.description}\n\nRun it directly with `rustual-py {demo.name}`."
            )

    def on_option_list_option_selected(self, event: OptionList.OptionSelected) -> None:
        self.exit(event.option_id)

"""Form controls: Checkbox, Switch, RadioSet, Select, OptionList, SelectionList, MaskedInput."""

from textual_rs import on
from textual_rs.app import App, ComposeResult
from textual_rs.containers import Horizontal, Vertical, VerticalScroll
from textual_rs.widgets import (
    Checkbox,
    Footer,
    Header,
    Input,
    Label,
    MaskedInput,
    OptionList,
    RadioButton,
    RadioSet,
    Select,
    SelectionList,
    Switch,
)
from textual_rs.widgets.option_list import Option
from textual_rs.widgets.selection_list import Selection

LANGUAGES = ["Python", "Rust", "Go", "Haskell", "OCaml"]


class FormsApp(App):
    CSS = """
    #left, #right { width: 1fr; height: 1fr; padding: 0 1; }
    .row { height: auto; }
    .row Label { padding: 1 1 0 0; }
    OptionList { height: 7; }
    SelectionList { height: 6; }
    #status { width: 1fr; height: 1; background: $boost; padding: 0 1; }
    """

    def compose(self) -> ComposeResult:
        yield Header()
        with Horizontal():
            with VerticalScroll(id="left"):
                yield Checkbox("Subscribe to newsletter", True, id="news")
                with Horizontal(classes="row"):
                    yield Label("Dark mode")
                    yield Switch(True, id="dark")
                with RadioSet(id="size"):
                    yield RadioButton("Small")
                    yield RadioButton("Medium", value=True)
                    yield RadioButton("Large")
                yield Select.from_values(LANGUAGES, prompt="Favourite language", id="lang")
            with Vertical(id="right"):
                yield OptionList(
                    Option("New file", id="new"),
                    Option("Open…", id="open"),
                    None,
                    Option("Save", id="save"),
                    Option("Quit", id="quit", disabled=True),
                    id="menu",
                )
                yield SelectionList(
                    Selection("Tests", "tests", True),
                    ("Docs", "docs"),
                    ("Benchmarks", "bench", True),
                    id="tasks",
                )
                yield MaskedInput("9999-9999-9999-9999;0", id="card")
        yield Label("Ready", id="status")
        yield Footer()

    def status(self, text: str) -> None:
        self.query_one("#status", Label).update(text)

    def on_checkbox_changed(self, event: Checkbox.Changed) -> None:
        self.status(f"newsletter: {event.value}")

    def on_switch_changed(self, event: Switch.Changed) -> None:
        self.theme = "textual-dark" if event.value else "textual-light"

    def on_radio_set_changed(self, event: RadioSet.Changed) -> None:
        self.status(f"size: {event.pressed.label} (#{event.index})")

    def on_select_changed(self, event: Select.Changed) -> None:
        self.status(f"language: {event.value}")

    @on(OptionList.OptionSelected, "#menu")
    def menu_selected(self, event: OptionList.OptionSelected) -> None:
        self.status(f"menu: {event.option_id}")

    def on_selection_list_selected_changed(self, event: SelectionList.SelectedChanged) -> None:
        self.status(f"tasks: {', '.join(event.selection_list.selected)}")

    @on(Input.Changed, "#card")
    def card_changed(self, event: Input.Changed) -> None:
        self.status(f"card: {event.value}")


if __name__ == "__main__":
    FormsApp().run()

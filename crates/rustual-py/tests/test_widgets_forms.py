"""Form widgets: ToggleButton, Checkbox, RadioButton, RadioSet, Switch, OptionList, Select,
SelectionList, MaskedInput."""

import pytest

from rustual import on
from rustual.app import App, ComposeResult
from rustual.widgets import (
    Checkbox,
    Input,
    MaskedInput,
    OptionList,
    RadioButton,
    RadioSet,
    Select,
    SelectionList,
    Static,
    Switch,
    ToggleButton,
)
from rustual.widgets.option_list import DuplicateID, Option, OptionDoesNotExist
from rustual.widgets.select import NULL, EmptySelectError, InvalidSelectValueError
from rustual.widgets.selection_list import Selection, SelectionError


# ------------------------------------------------------------------------------ toggles


async def test_checkbox_keys_clicks_and_messages():
    events = []

    class MyApp(App):
        def compose(self) -> ComposeResult:
            yield Checkbox("Agree", id="agree")
            yield Checkbox("Pre", True, id="pre")

        def on_checkbox_changed(self, event: Checkbox.Changed) -> None:
            events.append(("handler", event.checkbox.id, event.value, event.control is event.checkbox))

        @on(Checkbox.Changed, "#pre")
        def pre_changed(self, event: Checkbox.Changed) -> None:
            events.append(("on", event.value))

    app = MyApp()
    async with app.run_test() as pilot:
        agree = app.query_one("#agree", Checkbox)
        pre = app.query_one("#pre", Checkbox)
        assert agree.value is False and pre.value is True
        assert pre.has_class("-on") and not agree.has_class("-on")
        assert isinstance(agree, ToggleButton) and isinstance(agree, Static)
        assert agree.label == "Agree"
        assert "Agree" in pilot.screen_text()
        # No message for the initial value.
        assert events == []

        agree.focus()
        await pilot.press("space")
        assert agree.value is True
        assert events == [("handler", "agree", True, True)]
        await pilot.press("enter")
        assert agree.value is False

        await pilot.click("#pre")
        assert pre.value is False
        assert ("on", False) in events and ("handler", "pre", False, True) in events

        events.clear()
        agree.value = True
        await pilot.pause()
        assert events == [("handler", "agree", True, True)]
        agree.value = True  # unchanged: no message
        await pilot.pause()
        assert len(events) == 1

        agree.toggle()
        assert agree.value is False
        agree.label = "Accept [b]terms[/b]"
        await pilot.pause()
        assert "Accept terms" in pilot.screen_text()


async def test_toggle_button_compact_and_subclass_handler():
    seen = []

    class MyCheck(Checkbox):
        def on_checkbox_changed(self, event: Checkbox.Changed) -> None:
            seen.append(("self", event.value))

    class MyApp(App):
        def compose(self) -> ComposeResult:
            yield MyCheck("Mine", compact=True)
            yield ToggleButton("Plain", id="plain")

        def on_checkbox_changed(self, event: Checkbox.Changed) -> None:
            seen.append(("app", type(event.checkbox).__name__))

        def on_toggle_button_changed(self, event: ToggleButton.Changed) -> None:
            seen.append(("toggle", event.toggle_button.id, event.value))

    app = MyApp()
    async with app.run_test() as pilot:
        mine = app.query_one(MyCheck)
        assert mine.compact is True and mine.has_class("-textual-compact")
        mine.compact = False
        assert not mine.has_class("-textual-compact")
        await pilot.click(MyCheck)
        assert seen == [("self", True), ("app", "MyCheck")]
        app.query_one("#plain", ToggleButton).toggle()
        await pilot.pause()
        assert seen[-1] == ("toggle", "plain", True)


async def test_switch():
    events = []

    class MyApp(App):
        def compose(self) -> ComposeResult:
            yield Switch(id="s1", animate=False)
            yield Switch(True, id="s2")

        def on_switch_changed(self, event: Switch.Changed) -> None:
            events.append((event.switch.id, event.value, event.control is event.switch))

    app = MyApp()
    async with app.run_test() as pilot:
        s1 = app.query_one("#s1", Switch)
        s2 = app.query_one("#s2", Switch)
        assert s1.value is False and s2.value is True
        await pilot.click("#s1")
        assert s1.value is True
        assert events == [("s1", True, True)]
        s1.focus()
        await pilot.press("enter")
        assert s1.value is False
        s2.toggle()
        await pilot.pause()
        assert s2.value is False
        assert events[-1] == ("s2", False, True)
        s2.value = True
        await pilot.pause()
        assert events[-1] == ("s2", True, True)


# ------------------------------------------------------------------------------ RadioSet


async def test_radio_set():
    events = []

    class MyApp(App):
        def compose(self) -> ComposeResult:
            yield RadioSet("Red", "Green", RadioButton("Blue", id="blue"), id="colors")
            with RadioSet(id="sizes"):
                yield RadioButton("S")
                yield RadioButton("M", value=True)
                yield RadioButton("L")

        def on_radio_set_changed(self, event: RadioSet.Changed) -> None:
            events.append((event.radio_set.id, event.index, str(event.pressed.label), event.control is event.radio_set))

        @on(RadioSet.Changed, pressed="#blue")
        def blue(self, event: RadioSet.Changed) -> None:
            events.append("blue!")

    app = MyApp()
    async with app.run_test(size=(60, 24)) as pilot:
        colors = app.query_one("#colors", RadioSet)
        sizes = app.query_one("#sizes", RadioSet)
        assert colors.pressed_index == -1 and colors.pressed_button is None
        assert sizes.pressed_index == 1
        assert sizes.pressed_button.label == "M"
        assert [b.label for b in colors.query(RadioButton)] == ["Red", "Green", "Blue"]
        assert colors.can_focus and not colors.query_one(RadioButton).can_focus

        colors.focus()
        await pilot.press("down", "enter")
        assert colors.pressed_index == 1
        assert events == [("colors", 1, "Green", True)]
        await pilot.press("down", "space")
        assert colors.pressed_index == 2
        assert colors.pressed_button.id == "blue"
        assert "blue!" in events
        assert [b.value for b in colors.query(RadioButton)] == [False, False, True]

        await pilot.click(RadioButton)  # first button of the app: "Red"
        assert colors.pressed_index == 0
        assert app.focused is colors

        # Pressing a button programmatically switches the set.
        list(sizes.query(RadioButton))[2].value = True
        await pilot.pause()
        assert sizes.pressed_index == 2
        assert events[-1] == ("sizes", 2, "L", True)
        assert [b.value for b in sizes.query(RadioButton)] == [False, False, True]


# ------------------------------------------------------------------------------ OptionList


async def test_option_list():
    events = []

    class MyApp(App):
        def compose(self) -> ComposeResult:
            yield OptionList(
                "Alpha",
                Option("Beta", id="beta"),
                None,
                Option("Gamma", id="gamma", disabled=True),
                "Delta",
                id="ol",
            )

        def on_option_list_option_highlighted(self, event: OptionList.OptionHighlighted) -> None:
            events.append(("hl", event.option_index, event.option_id, event.option.prompt))

        @on(OptionList.OptionSelected, "#ol")
        def selected(self, event: OptionList.OptionSelected) -> None:
            events.append(("sel", event.option_index, event.option_id, event.option_list is event.control))

    app = MyApp()
    async with app.run_test() as pilot:
        ol = app.query_one(OptionList)
        assert ol.option_count == 4
        assert ol.highlighted == 0
        assert ol.highlighted_option.prompt == "Alpha"
        assert ol.get_option("beta").prompt == "Beta"
        assert ol.get_option_index("gamma") == 2
        with pytest.raises(OptionDoesNotExist):
            ol.get_option("nope")
        with pytest.raises(DuplicateID):
            ol.add_option(Option("x", id="beta"))
        text = pilot.screen_text()
        assert "Alpha" in text and "Delta" in text

        ol.focus()
        await pilot.press("down")
        assert ol.highlighted == 1
        assert events[-1] == ("hl", 1, "beta", "Beta")
        await pilot.press("down")  # skips disabled Gamma
        assert ol.highlighted == 3
        await pilot.press("enter")
        assert events[-1] == ("sel", 3, None, True)
        selected_option = None

        ol.enable_option("gamma")
        ol.highlighted = 2
        await pilot.pause()
        assert events[-1] == ("hl", 2, "gamma", "Gamma")

        ol.add_option(Option("Epsilon", id="eps"))
        assert ol.option_count == 5
        await pilot.pause()
        assert "Epsilon" in pilot.screen_text()
        ol.replace_option_prompt("eps", "Zeta")
        await pilot.pause()
        assert "Zeta" in pilot.screen_text() and ol.get_option("eps").prompt == "Zeta"
        ol.remove_option("beta")
        assert ol.option_count == 4
        assert [o.prompt for o in ol.options] == ["Alpha", "Gamma", "Delta", "Zeta"]

        ol.set_options(["One", "Two"])
        await pilot.pause()
        assert ol.option_count == 2
        assert "One" in pilot.screen_text() and "Alpha" not in pilot.screen_text()
        ol.clear_options()
        assert ol.option_count == 0 and ol.highlighted is None
        _ = selected_option


async def test_option_list_click_and_subclass():
    picked = []

    class Menu(OptionList):
        def on_option_list_option_selected(self, event: OptionList.OptionSelected) -> None:
            picked.append(event.option.id)

    class MyApp(App):
        def compose(self) -> ComposeResult:
            yield Menu(Option("Open", id="open"), Option("Save", id="save"))

    app = MyApp()
    async with app.run_test() as pilot:
        menu = app.query_one(Menu)
        # Border + padding: the first option is on row 1.
        await pilot.click(Menu, offset=(3, 2))
        assert picked == ["save"]
        menu.action_first()
        menu.action_select()
        await pilot.pause()
        assert picked == ["save", "open"]


# ------------------------------------------------------------------------------ Select


async def test_select():
    events = []

    class MyApp(App):
        def compose(self) -> ComposeResult:
            yield Select([("One", 1), ("Two", 2), ("Three", 3)], prompt="Pick", id="num")
            yield Select.from_values(["a", "b"], allow_blank=False, id="letters")

        def on_select_changed(self, event: Select.Changed) -> None:
            events.append((event.select.id, event.value, event.control is event.select))

    app = MyApp()
    async with app.run_test(size=(60, 24)) as pilot:
        num = app.query_one("#num", Select)
        letters = app.query_one("#letters", Select)
        assert num.value is NULL and num.is_blank() and num.selection is None
        assert letters.value == "a"
        assert "Pick" in pilot.screen_text()
        assert ("letters", "a", True) in events
        events.clear()

        num.focus()
        await pilot.press("enter")
        assert num.expanded
        await pilot.press("down", "down", "enter")
        assert not num.expanded
        assert num.value == 2
        assert events == [("num", 2, True)]
        assert "Two" in pilot.screen_text()
        assert app.focused is num

        num.value = 3
        await pilot.pause()
        assert events[-1] == ("num", 3, True)
        with pytest.raises(InvalidSelectValueError):
            num.value = 99
        num.clear()
        await pilot.pause()
        assert num.is_blank()
        assert events[-1][1] is NULL
        with pytest.raises(InvalidSelectValueError):
            letters.clear()

        num.prompt = "Choose"
        await pilot.pause()
        assert "Choose" in pilot.screen_text()

        num.set_options([("X", "x"), ("Y", "y")])
        num.value = "y"
        await pilot.pause()
        assert num.value == "y" and "Y" in pilot.screen_text()

        # Opening with a click and escaping keeps the value.
        await pilot.click("#num")
        assert num.expanded
        await pilot.press("escape")
        assert not num.expanded and num.value == "y"


async def test_select_errors_and_objects():
    with pytest.raises(EmptySelectError):
        Select([], allow_blank=False)
    with pytest.raises(InvalidSelectValueError):
        Select([("a", 1)], value=2)

    class Thing:
        def __init__(self, n: int) -> None:
            self.n = n

    things = [Thing(1), Thing(2)]

    class MyApp(App):
        def compose(self) -> ComposeResult:
            yield Select([("first", things[0]), ("second", things[1])], value=things[1])

    app = MyApp()
    async with app.run_test():
        assert app.query_one(Select).value is things[1]


# ------------------------------------------------------------------------------ SelectionList


async def test_selection_list():
    events = []

    class MyApp(App):
        def compose(self) -> ComposeResult:
            yield SelectionList(
                ("Falken's Maze", "maze", True),
                ("Black Jack", "jack"),
                Selection("Gin Rummy", "gin", id="gin"),
                id="games",
            )

        def on_selection_list_selected_changed(self, event: SelectionList.SelectedChanged) -> None:
            events.append(("changed", list(event.selection_list.selected), event.control.id))

        def on_selection_list_selection_toggled(self, event: SelectionList.SelectionToggled) -> None:
            events.append(("toggled", event.selection_index, event.selection.value))

        @on(SelectionList.SelectionHighlighted)
        def hl(self, event: SelectionList.SelectionHighlighted) -> None:
            events.append(("hl", event.selection.value))

    app = MyApp()
    async with app.run_test() as pilot:
        sl = app.query_one(SelectionList)
        assert isinstance(sl, OptionList)
        assert sl.selected == ["maze"]
        assert sl.option_count == 3
        assert sl.get_option("gin").value == "gin"
        assert "Black Jack" in pilot.screen_text()

        sl.focus()
        await pilot.press("down")
        assert sl.highlighted == 1
        assert events[-1] == ("hl", "jack")
        await pilot.press("space")
        assert sl.selected == ["maze", "jack"]
        assert ("changed", ["maze", "jack"], "games") in events
        assert ("toggled", 1, "jack") in events

        events.clear()
        sl.deselect("maze")
        await pilot.pause()
        assert sl.selected == ["jack"]
        assert events == [("changed", ["jack"], "games")]
        sl.select(sl.get_option("gin"))
        sl.toggle("jack")
        await pilot.pause()
        assert sl.selected == ["gin"]
        sl.select_all()
        assert set(sl.selected) == {"maze", "jack", "gin"}
        sl.deselect_all()
        assert sl.selected == []
        sl.toggle_all()
        assert set(sl.selected) == {"maze", "jack", "gin"}

        sl.add_option(("Chess", "chess"))
        assert sl.option_count == 4
        with pytest.raises(SelectionError):
            sl.add_option(None)
        sl.remove_option("gin")
        assert sl.option_count == 3 and "gin" not in sl.selected
        sl.clear_options()
        assert sl.option_count == 0 and sl.selected == []


async def test_selection_list_pre_mount_state():
    sl = SelectionList(("a", 1), ("b", 2))
    sl.select(2)
    assert sl.selected == [2]

    class MyApp(App):
        def compose(self) -> ComposeResult:
            yield sl

    app = MyApp()
    async with app.run_test():
        assert sl.selected == [2]


# ------------------------------------------------------------------------------ MaskedInput


async def test_masked_input():
    events = []

    class MyApp(App):
        def compose(self) -> ComposeResult:
            yield MaskedInput("9999-9999", id="card")

        def on_input_changed(self, event: Input.Changed) -> None:
            events.append(("changed", event.input.id, event.value))

        @on(Input.Submitted, "#card")
        def submitted(self, event: Input.Submitted) -> None:
            events.append(("submitted", event.value))

    app = MyApp()
    async with app.run_test() as pilot:
        m = app.query_one(MaskedInput)
        assert isinstance(m, Input)
        assert m.template == "9999-9999"
        m.focus()
        await pilot.press("1", "2", "x", "3", "4", "5")
        assert m.value == "1234-5"
        assert events[-1] == ("changed", "card", "1234-5")
        assert not m.validate().is_valid
        assert m.has_class("-invalid")
        await pilot.press("6", "7", "8")
        assert m.value == "1234-5678"
        assert m.is_valid and m.has_class("-valid")
        await pilot.press("enter")
        assert events[-1] == ("submitted", "1234-5678")
        await pilot.press("backspace")
        assert m.value == "1234-567"

        m.value = "4321-8765"
        assert m.value == "4321-8765"
        with pytest.raises(ValueError):
            m.value = "abcd"
        m.clear()
        assert m.value == ""
        m.template = ">AAA"
        m.focus()
        await pilot.press("a", "b", "c")
        assert m.value == "ABC"

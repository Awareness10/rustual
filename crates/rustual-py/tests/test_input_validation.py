"""Input / MaskedInput validation in apps. These are scenarios run with Textual 8.2.8; the
expected classes, messages and results are what Textual produced for the same code."""

from __future__ import annotations

from rustual.app import App, ComposeResult
from rustual.validation import Failure, Function, Integer, Length, Number, ValidationResult, Validator
from rustual.widgets import Button, Input, MaskedInput

import pytest


def descriptions(result: ValidationResult | None) -> list[str] | None:
    return None if result is None else result.failure_descriptions


class LogApp(App):
    def __init__(self, *widgets) -> None:
        super().__init__()
        self.widgets = widgets
        self.log_: list[tuple] = []

    def compose(self) -> ComposeResult:
        yield from self.widgets

    def on_input_changed(self, e: Input.Changed) -> None:
        self.log_.append(("changed", e.input.id, e.value, descriptions(e.validation_result)))

    def on_input_submitted(self, e: Input.Submitted) -> None:
        self.log_.append(("submitted", e.input.id, e.value, descriptions(e.validation_result)))

    def on_input_blurred(self, e: Input.Blurred) -> None:
        self.log_.append(("blurred", e.input.id, e.value, descriptions(e.validation_result)))


def validity(widget) -> list[str]:
    return sorted(c for c in widget.classes if c in ("-valid", "-invalid"))


async def test_input_scenario_matches_textual() -> None:
    app = LogApp(
        Input(value="abc", validators=[Number()], id="a"),
        Input(value="5", type="integer", id="b"),
        Input(id="c", validators=[Length(minimum=2)], validate_on=["submitted"]),
        Input(id="d", validators=[Length(minimum=2)], valid_empty=True),
        Input(value="x", id="e"),
        Button("b"),
    )
    async with app.run_test() as pilot:
        await pilot.pause()
        state = {i.id: (validity(i), i.is_valid) for i in app.query(Input)}
        assert state == {
            "a": (["-invalid"], False),
            "b": (["-valid"], True),
            "c": ([], True),
            "d": ([], True),
            "e": (["-valid"], True),
        }
        assert app.log_ == [
            ("changed", "a", "abc", ["Must be a valid number."]),
            ("changed", "b", "5", []),
            ("changed", "e", "x", None),
        ]
        app.log_.clear()

        # The first input has focus; moving to "c" blurs it.
        app.query_one("#c").focus()
        await pilot.pause()
        await pilot.press("x")
        await pilot.press("enter")
        await pilot.pause()
        assert validity(app.query_one("#c")) == ["-invalid"]
        assert app.log_ == [
            ("blurred", "a", "abc", ["Must be a valid number."]),
            ("changed", "c", "x", None),
            ("submitted", "c", "x", ["Must be longer than 2 characters."]),
        ]
        app.log_.clear()

        app.query_one("#d").focus()
        await pilot.pause()
        await pilot.press("x")
        await pilot.press("backspace")
        await pilot.pause()
        assert validity(app.query_one("#d")) == ["-valid"]
        assert app.log_ == [
            ("blurred", "c", "x", None),
            ("changed", "d", "x", ["Must be longer than 2 characters."]),
            ("changed", "d", "", None),
        ]
        app.log_.clear()

        # Changing valid_empty re-validates and posts Changed.
        d = app.query_one("#d", Input)
        d.valid_empty = False
        await pilot.pause()
        assert validity(d) == ["-invalid"]
        assert app.log_ == [("changed", "d", "", ["Must be longer than 2 characters."])]
        app.log_.clear()
        d.valid_empty = False  # unchanged: nothing happens
        await pilot.pause()
        assert app.log_ == []

        a = app.query_one("#a", Input)
        assert a.validate("12") == ValidationResult(failures=[])
        assert validity(a) == ["-valid"]
        b = app.query_one("#b", Input)
        assert [type(v) for v in b.validators] == [Integer]
        e = app.query_one("#e", Input)
        assert e.validate("zz") is None
        assert validity(e) == ["-valid"]


def test_input_constructor_like_textual() -> None:
    assert Input(validate_on=["changed", "bogus"]).validate_on == {"changed"}
    assert Input().validate_on == {"blur", "changed", "submitted"}
    assert Input(validate_on=[]).validate_on == set()
    assert [type(v) for v in Input(type="number").validators] == [Number]
    assert [type(v) for v in Input(type="integer", validators=[Length()]).validators] == [Length]
    single = Length()
    assert Input(validators=single).validators == [single]
    assert Input().validators == []
    assert Input().is_valid
    with pytest.raises(ValueError, match="Input type must be one of 'integer', 'number', or 'text'; not 'x'"):
        Input(type="x")  # type: ignore[arg-type]


async def test_failure_objects_reach_handlers() -> None:
    """Handlers see the validators' own Failure objects (subclasses, validator, value)."""
    seen = []

    class Even(Validator):
        class Odd(Failure):
            pass

        def validate(self, value: str) -> ValidationResult:
            return self.success() if value.isdigit() and int(value) % 2 == 0 else self.failure(failures=Even.Odd(self, value, "odd"))

    even = Even()

    class MyApp(App):
        def compose(self) -> ComposeResult:
            yield Input(validators=[even], id="n")

        def on_input_changed(self, event: Input.Changed) -> None:
            seen.append(event.validation_result)

    app = MyApp()
    async with app.run_test() as pilot:
        await pilot.press("3")
        await pilot.press("backspace", "4")
    [odd, empty, four] = seen
    assert isinstance(odd.failures[0], Even.Odd) and odd.failures[0].validator is even
    assert odd.failures[0].value == "3"
    assert not empty.is_valid  # "" is odd here (no valid_empty)
    assert four.is_valid


async def test_validation_runs_once_per_message() -> None:
    calls = []

    def check(value: str) -> bool:
        calls.append(value)
        return True

    class MyApp(App):
        def compose(self) -> ComposeResult:
            yield Input(validators=[Function(check)], id="i")

        def on_input_changed(self, event: Input.Changed) -> None:
            pass

    app = MyApp()
    async with app.run_test() as pilot:
        await pilot.press("a", "b")
    assert calls == ["a", "ab"]


async def test_masked_input_scenario_matches_textual() -> None:
    app = LogApp(
        MaskedInput(template="999", valid_empty=True, id="a"),
        MaskedInput(template="(999)", id="b"),
        MaskedInput(template="99", validators=[Function(lambda v: v != "12", "not 12")], id="c"),
        MaskedInput(template="99", validate_on=["submitted"], id="d"),
    )
    async with app.run_test() as pilot:
        await pilot.pause()
        state = {i.id: (i.value, validity(i), i.is_valid) for i in app.query(MaskedInput)}
        assert state == {
            "a": ("", [], True),
            "b": ("(", ["-invalid"], False),
            "c": ("", [], True),
            "d": ("", [], True),
        }
        assert app.log_ == [("changed", "b", "(", ["Value does not match template!"])]

        app.query_one("#c").focus()
        await pilot.pause()
        app.log_.clear()
        await pilot.press("1", "2")
        await pilot.press("enter")
        await pilot.pause()
        assert validity(app.query_one("#c")) == ["-invalid"]
        assert app.log_ == [
            ("changed", "c", "1", ["Value does not match template!"]),
            ("changed", "c", "12", ["not 12"]),
            ("submitted", "c", "12", ["not 12"]),
        ]

        app.query_one("#d").focus()
        await pilot.pause()
        app.log_.clear()
        await pilot.press("1")
        await pilot.press("enter")
        await pilot.pause()
        assert validity(app.query_one("#d")) == ["-invalid"]
        assert app.log_ == [
            ("changed", "d", "1", None),
            ("submitted", "d", "1", ["Value does not match template!"]),
        ]

        # valid_empty doesn't exempt the template.
        a = app.query_one("#a", MaskedInput)
        result = a.validate("")
        assert result.failure_descriptions == ["Value does not match template!"]
        assert result.failures[0].value == ""
        assert not a.is_valid
        # Characters past the template are ignored, as in Textual.
        assert a.validate("1234").is_valid


async def test_masked_input_template_change_revalidates() -> None:
    app = LogApp(MaskedInput(template="99", value="12", id="m"))
    async with app.run_test() as pilot:
        await pilot.pause()
        m = app.query_one("#m", MaskedInput)
        assert validity(m) == ["-valid"]
        app.log_.clear()
        m.template = "999"
        await pilot.pause()
        assert validity(m) == ["-invalid"]
        assert app.log_ == [("changed", "m", "12", ["Value does not match template!"])]

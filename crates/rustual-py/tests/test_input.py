"""Input: restrict, validators, messages."""

from rustual.app import App, ComposeResult
from rustual.validation import Function, Integer, Length, Number
from rustual.widgets import Input


async def test_restrict_regex_and_max_length():
    class MyApp(App):
        def compose(self) -> ComposeResult:
            yield Input(restrict=r"[0-9]*", max_length=4, id="digits")

    app = MyApp()
    async with app.run_test() as pilot:
        await pilot.press(*"1a2b345")
        assert app.query_one(Input).value == "1234"


async def test_validators_set_classes_and_results():
    results = []

    class MyApp(App):
        def compose(self) -> ComposeResult:
            yield Input(validators=[Number(minimum=1, maximum=100)], id="n")

        def on_input_changed(self, event: Input.Changed) -> None:
            results.append((event.value, event.validation_result.is_valid))

    app = MyApp()
    async with app.run_test() as pilot:
        await pilot.press("5")
        await pilot.press("0", "0")
        inp = app.query_one(Input)
        assert inp.has_class("-invalid")
        await pilot.press("backspace")
        assert inp.has_class("-valid")
    assert results == [("5", True), ("50", True), ("500", False), ("50", True)]


def test_validators_standalone():
    assert Integer().validate("12").is_valid
    assert not Integer().validate("1.5").is_valid
    assert Length(minimum=2).validate("a").failure_descriptions == ["Must be longer than 2 characters."]
    assert not Function(lambda v: v == "x", "must be x").validate("y").is_valid

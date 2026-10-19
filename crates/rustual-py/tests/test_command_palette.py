"""Command palette providers from Python (``App.COMMANDS``, ``Provider``, ``Hit``,
``DiscoveryHit``, ``SimpleProvider``, ``get_system_commands``, ``search_commands``)."""

from rustual.app import App, ComposeResult, SystemCommand
from rustual.command import DiscoveryHit, Hit, Hits, Provider
from rustual.screen import Screen
from rustual.widgets import Label

events: list[str] = []


class ColorProvider(Provider):
    COLORS = ["red", "green", "blue", "dark red"]

    async def startup(self) -> None:
        events.append("startup")

    async def discover(self) -> Hits:
        for color in self.COLORS[:2]:
            yield DiscoveryHit(f"Pick {color}", lambda c=color: self.app.pick(c), help="A color")

    async def search(self, query: str) -> Hits:
        matcher = self.matcher(query)
        for color in self.COLORS:
            name = f"Pick {color}"
            if (score := matcher.match(name)) > 0:
                yield Hit(score, matcher.highlight(name), lambda c=color: self.app.pick(c), help="A color")

    async def shutdown(self) -> None:
        events.append("shutdown")


class ScreenProvider(Provider):
    async def search(self, query: str) -> Hits:
        if "only" in query:
            yield Hit(1.0, "Screen only", lambda: self.app.pick("screen"))


class PaletteApp(App):
    COMMANDS = App.COMMANDS | {ColorProvider}

    def __init__(self) -> None:
        super().__init__()
        self.picked: list[str] = []

    def compose(self) -> ComposeResult:
        yield Label("main")

    def pick(self, value: str) -> None:
        self.picked.append(value)

    def get_system_commands(self, screen):
        yield from super().get_system_commands(screen)
        yield SystemCommand("Ring", "Ring the bell", lambda: self.pick("ring"))


def palette_lines(pilot) -> str:
    return pilot.screen_text()


async def type_text(pilot, text: str) -> None:
    for char in text:
        await pilot.press("space" if char == " " else char)
    await pilot.pause()


async def test_providers_discovery_search_and_run():
    events.clear()
    app = PaletteApp()
    async with app.run_test(size=(70, 50)) as pilot:
        await pilot.press("ctrl+p")
        await pilot.pause()
        text = palette_lines(pilot)
        # Discovery: system commands (incl. the override's) and the provider's discovery hits.
        for name in ("Theme", "Quit", "Keys", "Ring", "Pick red", "Pick green"):
            assert name in text, (name, text)
        assert "Pick blue" not in text
        assert events == ["startup"]

        await type_text(pilot, "pick blue")
        text = palette_lines(pilot)
        assert "Pick blue" in text and "Quit" not in text, text
        await pilot.press("enter")
        await pilot.pause()
        assert app.picked == ["blue"]
        assert events == ["startup", "shutdown"]

        await pilot.press("ctrl+p")
        await type_text(pilot, "ring")
        await pilot.press("enter")
        await pilot.pause()
        assert app.picked == ["blue", "ring"]


async def test_screen_commands_and_search_commands():
    class ProvidedScreen(Screen):
        COMMANDS = {ScreenProvider}

        def compose(self) -> ComposeResult:
            yield Label("screen")

    app = PaletteApp()
    async with app.run_test(size=(70, 50)) as pilot:
        await app.push_screen(ProvidedScreen())
        await pilot.press("ctrl+p")
        await type_text(pilot, "screen only")
        assert "Screen only" in palette_lines(pilot)
        await pilot.press("enter")
        await pilot.pause()
        assert app.picked == ["screen"]

        app.search_commands([("Alpha", lambda: app.pick("alpha"), "First"), ("Beta", lambda: app.pick("beta"))])
        await pilot.pause()
        text = palette_lines(pilot)
        assert "Alpha" in text and "Beta" in text and "Quit" not in text, text
        await type_text(pilot, "bet")
        await pilot.press("enter")
        await pilot.pause()
        assert app.picked == ["screen", "beta"]


async def test_matcher():
    from rustual.fuzzy import Matcher

    m = Matcher("re")
    assert m.match("re") > m.match("Remote one") > 0 == m.match("xyz")
    assert str(m.highlight("Remote")) == "Remote"
    assert "[reverse]R[/]" in m.highlight("Remote").__rs_markup__

    from rustual.command import Hit
    from rustual.content import Content

    # A hit's text is the plain text of its display.
    assert Hit(1.0, Content.from_markup("[b]Bold[/] name"), lambda: None).text == "Bold name"


async def test_keys_command_hides_shown_help_panel():
    app = PaletteApp()
    async with app.run_test(size=(100, 50)) as pilot:
        await pilot.press("ctrl+p")
        await pilot.pause()
        assert "Show help for the focused widget" in palette_lines(pilot)
        await type_text(pilot, "keys")
        await pilot.press("enter")
        await pilot.pause()
        assert app.screen.query("HelpPanel")

        # Textual offers "hide" once the help panel is shown (queried on the invoking screen,
        # not the palette).
        await pilot.press("ctrl+p")
        await pilot.pause()
        text = palette_lines(pilot)
        assert "Hide the keys and widget help panel" in text, text
        await type_text(pilot, "keys")
        await pilot.press("enter")
        await pilot.pause()
        assert not app.screen.query("HelpPanel")

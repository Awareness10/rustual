"""``layout: stream``: expected regions recorded from Textual 8.2.8 running the same app."""

from rustual.app import App, ComposeResult
from rustual.containers import Container, Vertical, VerticalScroll
from rustual.widgets import Static

LONG = "word " * 40

# (id, (x, y, width, height)) relative to the stream's content region, from Textual 8.2.8.
TEXTUAL = [
    ("fixed-height", (0, 0, 58, 1)),
    ("fixed-width", (0, 1, 58, 1)),
    ("wrap", (0, 2, 58, 4)),
    ("padded", (0, 6, 58, 8)),
    ("margins-a", (2, 15, 54, 1)),
    ("margins-b", (4, 18, 54, 1)),
    ("max", (0, 20, 58, 2)),
    ("max-pct", (0, 22, 58, 4)),
    ("min", (0, 26, 58, 4)),
    ("offset", (0, 30, 58, 1)),
    ("nested", (0, 31, 58, 4)),
    ("l0", (0, 35, 58, 1)),
    ("l29", (0, 64, 58, 1)),
]


class StreamApp(App):
    CSS = """
    #stream { layout: stream; height: 1fr; width: 60; }
    #fixed-height { height: 10; }
    #fixed-width { width: 20; }
    #padded { padding: 1 2; border: solid red; }
    #margins-a { margin: 1 2; }
    #margins-b { margin: 2 0 1 4; }
    #max { max-height: 2; }
    #max-pct { max-height: 10%; }
    #min { min-height: 4; }
    #offset { offset: 5 3; }
    #hidden { display: none; }
    #nested { height: auto; }
    #nested Static { height: 2; }
    """

    def compose(self) -> ComposeResult:
        with VerticalScroll(id="stream"):
            yield Static("fixed height ignored", id="fixed-height")
            yield Static("fixed width ignored", id="fixed-width")
            yield Static(LONG, id="wrap")
            yield Static(LONG, id="padded")
            yield Static("margins a", id="margins-a")
            yield Static("margins b", id="margins-b")
            yield Static(LONG, id="max")
            yield Static(LONG, id="max-pct")
            yield Static("min", id="min")
            yield Static("offset ignored", id="offset")
            yield Static("hidden", id="hidden")
            with Vertical(id="nested"):
                yield Static("n1")
                yield Static("n2")
            for i in range(30):
                yield Static(f"line {i}", id=f"l{i}")


async def test_stream_layout_matches_textual():
    app = StreamApp()
    async with app.run_test(size=(80, 24)) as pilot:
        await pilot.pause()
        stream = app.query_one("#stream")
        base = stream.content_region
        for widget_id, expected in TEXTUAL:
            r = app.query_one(f"#{widget_id}").region
            assert (r.x - base.x, r.y - base.y + stream.scroll_y, r.width, r.height) == expected, widget_id
        assert tuple(stream.virtual_size) == (58, 65)


class AutoApp(App):
    CSS = """
    #auto { layout: stream; height: auto; border: round green; padding: 0 1; margin: 1; }
    #auto Static { margin: 1 0; }
    #auto #big { margin: 3 2 2 2; height: 7; }
    #docked { dock: top; height: 2; }
    #after { height: 3; }
    """

    def compose(self) -> ComposeResult:
        with Container(id="auto"):
            yield Static("docked", id="docked")
            yield Static("a", id="a")
            yield Static("word " * 30, id="big")
            yield Static("c", id="c")
        yield Static("after", id="after")


async def test_auto_height_stream_with_dock_matches_textual():
    app = AutoApp()
    async with app.run_test(size=(50, 40)) as pilot:
        await pilot.pause()
        regions = {w: tuple(app.query_one(f"#{w}").region) for w in ("auto", "docked", "a", "big", "c", "after")}
    assert regions == {
        "auto": (1, 1, 48, 19),
        "docked": (3, 3, 44, 2),
        "a": (3, 7, 44, 1),
        "big": (5, 11, 40, 4),
        "c": (3, 17, 44, 1),
        "after": (0, 21, 50, 3),
    }


async def test_press_waits_for_scroll_animations():
    """Regression: ``pilot.press`` returned mid-animation when frames were fast (short labels),
    because animations only advance on frame ticks."""

    class ScrollApp(App):
        def compose(self) -> ComposeResult:
            with VerticalScroll(id="log"):
                for i in range(500):
                    yield Static(f"m{i}")

    app = ScrollApp()
    async with app.run_test(size=(40, 20)) as pilot:
        log = app.query_one("#log")
        await pilot.press("pagedown")
        assert log.scroll_y == 20
        await pilot.press("pagedown")
        assert log.scroll_y == 40

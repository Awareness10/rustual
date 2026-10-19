import pytest

from rustual import on
from rustual.app import App, ComposeResult
from rustual.widgets import (
    Button,
    Collapsible,
    ContentSwitcher,
    HelpPanel,
    KeyPanel,
    Label,
    Link,
    LoadingIndicator,
    Markdown,
    MarkdownViewer,
    Placeholder,
    Rule,
    Tab,
    TabbedContent,
    TabPane,
    Tabs,
    TextArea,
    Welcome,
)
from rustual.widgets.rule import InvalidLineStyle
from rustual.widgets.text_area import Selection, ThemeDoesNotExist

# ---------------------------------------------------------------------------- TabbedContent


class TabbedApp(App):
    def __init__(self, initial: str = "") -> None:
        super().__init__()
        self.initial = initial
        self.activated: list[str] = []
        self.on_beta: list[str] = []

    def compose(self) -> ComposeResult:
        with TabbedContent(initial=self.initial):
            with TabPane("Alpha", id="alpha"):
                yield Label("first page", id="l1")
            with TabPane("Beta", id="beta"):
                yield Label("second page", id="l2")
                yield Button("Go", id="go")

    def on_tabbed_content_tab_activated(self, event: TabbedContent.TabActivated) -> None:
        self.activated.append(event.pane.id)
        assert event.control is event.tabbed_content
        assert event.tab.id == "--content-tab-" + event.pane.id

    @on(TabbedContent.TabActivated, pane="#beta")
    def beta(self, event: TabbedContent.TabActivated) -> None:
        self.on_beta.append(event.pane.id)


async def test_tabbed_content_compose_and_switch():
    app = TabbedApp()
    async with app.run_test(size=(60, 15)) as pilot:
        await pilot.pause()
        tc = app.query_one(TabbedContent)
        assert tc.active == "alpha"
        assert tc.tab_count == 2
        assert app.activated == ["alpha"]
        text = pilot.screen_text()
        assert "Alpha" in text and "Beta" in text
        assert "first page" in text and "second page" not in text
        # Python children are mounted inside the pane, which is inside the switcher.
        label = app.query_one("#l2", Label)
        pane = tc.get_pane("beta")
        assert isinstance(pane, TabPane)
        assert label.parent is pane
        assert isinstance(pane.parent, ContentSwitcher)
        tc.active = "beta"
        assert tc.active == "beta"
        await pilot.pause()
        assert "second page" in pilot.screen_text()
        assert app.activated == ["alpha", "beta"]
        assert app.on_beta == ["beta"]
        assert tc.active_pane is pane
        # Keyboard: focus the tabs, go left.
        await pilot.click("#--content-tab-alpha")
        await pilot.pause()
        assert tc.active == "alpha"
        await pilot.press("right")
        assert tc.active == "beta"
        with pytest.raises(ValueError):
            tc.active = "nope"


async def test_tabbed_content_initial_and_panes():
    app = TabbedApp(initial="beta")
    async with app.run_test(size=(60, 15)) as pilot:
        tc = app.query_one(TabbedContent)
        assert tc.active == "beta"
        await tc.add_pane(TabPane("Gamma", Label("third"), id="gamma"), before="beta")
        await pilot.pause()
        assert tc.tab_count == 3
        assert [t.id for t in tc.query(Tab)] == [
            "--content-tab-alpha",
            "--content-tab-gamma",
            "--content-tab-beta",
        ]
        tc.active = "gamma"
        await pilot.pause()
        assert "third" in pilot.screen_text()
        await tc.remove_pane("gamma")
        await pilot.pause()
        assert tc.tab_count == 2
        assert tc.active in ("alpha", "beta")
        tc.disable_tab("alpha")
        await pilot.pause()
        assert tc.get_pane("alpha").disabled
        await tc.clear_panes()
        await pilot.pause()
        assert tc.tab_count == 0
        assert tc.active == ""


async def test_tabbed_content_titles_wrap_plain_children():
    class App2(App):
        def compose(self) -> ComposeResult:
            with TabbedContent("One", "Two"):
                yield Label("uno")
                yield Label("dos")

    app = App2()
    async with app.run_test(size=(60, 10)) as pilot:
        tc = app.query_one(TabbedContent)
        assert tc.active == "tab-1"
        assert isinstance(tc.get_pane("tab-2"), TabPane)
        assert "One" in pilot.screen_text() and "uno" in pilot.screen_text()


# ---------------------------------------------------------------------------- Tabs


async def test_tabs():
    events: list[tuple[str, str | None]] = []

    class TabsApp(App):
        def compose(self) -> ComposeResult:
            yield Tabs("Home", Tab("About", id="about"), active="about")

        def on_tabs_tab_activated(self, event: Tabs.TabActivated) -> None:
            events.append(("activated", event.tab.id))
            assert event.control is event.tabs

        def on_tabs_cleared(self, event: Tabs.Cleared) -> None:
            events.append(("cleared", None))

    app = TabsApp()
    async with app.run_test(size=(60, 10)) as pilot:
        tabs = app.query_one(Tabs)
        assert tabs.active == "about"
        assert tabs.tab_count == 2
        assert tabs.get_tab("tab-1").label_text == "Home"
        await tabs.add_tab("Extra")
        await tabs.add_tab(Tab("First", id="first"), before="tab-1")
        await pilot.pause()
        assert [t.id for t in tabs.query(Tab)] == ["first", "tab-1", "about", "tab-2"]
        tabs.focus()
        await pilot.press("right")
        assert tabs.active == "tab-2"
        await pilot.click("#first")
        assert tabs.active == "first"
        tabs.get_tab("first").label = "Premier"
        await pilot.pause()
        assert "Premier" in pilot.screen_text()
        await tabs.remove_tab("first")
        await pilot.pause()
        assert tabs.active == "tab-1"
        await tabs.clear()
        await pilot.pause()
        assert tabs.active == ""
    assert ("activated", "about") in events
    assert ("activated", "first") in events
    assert events[-1] == ("cleared", None)


# ---------------------------------------------------------------------------- ContentSwitcher


async def test_content_switcher():
    class SwitchApp(App):
        def compose(self) -> ComposeResult:
            with ContentSwitcher(initial="a"):
                yield Label("AAA", id="a")
                yield Label("BBB", id="b")

    app = SwitchApp()
    async with app.run_test(size=(40, 6)) as pilot:
        sw = app.query_one(ContentSwitcher)
        assert sw.current == "a"
        assert "AAA" in pilot.screen_text() and "BBB" not in pilot.screen_text()
        sw.current = "b"
        await pilot.pause()
        assert sw.visible_content.id == "b"
        assert "BBB" in pilot.screen_text() and "AAA" not in pilot.screen_text()
        await sw.add_content(Label("CCC"), id="c", set_current=True)
        await pilot.pause()
        assert sw.current == "c"
        assert "CCC" in pilot.screen_text()


# ---------------------------------------------------------------------------- Collapsible


async def test_collapsible():
    events: list[str] = []

    class CollApp(App):
        def compose(self) -> ComposeResult:
            with Collapsible(title="Details", id="c"):
                yield Label("hidden stuff")

        def on_collapsible_expanded(self, event: Collapsible.Expanded) -> None:
            events.append(f"expanded {event.collapsible.id}")

        @on(Collapsible.Collapsed, "#c")
        def collapsed(self, event: Collapsible.Collapsed) -> None:
            events.append("collapsed")

    app = CollApp()
    async with app.run_test(size=(40, 10)) as pilot:
        c = app.query_one(Collapsible)
        assert c.collapsed and c.title == "Details"
        assert "Details" in pilot.screen_text()
        assert "hidden stuff" not in pilot.screen_text()
        await pilot.click("CollapsibleTitle")
        assert not c.collapsed
        assert "hidden stuff" in pilot.screen_text()
        c.collapsed = True
        await pilot.pause()
        assert "hidden stuff" not in pilot.screen_text()
        c.title = "More"
        await pilot.pause()
        assert "More" in pilot.screen_text()
        # The Python child lives in the inner Contents container.
        assert app.query_one(Label).parent.parent is c
    assert events == ["expanded c", "collapsed"]


# ---------------------------------------------------------------------------- Markdown


DOC = """\
# Title

Some *text* with a [link](https://example.com) and [local](#second).

## Second

More.
"""


async def test_markdown():
    events: list[object] = []

    class MdApp(App):
        def __init__(self) -> None:
            super().__init__()
            self.opened: list[str] = []

        def compose(self) -> ComposeResult:
            yield Markdown(DOC, id="md")

        def open_url(self, url: str) -> None:
            self.opened.append(url)

        def on_markdown_link_clicked(self, event: Markdown.LinkClicked) -> None:
            events.append(event.href)
            assert event.markdown is self.query_one(Markdown)

        def on_markdown_table_of_contents_updated(self, event: Markdown.TableOfContentsUpdated) -> None:
            events.append(len(event.table_of_contents))

    app = MdApp()
    async with app.run_test(size=(60, 15)) as pilot:
        md = app.query_one(Markdown)
        assert md.source == DOC
        assert [(lvl, label) for lvl, label, _ in md.table_of_contents] == [(1, "Title"), (2, "Second")]
        assert "Some text with a link" in pilot.screen_text()
        await md.update("# New\n\nreplaced")
        await pilot.pause()
        assert "replaced" in pilot.screen_text()
        assert md.table_of_contents[0][1] == "New"
        await md.update(DOC)
        await pilot.pause()
        assert md.goto_anchor("second")
        assert not md.goto_anchor("missing")
        # Click the external link: LinkClicked bubbles and the URL goes through App.open_url.
        offset = pilot.screen_text().splitlines()
        row = next(i for i, line in enumerate(offset) if "link" in line)
        col = offset[row].index("link") + 1
        await pilot.click(offset=(col, row))
        await pilot.pause()
        assert "https://example.com" in events
        assert app.opened == ["https://example.com"]
    assert 2 in events


async def test_markdown_viewer():
    class ViewerApp(App):
        def compose(self) -> ComposeResult:
            yield MarkdownViewer(DOC, show_table_of_contents=True)

    app = ViewerApp()
    async with app.run_test(size=(80, 20)) as pilot:
        viewer = app.query_one(MarkdownViewer)
        assert isinstance(viewer.document, Markdown)
        assert viewer.document.source == DOC
        assert viewer.show_table_of_contents
        assert len(viewer.table_of_contents.table_of_contents) == 2
        viewer.show_table_of_contents = False
        await pilot.pause()
        assert not viewer.show_table_of_contents


# ---------------------------------------------------------------------------- TextArea


async def test_text_area_editing_and_messages():
    changes: list[str] = []
    selections: list[Selection] = []

    class TaApp(App):
        def compose(self) -> ComposeResult:
            yield TextArea("hello\nworld", id="ta")

        def on_text_area_changed(self, event: TextArea.Changed) -> None:
            changes.append(event.text_area.text)

        @on(TextArea.SelectionChanged, "#ta")
        def sel(self, event: TextArea.SelectionChanged) -> None:
            selections.append(event.selection)

    app = TaApp()
    async with app.run_test(size=(40, 10)) as pilot:
        ta = app.query_one(TextArea)
        assert ta.text == "hello\nworld"
        assert ta.document.line_count == 2
        ta.focus()
        await pilot.press("end", "!")
        assert ta.text == "hello!\nworld"
        assert ta.cursor_location == (0, 6)
        assert changes[-1] == "hello!\nworld"
        assert selections and selections[-1] == Selection((0, 6), (0, 6))
        result = ta.insert("XX", (1, 0))
        assert result.end_location == (1, 2)
        assert ta.text == "hello!\nXXworld"
        ta.delete((1, 0), (1, 2))
        ta.replace("WORLD", (1, 0), (1, 5))
        assert ta.text == "hello!\nWORLD"
        ta.selection = Selection((0, 0), (0, 5))
        assert ta.selected_text == "hello"
        ta.select_all()
        assert ta.selected_text == "hello!\nWORLD"
        ta.cursor_location = (1, 2)
        assert ta.selection == Selection.cursor((1, 2))
        assert ta.get_text_range((0, 0), (0, 2)) == "he"
        assert ta.get_line(1).plain == "WORLD"
        ta.undo()
        assert ta.text == "hello!\nworld"
        ta.redo()
        assert ta.text == "hello!\nWORLD"
        ta.clear()
        assert ta.text == ""
        ta.load_text("fresh")
        assert ta.text == "fresh" and not ta.can_undo
        ta.text = "via property"
        await pilot.pause()
        assert "via property" in pilot.screen_text()


async def test_text_area_configuration():
    class TaApp(App):
        def compose(self) -> ComposeResult:
            yield TextArea.code_editor("def f():\n    pass\n", language="python")
            yield TextArea("ro", read_only=True, id="ro")

    app = TaApp()
    async with app.run_test(size=(60, 15)) as pilot:
        code = app.query_one(TextArea)
        assert code.language == "python"
        assert code.theme == "monokai"
        assert code.show_line_numbers and not code.soft_wrap
        assert code.tab_behavior == "indent"
        assert "monokai" in code.available_themes
        code.theme = "dracula"
        assert code.theme == "dracula"
        with pytest.raises(ThemeDoesNotExist):
            code.theme = "no-such-theme"
        code.language = None
        assert code.language is None
        code.focus()
        await pilot.press("tab")
        assert code.text.startswith("    def")
        ro = app.query_one("#ro", TextArea)
        assert ro.read_only
        ro.focus()
        await pilot.press("x")
        assert ro.text == "ro"
        ro.read_only = False
        await pilot.press("x")
        assert ro.text == "xro"


# ---------------------------------------------------------------------------- simple widgets


async def test_rule_placeholder_link_misc():
    class MiscApp(App):
        CSS = "Placeholder { height: 5; } LoadingIndicator { height: 1; }"

        def __init__(self) -> None:
            super().__init__()
            self.opened: list[str] = []

        def compose(self) -> ComposeResult:
            yield Rule(line_style="heavy")
            yield Placeholder("PH", id="ph")
            yield Link("Docs", url="https://textual.textualize.io", id="link")
            yield LoadingIndicator()

        def open_url(self, url: str) -> None:
            self.opened.append(url)

    app = MiscApp()
    async with app.run_test(size=(80, 40)) as pilot:
        rule = app.query_one(Rule)
        assert rule.orientation == "horizontal" and rule.line_style == "heavy"
        assert "━━━━" in pilot.screen_text()
        rule.line_style = "double"
        await pilot.pause()
        assert "════" in pilot.screen_text()
        with pytest.raises(InvalidLineStyle):
            rule.line_style = "wavy"
        assert Rule.vertical().orientation == "vertical"
        ph = app.query_one(Placeholder)
        assert ph.variant == "default"
        assert "PH" in pilot.screen_text()
        ph.cycle_variant()
        assert ph.variant == "size"
        assert ph.has_class("-size")
        ph.variant = "text"
        await pilot.pause()
        assert "Lorem ipsum" in pilot.screen_text()
        link = app.query_one(Link)
        assert link.text == "Docs" and link.url == "https://textual.textualize.io"
        await pilot.click("#link")
        assert app.opened == ["https://textual.textualize.io"]
        link.text = "Guide"
        await pilot.pause()
        assert "Guide" in pilot.screen_text()
        assert app.query_one(LoadingIndicator).is_mounted


async def test_help_and_key_panels():
    class PanelApp(App):
        BINDINGS = [("ctrl+t", "noop", "Do nothing")]

        def compose(self) -> ComposeResult:
            yield TextArea("x")
            yield HelpPanel()
            yield KeyPanel()

    app = PanelApp()
    async with app.run_test(size=(120, 30)) as pilot:
        app.query_one(TextArea).focus()
        await pilot.pause()
        assert app.query_one(HelpPanel).is_mounted and app.query_one(KeyPanel).is_mounted
        assert "Do nothing" in pilot.screen_text()


async def test_welcome():
    class WelcomeApp(App):
        def compose(self) -> ComposeResult:
            yield Welcome()

        def on_button_pressed(self, event: Button.Pressed) -> None:
            self.exit(event.button.id)

    app = WelcomeApp()
    async with app.run_test(size=(80, 30)) as pilot:
        assert "Welcome" in pilot.screen_text()
        await pilot.click("#close")
    assert app.return_value == "close"

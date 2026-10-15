"""TabbedContent with a Markdown pane, a TextArea code editor and a few other widgets.

Run with ``uv run python examples/content.py``.
"""

from rustual import on
from rustual.app import App, ComposeResult
from rustual.widgets import (
    Collapsible,
    Footer,
    Header,
    Label,
    Link,
    Markdown,
    Placeholder,
    Rule,
    TabbedContent,
    TabPane,
    TextArea,
)

README = """\
# rustual

A **Textual-compatible** Python API on a Rust engine.

- Layout, styling and rendering run in Rust
- Your handlers run in Python

| Widget | Backed by |
|--------|-----------|
| `Markdown` | Rust |
| `TextArea` | Rust |
"""

CODE = '''\
def greet(name: str) -> str:
    """Say hello."""
    return f"Hello, {name}!"


print(greet("world"))
'''


class ContentApp(App):
    TITLE = "Content widgets"
    CSS = """
    TabbedContent { height: 1fr; }
    TabPane { padding: 1 2; }
    #editor { height: 12; }
    Placeholder { height: 5; }
    """
    BINDINGS = [("m", "show('markdown')", "Markdown"), ("e", "show('editor-pane')", "Editor")]

    def compose(self) -> ComposeResult:
        yield Header()
        with TabbedContent(initial="markdown"):
            with TabPane("Markdown", id="markdown"):
                yield Markdown(README)
            with TabPane("Editor", id="editor-pane"):
                yield TextArea.code_editor(CODE, language="python", id="editor")
                yield Label("", id="status")
            with TabPane("Other", id="other"):
                with Collapsible(title="More details", collapsed=False):
                    yield Label("Collapsible content")
                yield Rule(line_style="heavy")
                yield Placeholder("Placeholder")
                yield Link("Textual documentation", url="https://textual.textualize.io")
        yield Footer()

    def action_show(self, pane: str) -> None:
        self.query_one(TabbedContent).active = pane

    @on(TabbedContent.TabActivated, pane="#editor-pane")
    def editor_shown(self) -> None:
        self.query_one("#editor", TextArea).focus()

    def on_text_area_changed(self, event: TextArea.Changed) -> None:
        lines = event.text_area.document.line_count
        self.query_one("#status", Label).update(f"{lines} lines")


if __name__ == "__main__":
    ContentApp().run()

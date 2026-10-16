"""A chat client: messages in a ``layout: stream`` log, assistant replies rendered as Markdown and
streamed in word by word (like an LLM client), input history, and the keys you'd expect.

The assistant is simulated (canned replies picked by keyword: try "python", "table" or "plan"),
so the demo runs offline. Written against Textual's API: replace ``rustual`` with ``textual`` in
the imports and it runs under Textual."""

from __future__ import annotations

import re
from collections import deque

from rustual.app import App, ComposeResult
from rustual.binding import Binding
from rustual.containers import VerticalScroll
from rustual.markup import escape
from rustual.timer import Timer
from rustual.widgets import Footer, Header, Input, Markdown, Static

WELCOME = "**Hi!** I'm a simulated assistant. Ask me something — try *python*, *table* or *plan*."

REPLIES = {
    "python": """\
Here's a small Python function that computes Fibonacci numbers iteratively:

```python
def fib(n: int) -> int:
    a, b = 0, 1
    for _ in range(n):
        a, b = b, a + b
    return a
```

It runs in **O(n)** time and constant space. `fib(10)` is `55`.""",
    "table": """\
Some terminal UI libraries:

| Library | Language | Style |
|---|---|---|
| Textual | Python | DOM + CSS |
| rustual | Rust / Python | DOM + CSS |
| Ratatui | Rust | immediate mode |

*rustual* re-creates Textual's model in Rust.""",
    "plan": """\
A plan for shipping a release:

1. Freeze features on `dev`.
2. Run the full test suite and fix what breaks.
3. Update the **changelog**.
4. Tag the release and let CI publish it.

> Small, frequent releases are easier to review.""",
}


def reply(prompt: str) -> str:
    """A canned reply for a prompt."""
    for keyword, text in REPLIES.items():
        if keyword in prompt.lower():
            return text
    quoted = prompt.replace("\n", " ")
    return (
        f"You said:\n\n> {quoted}\n\nI'm only a simulation, so I can't really answer — but I can "
        "show **Markdown**: *emphasis*, `inline code`, lists and more."
    )


def tokens(text: str) -> deque[str]:
    """Split a reply into the chunks a streaming API would deliver (words with their spacing)."""
    return deque(re.findall(r"\S*\s*", text)[:-1] or [text])


class ChatApp(App):
    TITLE = "Chat"
    CSS = """
    #log { layout: stream; height: 1fr; padding: 0 1; }
    .message { padding: 0 1; margin-top: 1; }
    .user { margin-left: 16; background: $primary-muted; border-left: wide $primary; }
    .assistant { margin-right: 8; background: $boost; border-left: wide $accent; }
    .assistant.-streaming { border-left: wide $warning; }
    #prompt { margin: 0 1 1 1; }
    """
    BINDINGS = [
        Binding("escape", "stop", "Stop"),
        Binding("ctrl+l", "clear", "Clear"),
        Binding("ctrl+n", "new_chat", "New chat"),
        Binding("up", "history(True)", "Previous", show=False),
        Binding("down", "history(False)", "Next", show=False),
        Binding("ctrl+q", "quit", "Quit", priority=True),
    ]

    def __init__(self) -> None:
        super().__init__()
        self.history: list[str] = []
        # Position while browsing `history` with up / down (None: not browsing).
        self.history_index: int | None = None
        self.messages = 1
        # The reply being streamed: its widget, the chunks still to come, and the timer.
        self.streaming: tuple[Markdown, deque[str], Timer] | None = None

    def compose(self) -> ComposeResult:
        yield Header()
        with VerticalScroll(id="log"):
            yield Markdown(WELCOME, classes="message assistant")
        yield Input(placeholder="Message the assistant… (enter to send)", id="prompt")
        yield Footer()

    def on_mount(self) -> None:
        self.update_status()
        self.query_one("#prompt").focus()

    def update_status(self) -> None:
        typing = " · typing…" if self.streaming else ""
        self.sub_title = f"{self.messages} messages{typing}"

    def on_input_submitted(self, event: Input.Submitted) -> None:
        text = event.value.strip()
        if text:
            event.input.clear()
            self.send(text)

    def send(self, text: str) -> None:
        self.action_stop()
        self.history.append(text)
        self.history_index = None
        self.messages += 2
        log = self.query_one("#log", VerticalScroll)
        answer = Markdown("", classes="message assistant -streaming")
        log.mount(Static(escape(text), classes="message user"), answer)
        timer = self.set_interval(0.03, self.deliver)
        self.streaming = (answer, tokens(reply(text)), timer)
        self.update_status()
        log.scroll_end(animate=False)

    async def deliver(self) -> None:
        """Append the next chunk of the reply being streamed."""
        if self.streaming is None:
            return
        answer, pending, _ = self.streaming
        if not pending:
            self.action_stop()
            return
        await answer.append(pending.popleft())
        self.query_one("#log", VerticalScroll).scroll_end(animate=False)

    def action_stop(self) -> None:
        """Stop streaming (the reply so far stays)."""
        if self.streaming is not None:
            answer, pending, timer = self.streaming
            self.streaming = None
            timer.stop()
            answer.remove_class("-streaming")
            if pending:
                answer.append(" *(stopped)*")
        self.update_status()

    def action_clear(self) -> None:
        self.action_stop()
        self.query_one("#log").remove_children()
        self.messages = 0
        self.update_status()

    def action_new_chat(self) -> None:
        self.action_clear()
        self.query_one("#log").mount(Markdown(WELCOME, classes="message assistant"))
        self.history.clear()
        self.messages = 1
        self.update_status()
        self.notify("Started a new chat")

    def action_history(self, older: bool) -> None:
        if not self.history:
            return
        last = len(self.history) - 1
        index = self.history_index
        if older:
            index = last if index is None else max(index - 1, 0)
        else:
            index = index + 1 if index is not None and index < last else None
        self.history_index = index
        prompt = self.query_one("#prompt", Input)
        prompt.value = "" if index is None else self.history[index]
        prompt.cursor_position = len(prompt.value)


if __name__ == "__main__":
    ChatApp().run()

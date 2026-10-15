# textual_rs (Python)

A **Textual-compatible Python API** running on the [textual-rs](../../README.md) engine.
Write apps exactly as with [Textual](https://textual.textualize.io): same classes, `compose()`,
CSS, reactive attributes, messages, `@on`, screens, workers and `run_test()`. Layout, styling,
rendering, input handling and the built-in widgets run in Rust.

```python
from textual_rs import on
from textual_rs.app import App, ComposeResult
from textual_rs.containers import Horizontal
from textual_rs.reactive import reactive
from textual_rs.widgets import Button, Footer, Header, Static


class Counter(Static):
    count = reactive(0)

    def render(self) -> str:
        return f"Count: [b]{self.count}[/b]"


class CounterApp(App):
    CSS = "Counter { height: 3; border: round $accent; }"
    BINDINGS = [("+", "increment", "Add one")]

    def compose(self) -> ComposeResult:
        yield Header()
        yield Counter()
        with Horizontal():
            yield Button("Add", id="add", variant="primary")
            yield Button("Reset", id="reset")
        yield Footer()

    def action_increment(self) -> None:
        self.query_one(Counter).count += 1

    @on(Button.Pressed, "#add")
    def add(self) -> None:
        self.action_increment()

    @on(Button.Pressed, "#reset")
    def reset(self) -> None:
        self.query_one(Counter).count = 0


if __name__ == "__main__":
    CounterApp().run()
```

Porting a Textual app usually means replacing `textual` with `textual_rs` in the imports.

## Install / build

Requires Rust (stable) and [uv](https://docs.astral.sh/uv/).

```sh
cd crates/textual-py
uv sync                                   # builds the extension with maturin
uv run python examples/counter.py
uv run pytest                             # headless tests
uv sync --reinstall-package textual-rs    # rebuild after changing Rust code
```

`rich` is optional (`textual-rs[rich]`); it's needed only to display Rich renderables
(`Table`, `Syntax`, `Panel`, ...) in `Static` or `render()`.

## Performance

The same app code run headlessly with `run_test()` (`bench/bench.py`, release build, median of
3 runs, Python 3.14):

| Scenario | Textual 8.2.8 | textual_rs | Speed-up |
|---|---:|---:|---:|
| Startup with 1000 rows (3000 widgets) | 1951 ms | 35 ms | ~56× |
| 100 key presses updating a reactive | 5191 ms | 220 ms | ~24× |
| 100 scroll steps over 1000 rows | 11456 ms | 853 ms | ~13× |

Each textual_rs step includes rendering a full frame. Run it yourself with
`uv run python bench/bench.py textual_rs` and, in an environment with Textual installed,
`python bench/bench.py textual`.

## What's supported

- **App**: `CSS`, `CSS_PATH`, `TITLE`/`SUB_TITLE`, `BINDINGS`, `SCREENS`, `MODES`, `run()`
  (returns the `exit()` value), `run_async()`, `run_test()`, `exit()`, `notify()`, `bell()`,
  `push_screen(screen, callback)`, `pop_screen`, `switch_screen`, `install_screen`,
  `switch_mode`, `query`/`query_one`, `focused`/`set_focus`, `theme` / `register_theme`,
  `call_from_thread`, `copy_to_clipboard`, `push_screen_wait`, `open_url`.
- **Widget**: `compose()` (with `with Container():` nesting), `render()` (markup `str`,
  `Content`, Rich renderables), `DEFAULT_CSS`, `BINDINGS`, `can_focus`, `query*`, classes,
  `styles.*`, `display`/`visible`/`disabled`/`loading`, `border_title`/`border_subtitle`,
  `tooltip`, `mount`/`remove`/`remove_children`/`recompose`, `focus`/`blur`, scrolling,
  `set_timer`/`set_interval`/`call_later`, `post_message`, `run_worker`, `animate()`,
  `region`/`size`.
- **Reactivity**: `reactive`/`var` with `watch_*` (0–2 args, sync or async), `validate_*`,
  `compute_*`, `init`, `always_update`, `layout`, `recompose`, `toggle_class`, `watch()`.
- **Messages & events**: custom `Message` subclasses (handler names like Textual:
  `MyWidget.Changed` → `on_my_widget_changed`), bubbling, `stop()`, `prevent_default()`,
  `@on(Message, "selector")`, `key_<name>` methods, `on_key`, mouse, focus, enter/leave,
  resize and paste events; handlers on every class in the MRO run, as in Textual.
- **Async**: async handlers / watchers / actions, `@work` async workers (on an asyncio loop on
  the app thread, so they can update widgets directly), `@work(thread=True)` thread workers.
- **Screens**: `Screen`, `ModalScreen[T]`, `dismiss(result)`, screen `CSS`.
- **Validation**: `textual_rs.validation` (`Number`, `Integer`, `Length`, `Regex`, `Function`,
  `URL`) for `Input(validators=...)`; `Input(restrict=regex)`.
- **Widgets**: see `textual_rs.widgets` (the built-in widgets of textual-rs).

## Thread and GIL safety

- The event loop runs with the **GIL released**; handlers re-acquire it. Python threads keep
  running while the UI is idle.
- Widget APIs may be called from **any thread**: calls from a worker thread are run on the app
  thread automatically (Textual requires `app.call_from_thread`, which works too). The calling
  thread waits without holding the GIL, so this can't deadlock the event loop.
- If the app exits while a thread is waiting on such a call, the thread gets a
  `NoActiveAppError` instead of hanging.
- An exception in a handler exits the app cleanly (terminal restored) and is re-raised from
  `run()`, or from the next `Pilot` call in tests.

See [`docs/python-bindings.md`](../../docs/python-bindings.md) for the design.

## Differences from Textual

- `render_line()` (the line API), devtools / `log()` output, the command palette provider API,
  `App.suspend()` and inline mode aren't supported yet.
- Some widget APIs are partial; the gaps per widget are listed in `features.md` at the repo root.
- `widget.styles` attributes can be set (inline CSS); reading returns what was set, plus the
  computed `display` / `visibility`.
- Accessing the DOM from `compose()` or a widget's constructor raises an error.

<div align="center">

<img src="https://raw.githubusercontent.com/Awareness10/rustual/main/docs/assets/logo.svg" alt="rustual" width="360">

### rustual-py: Textual's Python API on a Rust engine

[![CI](https://github.com/Awareness10/rustual/actions/workflows/ci.yml/badge.svg)](https://github.com/Awareness10/rustual/actions/workflows/ci.yml)
[![Python tests](https://img.shields.io/endpoint?style=flat-square&url=https%3A%2F%2Fraw.githubusercontent.com%2FAwareness10%2Frustual%2Fbadges%2Fpython-tests.json)](https://github.com/Awareness10/rustual#testing)
[![Python coverage](https://img.shields.io/endpoint?style=flat-square&url=https%3A%2F%2Fraw.githubusercontent.com%2FAwareness10%2Frustual%2Fbadges%2Fpython-coverage.json)](https://github.com/Awareness10/rustual#testing)
[![Python 3.10+](https://img.shields.io/badge/python-3.10%2B-3776AB?style=flat-square&logo=python&logoColor=white)](https://github.com/Awareness10/rustual/tree/main/crates/rustual-py)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue?style=flat-square)](https://github.com/Awareness10/rustual/blob/main/LICENSE)

<img src="https://raw.githubusercontent.com/Awareness10/rustual/main/docs/assets/screenshots/py_stopwatch.svg" alt="Textual's stopwatch tutorial running on rustual" width="720">

<sub>Textual's stopwatch tutorial, unchanged except for its imports.</sub>

</div>

A **Textual-compatible Python API** running on the [rustual](https://github.com/Awareness10/rustual) engine.
Write apps exactly as with [Textual](https://textual.textualize.io): same classes, `compose()`,
CSS, reactive attributes, messages, `@on`, screens, workers and `run_test()`. Layout, styling,
rendering, input handling and the built-in widgets run in Rust.

```python
from rustual import on
from rustual.app import App, ComposeResult
from rustual.containers import Horizontal
from rustual.reactive import reactive
from rustual.widgets import Button, Footer, Header, Static


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

The distribution is called `rustual-py`; the import package is `rustual`. Porting a Textual app
usually means replacing `textual` with `rustual` in the imports.

## Install

```sh
pip install rustual-py        # or: uv add rustual-py
```

Prebuilt wheels (CPython 3.10+, one per platform: Linux x86_64 glibc and musl, Windows x86_64,
macOS Intel + Apple silicon) are on [PyPI](https://pypi.org/project/rustual-py/) and attached to
the [GitHub releases](https://github.com/Awareness10/rustual/releases).

## Build from source

Requires Rust (stable) and [uv](https://docs.astral.sh/uv/).

```sh
cd crates/rustual-py
uv sync                                   # builds the extension with maturin
uv run python examples/counter.py
uv run pytest                             # headless tests
uv sync --reinstall-package rustual-py    # rebuild after changing Rust code
```

`rich` is optional (`rustual-py[rich]`); it's needed only to display Rich renderables
(`Table`, `Syntax`, `Panel`, ...) in `Static` or `render()`.

## Performance

The same app code run headlessly with `run_test()` (`bench/bench.py`, release build, median of
3 runs, Python 3.14):

| Scenario | Textual 8.2.8 | rustual | Speed-up |
|---|---:|---:|---:|
| Startup with 1000 rows (3000 widgets) | 1951 ms | 35 ms | ~56× |
| 100 key presses updating a reactive | 5191 ms | 220 ms | ~24× |
| 100 scroll steps over 1000 rows | 11456 ms | 853 ms | ~13× |

Each rustual step includes rendering a full frame. Run it yourself with
`uv run python bench/bench.py rustual` and, in an environment with Textual installed,
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
- **Validation**: `rustual.validation` (`Number`, `Integer`, `Length`, `Regex`, `Function`,
  `URL`) for `Input(validators=...)`; `Input(restrict=regex)`.
- **Widgets**: see `rustual.widgets` (the built-in widgets of rustual).

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

See [`docs/python-bindings.md`](https://github.com/Awareness10/rustual/blob/main/docs/python-bindings.md) for the design.

## Differences from Textual

- `render_line()` (the line API), devtools / `log()` output, the command palette provider API,
  `App.suspend()` and inline mode aren't supported yet.
- Some widget APIs are partial; the gaps per widget are listed in `features.md` at the repo root.
- `widget.styles` attributes can be set (inline CSS); reading returns what was set, plus the
  computed `display` / `visibility`.
- Accessing the DOM from `compose()` or a widget's constructor raises an error.

## License

MIT, like Textual. See [`LICENSE`](LICENSE), which also carries the Textual and Rich copyright
notices for the code ported from them.

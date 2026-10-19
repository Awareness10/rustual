# Python bindings (`rustual`)

`crates/rustual-py` is a PyO3 extension plus a pure-Python package that exposes a
**Textual-compatible Python API** on top of the rustual engine. Porting a Textual app is meant
to be a matter of replacing `textual` with `rustual` in its imports:

```python
from rustual.app import App, ComposeResult
from rustual.widgets import Button, Header, Footer
from rustual.reactive import reactive
from rustual import on, work
```

Layout, styling, rendering, compositing, input handling and the built-in widgets run in Rust;
Python code runs only in your handlers, `compose()`, `render()` and workers.

## Building and testing

```sh
cd crates/rustual-py
uv sync                                  # builds the extension (maturin) into .venv
uv sync --reinstall-package rustual-py   # rebuild after changing Rust code
uv run pytest                            # Python tests (headless, via Pilot)
uv run rustual-py                        # the example launcher (or: uv run rustual-py tasks)
```

Python-only changes don't need a rebuild (the package is installed in editable mode).

## Architecture

```
python/rustual/            pure-Python API (App, Widget, reactive, messages, widgets, ...)
  _bridge.py                  `native` proxy: marshals calls from other threads
  _context.py                 active app + asyncio integration
  widget.py                   Widget base class, compose stack, dispatch, queries, timers
src/
  bridge.rs                   frame stack: how Python reaches the running app safely
  adapter.rs                  PyWidget: a Python object mounted as a Rust `Widget`
  ops.rs                      native functions (all take explicit node ids)
  runner.rs                   App.run (GIL released) and the test Pilot
  widgets/                    built-in widget bindings (constructors, attributes, messages)
```

### Python widgets in the Rust DOM

Every widget mounted from Python is a `PyWidget` adapter that holds the Python object and a
*base* Rust widget: the built-in that backs it (`Button`, `Input`, ...) or an empty widget for
pure-Python widgets and the app. The adapter calls the object's `_rs_*` hooks (compose, mount,
events, messages, render, actions) when its class defines matching handlers, and otherwise
delegates to the base. Typed lookups in the core (`dom.widget::<Button>()`, timers, ...) see
through the adapter via `Widget::delegate`.

Per-class information (CSS type names, `DEFAULT_CSS`, handler names, bindings) is computed once
by `Widget._rs_class_info()` and cached, so the adapter only crosses into Python for events the
class actually handles.

### The frame stack (how Python reaches the app)

The core gives access to the DOM only through a `Ctx` (inside handlers) or a `RenderCtx` (inside
`render`). Python runs *inside* those calls, so before calling Python the adapter pushes a frame
holding pointers to its context and its checked-out base widget; native functions use the top
frame:

| Top frame | Access |
|---|---|
| handler (`Ctx`) | full access |
| `render` | read-only; mutations raise `RuntimeError` |
| `compose` / screen factory | none (the runtime is mid-mount); use `on_mount` |
| none, test pilot registered | access through the pilot (test code between steps) |
| none, another thread | `WrongThreadError`, turned into a `call_from_thread` round trip (except `post_message`, which is queued without waiting) |

All native functions take explicit node ids, so they behave the same whichever frame is on top.

### GIL and threads

- `App.run()` runs the event loop inside `py.detach` (GIL released); every callback re-attaches
  with `Python::attach`. Python threads keep running while the UI is idle.
- Nothing blocks on another thread while holding the GIL. A worker thread calling a widget API
  posts the call to the app thread and waits on a `threading.Event` (which releases the GIL).
- `post_message` from another thread never waits (as in Textual): the message goes through the
  app's `AppHandle` channel, in order, and is dropped on the app thread if its target has been
  removed by then. Prefer it to widget calls for high-rate updates from threads.
- `App.call_from_thread` runs a callable on the app thread and returns its result; an async
  callable is awaited on the app's loop first. Like Textual it raises `RuntimeError` on the app
  thread itself.
- Rust panics inside native functions become Python exceptions (PyO3); the frame stack unwinds
  with them, and the terminal is restored by the driver's panic hook.
- A Python exception in a handler is recorded, the app exits, and `run()` (or the next `Pilot`
  step) re-raises it, as in Textual.

### asyncio

Async handlers, async watchers and async workers run as tasks on an asyncio loop that lives on
the app thread, so they may touch widgets directly:

- `App.run()`: the loop is owned by `rustual` and pumped by the Rust event loop (each pump
  runs ready callbacks without blocking). After each pump a one-shot native timer is set for the
  loop's next scheduled callback, so `await asyncio.sleep(1 / 60)` ticks at 60 Hz. On Linux a
  daemon thread (`rustual-asyncio`) blocks in `poll()` on the loop's epoll descriptor and posts
  a pump as soon as a socket, pipe or `call_soon_threadsafe` makes it readable (it waits for
  that pump before polling again). Elsewhere (macOS for now, Windows' proactor loop) pending
  tasks are polled every 10 ms instead.
- `App.run_test()`: the test's running loop is used; tasks start eagerly and `Pilot` methods
  yield to the loop until the app is idle.

Mount/remove/screen operations complete immediately and return awaitables that never block,
so `await self.mount(...)` works in sync and async code alike.

## Adding a widget binding

A built-in widget needs a Rust part (in `src/widgets/<group>.rs`) and a Python class
(in `python/rustual/widgets/_<name>.py`). Follow `src/widgets/basic.rs` and
`python/rustual/widgets/_button.py` / `_input.py`.

**Rust** (`register(r: &mut Registry)` in your module, called from `widgets/mod.rs::registry()`):

1. `r.kind("Name", |py, kwargs, children| ...)`: build the widget from the Python constructor
   kwargs (`kw::<T>(kwargs, "key")?` returns `None` when absent or `None`). Widgets whose children
   are data (tab panes, list items) may consume `children`; whatever is left is mounted as
   ordinary children. Container widgets whose Rust implementation moves `.with(...)` children
   into place on mount (`TabbedContent`, `Collapsible`, `Tabs`, ...) need nothing special: the
   Python children are mounted as children and moved like Rust ones (see `widgets/content.rs`).
   A caller can mount a Python widget itself with `adapter::build_child(py, obj)`.
2. A getter `fn(py, &dyn Widget, attr) -> Option<PyResult<Py<PyAny>>>`: downcast with
   `as_ref::<W>(w)`, return `None` if it isn't your widget or attribute.
3. A setter and a caller (methods) in the same style; they get a `&mut Ctx` targeting the
   widget, for operations that post messages or change classes.
4. Messages: `r.message::<rust::Message>("on_name_message", build)` where `build` creates the
   Python message with `new_py(py, "Name.Message", &kwargs)`. The sender is attached
   automatically (`message.sender` / `control`).

**Python**:

1. `class Name(Widget)` with `_rs_kind = "Name"` and `_rs_framework = True`; the constructor
   matches Textual's signature and stores native kwargs in `self._rs_kw`.
2. Attributes as properties using `self._native_get(attr, default)` /
   `self._native_set(attr, value)` (these work before mounting by reading/writing `_rs_kw`);
   methods via `self._native_call(method, *args)`.
3. Message classes nested in the widget class (so handler names match Textual, e.g.
   `Checkbox.Changed` → `on_checkbox_changed`), with a `control` property; register them with
   `_native.register_class("Name.Message", factory)`.
4. Register the class for proxies: `_NATIVE_CLASSES["Name"] = Name`, and export it from
   `widgets/__init__.py`.
5. Add tests in `tests/test_widgets_*.py` that drive the widget with `app.run_test()`.

**Widgets filled before mounting** (`DataTable`, `Tree`, `Log`, ... — see `src/widgets/data.rs`
and `python/rustual/widgets/_detached.py`): the Python class mixes in `DetachedNative`, and
its methods use `self._call(...)` / `self._get(...)`. Before the widget is mounted these run on
a *detached* Rust widget (`_native.Detached`, built lazily from `_rs_kw`) which the kind
constructor takes over at mount time (the `_detached` kwarg); afterwards they go to the mounted
widget. The Rust method dispatcher receives `Option<&mut Ctx>` and uses ctx-free fallbacks
(e.g. `Log::push`, `Tree::set_expanded`) when there is no ctx.

## Differences from Textual

- Import from `rustual` instead of `textual`.
- `render()` may return a markup `str`, `rustual.content.Content` or a Rich `Text`
  (converted through its markup); other Rich renderables are rendered with `str()`.
  `render_line()` (the line API) isn't supported.
- `widget.styles` is write-mostly: setting any CSS property works (inline styles); reading
  returns what was set (and the computed `display` / `visibility`).
- Accessing the DOM from `compose()` or a widget constructor raises (Textual raises `NoMatches`).
- `log()` / `self.log` write to the file named by `TEXTUAL_LOG` (as Textual does without devtools)
  and to Python's `logging` (logger `"rustual"`); there is no devtools console.
- Command palette `Provider`s run their async `search` / `discover` as asyncio tasks on the app
  thread; hits go to the Rust palette with `palette_add_hits`, tagged with the query's generation
  (stale ones are dropped), and selecting a hit runs `app._rs_palette_callback(n)`.
- `Lazy` / `Reveal` mount their widget at once (Textual defers it until after a refresh).
- Calls from worker threads are marshalled to the app thread automatically (Textual requires
  `call_from_thread`, which also works); each such call waits for the app thread, while
  `post_message` doesn't.

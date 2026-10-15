# Python bindings (`textual_rs`)

`crates/textual-py` is a PyO3 extension plus a pure-Python package that exposes a
**Textual-compatible Python API** on top of the textual-rs engine. Porting a Textual app is meant
to be a matter of replacing `textual` with `textual_rs` in its imports:

```python
from textual_rs.app import App, ComposeResult
from textual_rs.widgets import Button, Header, Footer
from textual_rs.reactive import reactive
from textual_rs import on, work
```

Layout, styling, rendering, compositing, input handling and the built-in widgets run in Rust;
Python code runs only in your handlers, `compose()`, `render()` and workers.

## Building and testing

```sh
cd crates/textual-py
uv sync                                  # builds the extension (maturin) into .venv
uv sync --reinstall-package textual-rs   # rebuild after changing Rust code
uv run pytest                            # Python tests (headless, via Pilot)
uv run python examples/counter.py        # run an example in the terminal
```

Python-only changes don't need a rebuild (the package is installed in editable mode).

## Architecture

```
python/textual_rs/            pure-Python API (App, Widget, reactive, messages, widgets, ...)
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
| none, another thread | `WrongThreadError`, turned into a `call_from_thread` round trip |

All native functions take explicit node ids, so they behave the same whichever frame is on top.

### GIL and threads

- `App.run()` runs the event loop inside `py.detach` (GIL released); every callback re-attaches
  with `Python::attach`. Python threads keep running while the UI is idle.
- Nothing blocks on another thread while holding the GIL. A worker thread calling a widget API
  posts the call to the app thread and waits on a `threading.Event` (which releases the GIL).
- Rust panics inside native functions become Python exceptions (PyO3); the frame stack unwinds
  with them, and the terminal is restored by the driver's panic hook.
- A Python exception in a handler is recorded, the app exits, and `run()` (or the next `Pilot`
  step) re-raises it, as in Textual.

### asyncio

Async handlers, async watchers and async workers run as tasks on an asyncio loop that lives on
the app thread, so they may touch widgets directly:

- `App.run()`: the loop is owned by `textual_rs` and pumped by the Rust event loop (each pump
  runs ready callbacks without blocking; while tasks are pending a 10 ms native timer keeps
  pumping, so `await asyncio.sleep()` and network I/O make progress).
- `App.run_test()`: the test's running loop is used; tasks start eagerly and `Pilot` methods
  yield to the loop until the app is idle.

Mount/remove/screen operations complete immediately and return awaitables that never block,
so `await self.mount(...)` works in sync and async code alike.

## Adding a widget binding

A built-in widget needs a Rust part (in `src/widgets/<group>.rs`) and a Python class
(in `python/textual_rs/widgets/_<name>.py`). Follow `src/widgets/basic.rs` and
`python/textual_rs/widgets/_button.py` / `_input.py`.

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
and `python/textual_rs/widgets/_detached.py`): the Python class mixes in `DetachedNative`, and
its methods use `self._call(...)` / `self._get(...)`. Before the widget is mounted these run on
a *detached* Rust widget (`_native.Detached`, built lazily from `_rs_kw`) which the kind
constructor takes over at mount time (the `_detached` kwarg); afterwards they go to the mounted
widget. The Rust method dispatcher receives `Option<&mut Ctx>` and uses ctx-free fallbacks
(e.g. `Log::push`, `Tree::set_expanded`) when there is no ctx.

## Differences from Textual

- Import from `textual_rs` instead of `textual`.
- `render()` may return a markup `str`, `textual_rs.content.Content` or a Rich `Text`
  (converted through its markup); other Rich renderables are rendered with `str()`.
  `render_line()` (the line API) isn't supported.
- `widget.styles` is write-mostly: setting any CSS property works (inline styles); reading
  returns what was set (and the computed `display` / `visibility`).
- Accessing the DOM from `compose()` or a widget constructor raises (Textual raises `NoMatches`).
- `log()` is a no-op (no devtools console).
- Calls from worker threads are marshalled to the app thread automatically (Textual requires
  `call_from_thread`, which also works).

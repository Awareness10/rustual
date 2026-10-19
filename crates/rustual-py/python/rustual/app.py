"""The ``App`` class (Textual's ``textual.app``)."""

from __future__ import annotations

import asyncio
import inspect
import os
import threading
from contextlib import asynccontextmanager
from pathlib import Path
from typing import Any, AsyncIterator, Callable, ClassVar, Generic, Iterable, NamedTuple, TypeVar, Union

from . import _bridge, _context
from ._bridge import _native, native
from .css.query import NoMatches, WrongType
from .geometry import Size
from .screen import ModalScreen, Screen
from .widget import AwaitComplete, AwaitMount, Widget, _invoke, _proxies, _selector, _widget_for

ReturnType = TypeVar("ReturnType")

ComposeResult = Iterable[Widget]
RenderResult = Any
try:
    from rich.console import RenderableType
except ImportError:  # rich is optional
    RenderableType = Any  # type: ignore[misc,assignment]
CSSPathType = Union[str, Path, list[Union[str, Path]]]


class AppError(Exception):
    pass


class SystemCommand(NamedTuple):
    """A command from :meth:`App.get_system_commands`."""

    title: str
    help: str
    callback: Callable[[], Any]
    discover: bool = True


def get_system_commands_provider() -> type:
    """The provider of :meth:`App.get_system_commands` (lazily imported, as in Textual)."""
    from .system_commands import SystemCommandsProvider

    return SystemCommandsProvider


class InvalidThemeError(ValueError):
    """Raised when setting ``App.theme`` to a theme that isn't registered."""


def _read_css_paths(cls: type, css_path: CSSPathType) -> list[str]:
    """Read CSS files; relative paths are relative to the file defining ``cls`` (like Textual)."""
    paths = css_path if isinstance(css_path, (list, tuple)) else [css_path]
    try:
        base = Path(inspect.getfile(cls)).parent
    except (TypeError, OSError):
        base = Path.cwd()
    out = []
    for p in paths:
        path = Path(p)
        if not path.is_absolute():
            path = base / path
        out.append(path.read_text(encoding="utf-8"))
    return out


def _screen_factory(value: Any) -> Callable[[], Screen[Any]]:
    if isinstance(value, Screen):
        return lambda: value
    return value


class App(Widget, Generic[ReturnType]):
    """The base class for Textual apps. Subclass it, then call :meth:`run`."""

    _rs_framework: ClassVar[bool] = True

    CSS: ClassVar[str] = ""
    CSS_PATH: ClassVar[CSSPathType | None] = None
    TITLE: ClassVar[str | None] = None
    SUB_TITLE: ClassVar[str | None] = None
    SCREENS: ClassVar[dict[str, Any]] = {}
    MODES: ClassVar[dict[str, Any]] = {}
    DEFAULT_MODE: ClassVar[str | None] = None
    AUTO_FOCUS: ClassVar[str | None] = "*"
    ENABLE_COMMAND_PALETTE: ClassVar[bool] = True
    COMMAND_PALETTE_BINDING: ClassVar[str] = "ctrl+p"
    HORIZONTAL_BREAKPOINTS: ClassVar[list[tuple[int, str]] | None] = []
    """``(min width, class)`` pairs: the screen gets the class of the largest width not above
    the terminal's (e.g. ``[(0, "-narrow"), (80, "-wide")]``)."""
    VERTICAL_BREAKPOINTS: ClassVar[list[tuple[int, str]] | None] = []
    """Like ``HORIZONTAL_BREAKPOINTS``, for the terminal height."""
    COMMANDS: ClassVar[set[Any]] = {get_system_commands_provider}
    """Command palette providers: ``Provider`` classes, or callables returning one."""
    NOTIFICATION_TIMEOUT: ClassVar[float] = 5

    def __init__(
        self,
        driver_class: Any = None,
        css_path: CSSPathType | None = None,
        watch_css: bool = False,
        ansi_color: bool | None = None,
    ) -> None:
        super().__init__()
        self._ansi_color = ansi_color
        self._title = self.TITLE if self.TITLE is not None else type(self).__name__
        self._sub_title = self.SUB_TITLE or ""
        self._css_path = css_path
        self._theme = os.environ.get("TEXTUAL_THEME")
        self._return_value: ReturnType | None = None
        self._return_code: int | None = None
        self._running = False
        self._themes: dict[str, Any] = {}

    # -- configuration ------------------------------------------------------------------------

    def _config(self) -> dict[str, Any]:
        css: list[tuple[str, str]] = []
        css_path = self._css_path if self._css_path is not None else self.CSS_PATH
        if css_path:
            for i, text in enumerate(_read_css_paths(type(self), css_path)):
                css.append((f"app-css-path-{i}", text))
        if self.CSS:
            css.append(("app-css", self.CSS))
        screens = []
        for name, value in self.SCREENS.items():
            modal = isinstance(value, type) and issubclass(value, ModalScreen) or isinstance(value, ModalScreen)
            screens.append((name, _screen_factory(value), modal))
            if isinstance(value, type) and issubclass(value, Screen):
                css.extend(value._rs_css())
        modes = []
        for name, value in self.MODES.items():
            if isinstance(value, str):
                target = self.SCREENS[value]
                value = target
            modes.append((name, _screen_factory(value)))
            if isinstance(value, type) and issubclass(value, Screen):
                css.extend(value._rs_css())
        return {
            "css": css,
            "title": self._title,
            "sub_title": self._sub_title,
            "theme": self._theme,
            "screens": screens,
            "modes": modes,
            "default_mode": self.DEFAULT_MODE,
            "themes": [t._native() for t in self._themes.values()],
            "command_palette": bool(self.ENABLE_COMMAND_PALETTE),
            "auto_focus": self.AUTO_FOCUS,
            "ansi_color": self._ansi_color,
            "horizontal_breakpoints": [(int(n), str(c)) for n, c in (self.HORIZONTAL_BREAKPOINTS or [])],
            "vertical_breakpoints": [(int(n), str(c)) for n, c in (self.VERTICAL_BREAKPOINTS or [])],
        }

    # -- running ------------------------------------------------------------------------------

    def run(
        self,
        *,
        headless: bool = False,
        inline: bool = False,
        inline_no_clear: bool = False,
        mouse: bool = True,
        size: tuple[int, int] | None = None,
        auto_pilot: Any = None,
    ) -> ReturnType | None:
        """Run the app in the terminal; returns the value passed to :meth:`exit`."""
        config = self._config()
        _context.set_active_app(self)
        runner = _context.AsyncRunner()
        _context.set_runner(runner)
        self._running = True
        try:
            _native.run_app(self, config)
        finally:
            self._running = False
            runner.close()
            _context.set_runner(None)
            _context.set_active_app(None)
            _proxies.clear()
        return self._return_value

    async def run_async(self, **kwargs: Any) -> ReturnType | None:
        """Run the app in a worker thread (the event loop stays free)."""
        loop = asyncio.get_running_loop()
        return await loop.run_in_executor(None, lambda: self.run(**kwargs))

    @asynccontextmanager
    async def run_test(
        self,
        *,
        headless: bool = True,
        size: tuple[int, int] | None = (80, 24),
        tooltips: bool = False,
        notifications: bool = False,
        message_hook: Any = None,
    ) -> AsyncIterator[Any]:
        """Run the app headlessly and yield a :class:`~rustual.pilot.Pilot` to drive it."""
        from .pilot import Pilot

        width, height = size or (80, 24)
        config = self._config()
        _context.set_active_app(self)
        runner = _context.AsyncRunner(asyncio.get_running_loop())
        _context.set_runner(runner)
        self._running = True
        handle = None
        try:
            handle = _native.start_test(self, config, width, height)
            pilot = Pilot(self, handle)
            await pilot._settle()
            yield pilot
        finally:
            self._running = False
            runner.close()
            if handle is not None:
                handle.close()
            _context.set_runner(None)
            _context.set_active_app(None)
            _proxies.clear()
        error = _native.take_error()
        if error is not None:
            raise error

    def exit(self, result: ReturnType | None = None, return_code: int = 0, message: Any = None) -> None:
        """Exit the app; :meth:`run` returns ``result``."""
        self._return_value = result
        self._return_code = return_code
        if self._running:
            native.exit()

    @property
    def return_value(self) -> ReturnType | None:
        return self._return_value

    @property
    def return_code(self) -> int | None:
        return self._return_code

    @property
    def is_running(self) -> bool:
        return self._running

    @property
    def debug(self) -> bool:
        """Debug mode (``TEXTUAL=debug``)."""
        return "debug" in {f.strip() for f in os.environ.get("TEXTUAL", "").split(",")}

    def clear_notifications(self) -> None:
        """Remove every notification."""
        native.clear_notifications()

    def set_keymap(self, keymap: dict[str, str]) -> None:
        """Replace the keymap (binding id → keys, e.g. ``{"save": "ctrl+s"}``)."""
        self._keymap = dict(keymap)
        if self._running:
            native.set_keymap(self._keymap, True)

    def update_keymap(self, keymap: dict[str, str]) -> None:
        """Merge entries into the keymap."""
        self._keymap = {**getattr(self, "_keymap", {}), **keymap}
        if self._running:
            native.set_keymap(dict(keymap), False)

    def get_key_display(self, binding: Any) -> str:
        """How a binding's key is shown (its ``key_display``, else e.g. ``ctrl+s`` → ``^s``)."""
        if getattr(binding, "key_display", None):
            return binding.key_display
        key = binding if isinstance(binding, str) else binding.key
        return _native.format_key(key.split(",")[0].strip())

    # -- command palette -----------------------------------------------------------------------

    def get_system_commands(self, screen: Screen[Any]) -> Iterable[SystemCommand]:
        """The system commands of the palette (theme, quit, keys, ...). Override to add your
        own: ``yield from super().get_system_commands(screen)`` then ``yield SystemCommand(...)``."""
        for title, help_text, action in native.system_commands():

            def run(action: str = action) -> None:
                self.run_action(action)

            yield SystemCommand(title, help_text, run)

    def action_command_palette(self) -> None:
        """Open the command palette."""
        if self.ENABLE_COMMAND_PALETTE:
            native.open_command_palette("Search for commands…", True)

    def search_commands(self, commands: Iterable[Any], placeholder: str = "Search for commands…") -> AwaitComplete:
        """Open a palette searching ``commands`` (``SimpleCommand`` or ``(name, callback[, help])``)."""
        from .command import CommandPalette, SimpleProvider

        return self.push_screen(CommandPalette(providers=[SimpleProvider(self.screen, list(commands))], placeholder=placeholder))

    def _open_command_palette(self, palette: Any) -> None:
        self.__dict__["_pending_palette_providers"] = palette.providers
        native.open_command_palette(palette.placeholder, palette.providers is None)

    def _palette_providers(self, sources: Any, screen: Screen[Any], match_style: Any) -> list[Any]:
        from .command import Provider, SimpleProvider

        providers = []
        for source in sources:
            if isinstance(source, SimpleProvider):
                providers.append(source(screen, match_style))
            elif inspect.isclass(source) and issubclass(source, Provider):
                providers.append(source(screen, match_style))
            else:
                providers.append(source()(screen, match_style))  # lazily loaded provider class
        return providers

    def _rs_command_search(self, event: str, palette: int, generation: int, query: str) -> None:
        """Drive the providers of an open palette (called by the runtime)."""
        state = self.__dict__.setdefault("_palette", {})
        if event == "opened":
            stack = self.screen_stack
            screen = stack[-2] if len(stack) > 1 else stack[-1]
            sources = self.__dict__.pop("_pending_palette_providers", None)
            if sources is None:
                sources = [*self.COMMANDS, *getattr(screen, "COMMANDS", ())]
            color, bgcolor, attrs = native.widget_style(palette, "command-palette--highlight", True)
            style = " ".join(name for name, on in attrs.items() if on) or "reverse"
            providers = self._palette_providers(sources, screen, style)
            for provider in providers:
                provider._post_init()
            state.update(palette=palette, providers=providers, task=None)
            self.__dict__["_palette_callbacks"] = {}
        elif event == "query":
            task = state.get("task")
            if task is not None:
                task.cancel()
            state["task"] = _context.run_coroutine(self._palette_search(palette, generation, query, state.get("providers", [])))
        elif event == "closed":
            task = state.pop("task", None)
            if task is not None:
                task.cancel()
            for provider in state.pop("providers", []):
                _context.run_coroutine(provider._shutdown())

    async def _palette_search(self, palette: int, generation: int, query: str, providers: list[Any]) -> None:
        from .command import Hit
        from .content import to_markup
        from .content import escape as escape_markup

        callbacks = self.__dict__.setdefault("_palette_callbacks", {})

        def native_hit(hit: Any) -> tuple:
            key = len(callbacks)
            callbacks[key] = hit.command
            text, is_markup = to_markup(hit.prompt)
            display = text if is_markup else escape_markup(text)
            score = hit.score if isinstance(hit, Hit) else None
            return (score, display, hit.text or "", hit.help or "", key)

        async def consume(provider: Any) -> None:
            batch: list[tuple] = []
            async for hit in provider._search(query):
                batch.append(native_hit(hit))
                if len(batch) >= 50:
                    native.palette_add_hits(palette, generation, batch)
                    batch = []
            if batch:
                native.palette_add_hits(palette, generation, batch)

        await asyncio.gather(*(consume(p) for p in providers))

    def action__rs_palette_callback(self, key: int) -> None:
        callback = self.__dict__.get("_palette_callbacks", {}).get(key)
        if callback is not None:
            result = callback()
            if inspect.isawaitable(result):
                _context.run_coroutine(result)

    def action_show_help_panel(self) -> None:
        native.help_panel(True)

    def action_hide_help_panel(self) -> None:
        native.help_panel(False)

    def call_from_thread(self, callback: Callable[..., Any], *args: Any, **kwargs: Any) -> Any:
        """Run ``callback`` on the app thread from another thread and return its result.

        As in Textual, an async callback is awaited (on the app's loop) and its result returned,
        and calling this on the app thread itself raises ``RuntimeError``. For fire-and-forget
        updates, ``post_message`` (thread-safe, never blocks) is cheaper.
        """
        if not getattr(self, "_running", False) or not _native.app_running():
            raise RuntimeError("App is not running")
        if _native.on_app_thread():
            raise RuntimeError("The `call_from_thread` method must run in a different thread from the app")
        done = threading.Event()
        box: dict[str, Any] = {}

        async def finish(awaitable: Any) -> None:
            try:
                box["result"] = await awaitable
            except BaseException as error:  # noqa: BLE001 - re-raised in the caller's thread
                box["error"] = error
            finally:
                done.set()

        def run() -> None:
            try:
                result = callback(*args, **kwargs)
            except BaseException as error:  # noqa: BLE001
                box["error"] = error
                done.set()
                return
            if inspect.isawaitable(result):
                _context.run_coroutine(finish(result))
            else:
                box["result"] = result
                done.set()

        if not _native.post_call(run):
            raise RuntimeError("App is not running")
        while not done.wait(0.05):
            if not _native.app_running():
                raise _bridge.NoActiveAppError("the app exited before the call completed")
        if "error" in box:
            raise box["error"]
        return box.get("result")

    # -- engine hooks ---------------------------------------------------------------------------

    def _rs_on_mount(self) -> None:
        if getattr(self, "_keymap", None):
            native.set_keymap(self._keymap, True)
        super()._rs_on_mount()
        from . import events

        self.call_later(lambda: self._dispatch(events.Ready()))

    # -- app-level properties -------------------------------------------------------------------

    @property
    def app(self) -> App[Any]:
        return self

    @property
    def title(self) -> str:
        return self._title

    def _set_app_text(self, which: str, value: str) -> None:
        if not self._running:
            return
        try:
            native.set_app_text(which, value)
        except RuntimeError:  # e.g. set from compose(); apply as soon as possible
            _context.defer(lambda: native.set_app_text(which, value))

    @title.setter
    def title(self, value: str) -> None:
        self._title = str(value)
        self._set_app_text("title", self._title)

    @property
    def sub_title(self) -> str:
        return self._sub_title

    @sub_title.setter
    def sub_title(self, value: str) -> None:
        self._sub_title = str(value)
        self._set_app_text("sub_title", self._sub_title)

    @property
    def theme(self) -> str:
        if self._running:
            try:
                return native.theme()
            except RuntimeError:  # e.g. in compose()
                pass
        return self._theme or "textual-dark"

    @theme.setter
    def theme(self, name: str) -> None:
        self._theme = name
        if not self._running:
            return
        try:
            ok = native.set_theme(name)
        except RuntimeError:  # e.g. set from compose(); apply as soon as possible
            _context.defer(lambda: native.set_theme(name))
            return
        if not ok:
            raise InvalidThemeError(f"unknown theme {name!r}")
        self.theme_changed_signal.publish(self._themes.get(name, name))

    @property
    def ansi_color(self) -> bool | None:
        """Native ANSI colors (``True``), truecolor (``False``), or the theme's choice (``None``)."""
        return self._ansi_color

    @ansi_color.setter
    def ansi_color(self, value: bool | None) -> None:
        self._ansi_color = value
        if self._running:
            native.set_ansi_color(value)

    @property
    def native_ansi_color(self) -> bool:
        """Whether ANSI colors are written natively (``ansi_color``, else the theme's ``ansi``)."""
        if self._running:
            try:
                return native.native_ansi_color()
            except RuntimeError:
                pass
        if self._ansi_color is not None:
            return self._ansi_color
        return bool(getattr(self.get_theme(self.theme), "ansi", False))

    @property
    def no_color(self) -> bool:
        """Whether ``NO_COLOR`` is set (colors are removed)."""
        return "NO_COLOR" in os.environ

    @property
    def theme_changed_signal(self) -> Any:
        """Published with the new theme whenever ``theme`` changes (Textual's ``theme_changed_signal``)."""
        signal = self.__dict__.get("_theme_changed_signal")
        if signal is None:
            from .signal import Signal

            signal = self.__dict__["_theme_changed_signal"] = Signal(self, "theme-changed")
        return signal

    @property
    def available_themes(self) -> dict[str, Any]:
        names = native.theme_names() if self._running else list(self._themes)
        return {name: self._themes.get(name, name) for name in names}

    def register_theme(self, theme: Any) -> None:
        """Register a :class:`~rustual.theme.Theme` (before or while running)."""
        self._themes[theme.name] = theme
        if self._running:
            native.register_theme(theme._native())

    def get_theme(self, theme_name: str) -> Any:
        """The :class:`~rustual.theme.Theme` named ``theme_name`` (registered or built in), or ``None``."""
        theme = self._themes.get(theme_name)
        if theme is None:
            from .theme import Theme

            fields = _native.builtin_theme(theme_name)
            theme = Theme(**fields) if fields is not None else None
        return theme

    @property
    def current_theme(self) -> Any:
        """The active :class:`~rustual.theme.Theme`."""
        return self.get_theme(self.theme)

    @property
    def size(self) -> Size:
        return Size(*native.app_size())

    @property
    def screen(self) -> Screen[Any]:
        return _widget_for(native.active_screen())

    @property
    def screen_stack(self) -> list[Screen[Any]]:
        return [_widget_for(n) for n in native.screen_stack()]

    @property
    def focused(self) -> Widget | None:
        node = native.focused()
        return None if node is None else _widget_for(node)

    def set_focus(self, widget: Widget | None, scroll_visible: bool = True) -> None:
        if widget is None:
            native.blur()
        else:
            widget.focus(scroll_visible)

    @property
    def clipboard(self) -> str:
        return native.clipboard()

    def copy_to_clipboard(self, text: str) -> None:
        native.copy_to_clipboard(text)

    def notify(
        self,
        message: str,
        *,
        title: str = "",
        severity: str = "information",
        timeout: float | None = None,
        markup: bool = True,
    ) -> None:
        native.notify(str(message), title, severity, timeout)

    def bell(self) -> None:
        native.bell()

    def open_url(self, url: str, *, new_tab: bool = True) -> None:
        """Open a URL in the browser (override to customise; used by ``Link`` and ``Markdown``)."""
        import webbrowser

        webbrowser.open(url, new=2 if new_tab else 0)

    # -- queries: the active screen first, then everything ---------------------------------------

    def _query_nodes(self, selector: str) -> list[Any]:
        if not self._running:
            return []
        screen = native.active_screen()
        nodes = native.query(screen, selector)
        seen = set(nodes)
        nodes += [n for n in native.query(native.root(), selector) if n not in seen]
        root = native.root()
        return [_widget_for(n) for n in nodes if n != root]

    def query_one(self, selector: Any, expect_type: type[Any] | None = None) -> Any:
        return super().query_one(selector, expect_type)

    # -- screens ---------------------------------------------------------------------------------

    def push_screen(
        self,
        screen: Screen[Any] | str,
        callback: Callable[..., Any] | None = None,
        wait_for_dismiss: bool = False,
    ) -> Any:
        """Push a screen; ``callback`` gets the ``dismiss()`` result.

        With ``wait_for_dismiss=True`` (from a worker or other async code), returns a future
        that resolves to the result.
        """
        future: asyncio.Future[Any] | None = None
        if wait_for_dismiss:
            try:
                loop = asyncio.get_running_loop()
            except RuntimeError:
                raise RuntimeError("push_screen(wait_for_dismiss=True) must be awaited from a worker or async handler") from None
            future = loop.create_future()
        from .command import CommandPalette

        if isinstance(screen, CommandPalette):
            self._open_command_palette(screen)
            return AwaitComplete()
        if isinstance(screen, str):
            node = native.named_screen("push", screen)
            target = _widget_for(node)
        else:
            if not isinstance(screen, Screen):
                raise TypeError(f"expected a Screen or screen name, got {screen!r}")
            screen._register_css()
            native.push_screen(native.root(), screen, screen.is_modal)
            target = screen
        if callback is not None or future is not None:

            def on_result(result: Any) -> None:
                if callback is not None:
                    _invoke(callback, result)
                if future is not None and not future.done():
                    future.set_result(result)

            target._result_callback = on_result
        return future if future is not None else AwaitMount([target])

    async def push_screen_wait(self, screen: Screen[Any] | str) -> Any:
        """Push a screen and wait for its ``dismiss()`` result (call from a worker)."""
        return await self.push_screen(screen, wait_for_dismiss=True)

    def pop_screen(self) -> AwaitComplete:
        native.pop_screen()
        return AwaitComplete()

    def switch_screen(self, screen: Screen[Any] | str) -> AwaitComplete:
        if isinstance(screen, str):
            native.named_screen("switch", screen)
        else:
            screen._register_css()
            native.switch_screen(screen)
        return AwaitComplete()

    def install_screen(self, screen: Screen[Any], name: str) -> None:
        screen._register_css()
        native.install_screen(screen, name)

    def uninstall_screen(self, screen: Screen[Any] | str) -> str | None:
        name = screen if isinstance(screen, str) else screen.name
        if name and native.uninstall_screen(name):
            return name
        return None

    def is_screen_installed(self, screen: Screen[Any] | str) -> bool:
        name = screen if isinstance(screen, str) else screen.name
        return bool(name) and native.is_screen_installed(name)

    def switch_mode(self, mode: str) -> AwaitComplete:
        native.switch_mode(mode)
        return AwaitComplete()

    @property
    def current_mode(self) -> str:
        return native.current_mode()

    # -- actions ---------------------------------------------------------------------------------

    def action_quit(self) -> None:
        self.exit()

    def action_bell(self) -> None:
        self.bell()

    def action_notify(self, message: str, title: str = "", severity: str = "information") -> None:
        self.notify(message, title=title, severity=severity)

    def action_push_screen(self, screen: str) -> None:
        self.push_screen(screen)

    def action_switch_screen(self, screen: str) -> None:
        self.switch_screen(screen)

    def action_pop_screen(self) -> None:
        self.pop_screen()

    def action_switch_mode(self, mode: str) -> None:
        self.switch_mode(mode)

    def action_focus_next(self) -> None:
        native.focus_next(True)

    def action_focus_previous(self) -> None:
        native.focus_next(False)

    def action_toggle_dark(self) -> None:
        self.theme = "textual-light" if self.theme == "textual-dark" else "textual-dark"


__all__ = [
    "App",
    "AppError",
    "ComposeResult",
    "RenderResult",
    "ReturnType",
    "NoMatches",
    "WrongType",
]

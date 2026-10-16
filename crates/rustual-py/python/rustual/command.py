"""Command palette providers (Textual's ``textual.command``).

A :class:`Provider` yields :class:`Hit`\\ s for a query (``search``) and :class:`DiscoveryHit`\\ s
for an empty input (``discover``); list provider classes in ``App.COMMANDS`` or
``Screen.COMMANDS``. Searches run as asyncio tasks while the user types; the palette shows hits
as they arrive, best score first.
"""

from __future__ import annotations

from abc import ABC, abstractmethod
from dataclasses import dataclass
from typing import TYPE_CHECKING, Any, AsyncIterator, Callable, Iterable, NamedTuple, Union

from .fuzzy import Matcher

if TYPE_CHECKING:
    from .app import App
    from .screen import Screen
    from .widget import Widget

IgnoreReturnCallbackType = Callable[[], Any]


def _plain(display: Any) -> str:
    """The text of a hit's display (for matching and sorting)."""
    if isinstance(display, str):
        return display
    plain = getattr(display, "plain", None)
    if isinstance(plain, str):
        return plain
    markup = getattr(display, "__rs_markup__", None)
    if isinstance(markup, str):
        from ._bridge import _native

        return _native.markup_plain(markup)
    return str(display)


@dataclass
class Hit:
    """A search result: ``score`` (from :meth:`Provider.matcher`), what to show, and the
    callback to run when it is selected."""

    score: float
    match_display: Any
    command: IgnoreReturnCallbackType
    text: str | None = None
    help: str | None = None

    @property
    def prompt(self) -> Any:
        return self.match_display

    def __lt__(self, other: object) -> bool:
        if isinstance(other, Hit):
            return self.score < other.score
        return NotImplemented

    def __eq__(self, other: object) -> bool:
        if isinstance(other, Hit):
            return self.score == other.score
        return NotImplemented

    def __post_init__(self) -> None:
        if self.text is None:
            self.text = _plain(self.match_display)


@dataclass
class DiscoveryHit:
    """A command shown while the input is empty (in the order providers yield them)."""

    display: Any
    command: IgnoreReturnCallbackType
    text: str | None = None
    help: str | None = None

    @property
    def prompt(self) -> Any:
        return self.display

    @property
    def score(self) -> float:
        return 0.0

    def __lt__(self, other: object) -> bool:
        if isinstance(other, DiscoveryHit):
            assert self.text is not None and other.text is not None
            return other.text < self.text
        return NotImplemented

    def __eq__(self, other: object) -> bool:
        if isinstance(other, Hit):
            return self.text == other.text
        return NotImplemented

    def __post_init__(self) -> None:
        if self.text is None:
            self.text = _plain(self.display)


Hits = AsyncIterator[Union[DiscoveryHit, Hit]]
"""What ``search`` / ``discover`` return."""

ProviderSource = Iterable[Union[type["Provider"], Callable[[], type["Provider"]]]]


class Provider(ABC):
    """Base class of command palette providers: implement :meth:`search` (and optionally
    :meth:`discover`, :meth:`startup`, :meth:`shutdown`)."""

    def __init__(self, screen: Screen[Any], match_style: Any = None) -> None:
        self.__screen = screen
        self.__match_style = match_style
        self._init_task: Any = None
        self._init_success = False

    @property
    def focused(self) -> Widget | None:
        """The focused widget of the screen the palette was opened from."""
        return self.__screen.focused

    @property
    def screen(self) -> Screen[object]:
        """The screen the palette was opened from."""
        return self.__screen

    @property
    def app(self) -> App[object]:
        return self.__screen.app

    @property
    def match_style(self) -> Any:
        """The style for highlighting matched letters (``match_display``)."""
        return self.__match_style

    def matcher(self, user_input: str, case_sensitive: bool = False) -> Matcher:
        """A fuzzy matcher for ``user_input`` (scores and highlights candidates)."""
        return Matcher(user_input, match_style=self.match_style, case_sensitive=case_sensitive)

    def _post_init(self) -> None:
        from . import _context

        async def post_init_task() -> None:
            try:
                await self.startup()
            except Exception:  # noqa: BLE001 - reported like Textual (logged)
                import traceback

                self.app.log.error(traceback.format_exc())
            else:
                self._init_success = True

        self._init_task = _context.run_coroutine(post_init_task())

    async def _wait_init(self) -> None:
        if self._init_task is not None:
            await self._init_task
        self._init_task = None

    async def startup(self) -> None:
        """Called after the provider is created, before any ``search``."""

    async def _search(self, query: str) -> Hits:
        await self._wait_init()
        if self._init_success:
            hits = self.search(query) if query else self.discover()
            async for hit in hits:
                if hit is not NotImplemented:
                    yield hit

    @abstractmethod
    async def search(self, query: str) -> Hits:
        """Yield :class:`Hit`\\ s for ``query``."""
        yield NotImplemented  # type: ignore[misc]

    async def discover(self) -> Hits:
        """Yield :class:`DiscoveryHit`\\ s to show while the input is empty (optional)."""
        yield NotImplemented  # type: ignore[misc]

    async def _shutdown(self) -> None:
        try:
            await self.shutdown()
        except Exception:  # noqa: BLE001
            import traceback

            self.app.log.error(traceback.format_exc())

    async def shutdown(self) -> None:
        """Called when the palette closes."""


class SimpleCommand(NamedTuple):
    """A command for :class:`SimpleProvider` / ``App.search_commands``."""

    name: str
    callback: IgnoreReturnCallbackType
    help_text: str | None = None


CommandListItem = Union[
    SimpleCommand,
    "tuple[str, IgnoreReturnCallbackType, str | None]",
    "tuple[str, IgnoreReturnCallbackType]",
]


class SimpleProvider(Provider):
    """A provider for a fixed list of commands."""

    def __init__(self, screen: Screen[Any], commands: list[CommandListItem]) -> None:
        super().__init__(screen, None)
        self._commands: list[SimpleCommand] = []
        for command in commands:
            if isinstance(command, SimpleCommand):
                self._commands.append(command)
            elif len(command) == 2:
                self._commands.append(SimpleCommand(*command, None))
            elif len(command) == 3:
                self._commands.append(SimpleCommand(*command))
            else:
                raise ValueError(f"Invalid command: {command}")
        self.__match_style: Any = None

    def __call__(self, screen: Screen[Any], match_style: Any = None) -> SimpleProvider:
        self.__match_style = match_style
        return self

    @property
    def match_style(self) -> Any:
        return self.__match_style

    async def search(self, query: str) -> Hits:
        matcher = self.matcher(query)
        for name, callback, help_text in self._commands:
            if (match := matcher.match(name)) > 0:
                yield Hit(match, matcher.highlight(name), callback, help=help_text)

    async def discover(self) -> Hits:
        for name, callback, help_text in self._commands:
            yield DiscoveryHit(name, callback, help=help_text)


class CommandPalette:
    """The command palette. ``app.push_screen(CommandPalette(providers=[...]))`` opens it with
    the given providers (default: ``App.COMMANDS`` and the screen's ``COMMANDS``)."""

    def __init__(
        self,
        providers: ProviderSource | None = None,
        *,
        placeholder: str = "Search for commands…",
        name: str | None = None,
        id: str | None = None,
        classes: str | None = None,
    ) -> None:
        self.providers = providers
        self.placeholder = placeholder

    @staticmethod
    def is_open(app: App[object]) -> bool:
        """Whether a command palette is open."""
        try:
            return app.screen.has_class("--textual-command-palette")
        except Exception:  # noqa: BLE001 - no active screen
            return False


__all__ = [
    "CommandListItem",
    "CommandPalette",
    "DiscoveryHit",
    "Hit",
    "Hits",
    "Matcher",
    "Provider",
    "SimpleCommand",
    "SimpleProvider",
]

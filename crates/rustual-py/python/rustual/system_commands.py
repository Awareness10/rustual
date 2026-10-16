"""``SystemCommandsProvider`` (Textual's ``textual.system_commands``): the commands from
``App.get_system_commands`` (theme, quit, keys, maximize, screenshot, ...)."""

from __future__ import annotations

from .command import DiscoveryHit, Hit, Hits, Provider


class SystemCommandsProvider(Provider):
    """Provides ``App.get_system_commands`` (included in ``App.COMMANDS`` by default)."""

    async def discover(self) -> Hits:
        commands = sorted(self.app.get_system_commands(self.screen), key=lambda command: command[0])
        for name, help_text, callback, discover in commands:
            if discover:
                yield DiscoveryHit(name, callback, help=help_text)

    async def search(self, query: str) -> Hits:
        matcher = self.matcher(query)
        for name, help_text, callback, *_ in self.app.get_system_commands(self.screen):
            if (match := matcher.match(name)) > 0:
                yield Hit(match, matcher.highlight(name), callback, help=help_text)


__all__ = ["SystemCommandsProvider"]

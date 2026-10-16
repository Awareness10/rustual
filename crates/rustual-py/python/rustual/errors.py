"""General exceptions (Textual's ``textual.errors``)."""


class TextualError(Exception):
    """Base class for Textual errors."""


class NoWidget(TextualError):
    """Raised when there is no widget to query."""


class RenderError(TextualError):
    """An object could not be rendered."""


class DuplicateKeyHandlers(TextualError):
    """More than one handler for a single key press."""


__all__ = ["TextualError", "NoWidget", "RenderError", "DuplicateKeyHandlers"]

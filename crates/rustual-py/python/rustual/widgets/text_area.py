"""Types used with :class:`~rustual.widgets.TextArea` (Textual's ``textual.widgets.text_area``)."""

from ._text_area import (
    BUILTIN_LANGUAGES,
    Document,
    EditResult,
    LanguageDoesNotExist,
    Location,
    Selection,
    ThemeDoesNotExist,
)

__all__ = [
    "BUILTIN_LANGUAGES",
    "Document",
    "EditResult",
    "LanguageDoesNotExist",
    "Location",
    "Selection",
    "ThemeDoesNotExist",
]

"""Types used with :class:`~textual_rs.widgets.TextArea` (Textual's ``textual.widgets.text_area``)."""

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

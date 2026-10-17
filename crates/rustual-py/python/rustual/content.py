"""Content helpers (a minimal subset of Textual's ``textual.content``)."""

from __future__ import annotations

from typing import Any


class Content:
    """Text content built from markup (``Content.from_markup``) or plain text."""

    def __init__(self, text: str = "", *, markup: str | None = None) -> None:
        self._plain = text
        self._markup = markup if markup is not None else escape(text)

    @classmethod
    def from_markup(cls, markup: str, **variables: Any) -> Content:
        if variables:
            markup = markup.format(**variables)
        from ._bridge import _native

        content = cls.__new__(cls)
        content._markup = markup
        content._plain = _native.markup_plain(markup)
        return content

    @property
    def plain(self) -> str:
        return self._plain

    @property
    def __rs_markup__(self) -> str:
        return self._markup

    def __str__(self) -> str:
        return self._plain

    def __repr__(self) -> str:
        return f"Content({self._markup!r})"


def escape(text: str) -> str:
    """Escape markup characters so ``text`` renders literally."""
    return text.replace("[", r"\[")


def to_markup(content: Any, markup: bool = True) -> tuple[str, bool]:
    """Normalize widget content to ``(text, is_markup)``."""
    if content is None:
        return "", markup
    if isinstance(content, str):
        return content, markup
    rs = getattr(content, "__rs_markup__", None)
    if isinstance(rs, str):
        return rs, True
    if hasattr(content, "spans") and hasattr(content, "plain"):
        return _rich_text_markup(content), True
    rich_markup = getattr(content, "markup", None)
    if isinstance(rich_markup, str) and hasattr(content, "plain"):
        return rich_markup, True
    return str(content), False


_ANSI_NAMES = ("black", "red", "green", "yellow", "blue", "magenta", "cyan", "white")


def _rich_color(color: Any) -> str | None:
    """A Rich color as a rustual color word: the 16 standard colors become ``ansi_*`` (so they
    follow the ANSI theme, like Textual), other colors become hex."""
    if color is None or color.is_default:
        return None
    from rich.color import ColorType

    if color.type == ColorType.STANDARD and color.number is not None and color.number < 16:
        name = _ANSI_NAMES[color.number % 8]
        return f"ansi_{'bright_' if color.number >= 8 else ''}{name}"
    return color.get_truecolor().hex


def _rich_style_markup(style: Any) -> str:
    """A Rich style (or style name such as ``repr.str``) as rustual markup style words."""
    from rich.style import Style as RichStyle

    if isinstance(style, str):
        from rich.default_styles import DEFAULT_STYLES

        try:
            style = DEFAULT_STYLES.get(style) or RichStyle.parse(style)
        except Exception:
            return ""
    words = []
    for attr in ("bold", "dim", "italic", "underline", "strike", "reverse"):
        value = getattr(style, attr)
        if value is not None:
            words.append(attr if value else f"not {attr}")
    fg = _rich_color(style.color)
    if fg:
        words.append(fg)
    bg = _rich_color(style.bgcolor)
    if bg:
        words.append(f"on {bg}")
    return " ".join(words)


def _rich_text_markup(text: Any) -> str:
    """Markup for a Rich ``Text``, with theme style names resolved (Textual resolves them
    through the console theme in ``Content.from_rich_text``)."""
    from rich.text import Span, Text

    resolved = Text(text.plain)
    spans = []
    if text.style:
        spans.append(Span(0, len(text.plain), text.style))
    spans.extend(text.spans)
    for span in spans:
        style = _rich_style_markup(span.style)
        if style and span.end > span.start:
            resolved.spans.append(Span(span.start, span.end, style))
    return resolved.markup

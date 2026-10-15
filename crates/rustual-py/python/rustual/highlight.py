"""Syntax highlighting (Textual's ``textual.highlight``), returning ``Content``.

Uses Pygments for tokenizing and Textual's ``HighlightTheme`` (theme variables such as
``$text-accent``), so highlighted code follows the app's theme and is rendered natively.
"""

from __future__ import annotations

import os
from typing import Tuple

from .content import Content, escape

TokenType = Tuple[str, ...]


def _token_styles() -> dict[TokenType, str]:
    from pygments.token import Token

    return {
        Token.Comment: "$text 60%",
        Token.Error: "$text-error on $error-muted",
        Token.Generic.Strong: "bold",
        Token.Generic.Emph: "italic",
        Token.Generic.Error: "$text-error on $error-muted",
        Token.Generic.Heading: "$text-primary underline",
        Token.Generic.Subheading: "$text-primary",
        Token.Keyword: "$text-accent",
        Token.Keyword.Constant: "bold $text-success 80%",
        Token.Keyword.Namespace: "$text-error",
        Token.Keyword.Type: "bold",
        Token.Literal.Number: "$text-warning",
        Token.Literal.String.Backtick: "$text 60%",
        Token.Literal.String: "$text-success 90%",
        Token.Literal.String.Doc: "$text-success 80% italic",
        Token.Literal.String.Double: "$text-success 90%",
        Token.Name: "$text-primary",
        Token.Name.Attribute: "$text-warning",
        Token.Name.Builtin: "$text-accent",
        Token.Name.Builtin.Pseudo: "italic",
        Token.Name.Class: "$text-warning bold",
        Token.Name.Constant: "$text-error",
        Token.Name.Decorator: "$text-primary bold",
        Token.Name.Function: "$text-warning underline",
        Token.Name.Function.Magic: "$text-warning underline",
        Token.Name.Tag: "$text-primary bold",
        Token.Name.Variable: "$text-secondary",
        Token.Number: "$text-warning",
        Token.Operator: "bold",
        Token.Operator.Word: "bold $text-error",
        Token.String: "$text-success",
        Token.Whitespace: "",
    }


class HighlightTheme:
    """Token → style mapping used by :func:`highlight`."""

    @classmethod
    def styles(cls) -> dict[TokenType, str]:
        return _token_styles()


def guess_language(code: str, path: str | None) -> str:
    """Guess the Pygments language name from the code and/or path."""
    from pygments.lexers import get_lexer_by_name, guess_lexer, guess_lexer_for_filename
    from pygments.util import ClassNotFound

    if path and os.path.splitext(path)[-1] == ".tcss":
        return "scss"
    lexer = None
    if code:
        if path:
            try:
                lexer = guess_lexer_for_filename(path, code)
            except ClassNotFound:
                pass
        if lexer is None:
            try:
                lexer = guess_lexer(code)
            except Exception:  # noqa: BLE001
                pass
    if not lexer and path:
        try:
            ext = os.path.splitext(path)[1].lstrip(".").lower()
            if ext:
                lexer = get_lexer_by_name(ext)
        except ClassNotFound:
            pass
    return lexer.name.lower() if lexer else "default"


def highlight(
    code: str,
    *,
    language: str | None = None,
    path: str | None = None,
    theme: type[HighlightTheme] = HighlightTheme,
    tab_size: int = 8,
) -> Content:
    """Highlight ``code`` and return :class:`~rustual.content.Content` (markup with theme variables)."""
    from pygments.lexers import get_lexer_by_name
    from pygments.util import ClassNotFound

    if language is None:
        language = guess_language(code, path)
    try:
        lexer = get_lexer_by_name(language or "text", stripnl=False, ensurenl=True, tabsize=tab_size)
    except ClassNotFound:
        lexer = get_lexer_by_name("text", stripnl=False, ensurenl=True, tabsize=tab_size)
    styles = theme.styles()
    parts: list[str] = []
    for token_type, token in lexer.get_tokens(code):
        style = ""
        while token_type:
            style = styles.get(token_type, "")
            if style or token_type in styles:
                break
            token_type = token_type.parent
        text = escape(token)
        parts.append(f"[{style}]{text}[/]" if style else text)
    markup = "".join(parts).rstrip("\n")
    content = Content.from_markup(markup)
    content._plain = code.rstrip("\n")
    return content

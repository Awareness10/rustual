"""Built-in widgets (Textual's ``textual.widgets``), backed by the Rust implementations."""

from ._button import Button
from ._collapsible import Collapsible
from ._content_switcher import ContentSwitcher
from ._footer import Footer
from ._header import Header
from ._help_panel import HelpPanel
from ._input import Input
from ._key_panel import KeyPanel
from ._label import Label
from ._link import Link
from ._loading_indicator import LoadingIndicator
from ._markdown import Markdown, MarkdownViewer
from ._placeholder import Placeholder
from ._rule import Rule
from ._static import Static
from ._tabbed_content import TabbedContent, TabPane
from ._tabs import Tab, Tabs
from ._text_area import TextArea
from ._welcome import Welcome
from ..widget import Widget

__all__ = [
    "Button",
    "Collapsible",
    "ContentSwitcher",
    "Footer",
    "Header",
    "HelpPanel",
    "Input",
    "KeyPanel",
    "Label",
    "Link",
    "LoadingIndicator",
    "Markdown",
    "MarkdownViewer",
    "Placeholder",
    "Rule",
    "Static",
    "Tab",
    "TabbedContent",
    "TabPane",
    "Tabs",
    "TextArea",
    "Welcome",
    "Widget",
]

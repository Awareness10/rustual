"""Notification types (Textual's ``textual.notifications``)."""

from __future__ import annotations

from typing import Literal

SeverityLevel = Literal["information", "warning", "error"]
"""The severity level of a notification."""

__all__ = ["SeverityLevel"]

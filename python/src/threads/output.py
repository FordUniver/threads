"""Output formatting utilities with TTY auto-detection."""

import os
import sys
from enum import Enum


class OutputFormat(Enum):
    """Output format for commands."""

    FANCY = "fancy"
    PLAIN = "plain"
    JSON = "json"
    YAML = "yaml"


def parse_format(s: str) -> OutputFormat:
    """Parse a format string into OutputFormat."""
    try:
        return OutputFormat(s.lower())
    except ValueError:
        return OutputFormat.FANCY


def is_tty() -> bool:
    """Check if stdout is connected to a terminal."""
    return sys.stdout.isatty()


def resolve_format(fmt: OutputFormat) -> OutputFormat:
    """Resolve the output format, applying TTY auto-detection.

    If format is FANCY but stdout is not a TTY, returns PLAIN.
    """
    if fmt == OutputFormat.FANCY and not is_tty():
        return OutputFormat.PLAIN
    return fmt

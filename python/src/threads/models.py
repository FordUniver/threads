"""Data models for threads."""

from dataclasses import dataclass, field
from pathlib import Path

# Valid status values
ACTIVE_STATUSES = frozenset({"idea", "planning", "active", "blocked", "paused"})
TERMINAL_STATUSES = frozenset({"resolved", "superseded", "deferred"})
ALL_STATUSES = ACTIVE_STATUSES | TERMINAL_STATUSES


@dataclass
class Note:
    """A note entry with hash identifier."""

    text: str
    hash: str  # 4-char sha256 prefix


@dataclass
class Todo:
    """A todo item with hash identifier."""

    text: str
    hash: str  # 4-char sha256 prefix
    checked: bool = False


@dataclass
class LogEntry:
    """A timestamped log entry."""

    time: str  # HH:MM format
    text: str


@dataclass
class Thread:
    """A thread with frontmatter metadata and sections."""

    # Frontmatter fields
    id: str  # 6-char hex
    name: str  # Human-readable title
    desc: str = ""  # One-line description
    status: str = "idea"

    # Sections (parsed from content)
    body: str = ""
    notes: list[Note] = field(default_factory=list)
    todos: list[Todo] = field(default_factory=list)
    log: dict[str, list[LogEntry]] = field(default_factory=dict)  # date -> entries

    # File metadata (set after loading)
    file_path: Path | None = None
    category: str | None = None  # "-" means workspace level
    project: str | None = None  # "-" means category level

    def is_terminal(self) -> bool:
        """Check if thread has a terminal status."""
        base = self.status.split(" (")[0]  # Strip reason suffix like "blocked (waiting)"
        return base in TERMINAL_STATUSES

    def base_status(self) -> str:
        """Get status without reason suffix."""
        return self.status.split(" (")[0]

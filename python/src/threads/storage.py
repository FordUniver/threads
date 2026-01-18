"""File I/O and section parsing for threads."""

import os
import re
import tempfile
from dataclasses import dataclass, field
from pathlib import Path
from typing import Optional

import frontmatter

from .models import LogEntry, Note, Thread, Todo


@dataclass
class FindOptions:
    """Options for finding threads with direction and boundary controls."""

    # Down depth: None = no recursion, -1 = unlimited, N = N levels
    down: Optional[int] = None
    # Up depth: None = no up search, -1 = unlimited (to git root), N = N levels
    up: Optional[int] = None
    # Cross git boundaries when searching down
    no_git_bound_down: bool = False
    # Cross git boundaries when searching up
    no_git_bound_up: bool = False

    @classmethod
    def new(cls) -> "FindOptions":
        return cls()

    def with_down(self, depth: Optional[int]) -> "FindOptions":
        self.down = depth
        return self

    def with_up(self, depth: Optional[int]) -> "FindOptions":
        self.up = depth
        return self

    def with_no_git_bound_down(self, value: bool) -> "FindOptions":
        self.no_git_bound_down = value
        return self

    def with_no_git_bound_up(self, value: bool) -> "FindOptions":
        self.no_git_bound_up = value
        return self

    def has_down(self) -> bool:
        return self.down is not None

    def has_up(self) -> bool:
        return self.up is not None

# Section header pattern
SECTION_RE = re.compile(r"^## (\w+)\s*$", re.MULTILINE)

# Note pattern: - text  <!-- hash -->
NOTE_RE = re.compile(r"^- (.+?)\s*<!--\s*([a-f0-9]{4})\s*-->$")

# Todo pattern: - [ ] or - [x] text  <!-- hash -->
TODO_RE = re.compile(r"^- \[([ x])\] (.+?)\s*<!--\s*([a-f0-9]{4})\s*-->$")

# Log date heading: ### YYYY-MM-DD
LOG_DATE_RE = re.compile(r"^### (\d{4}-\d{2}-\d{2})\s*$")

# Log entry: - **HH:MM** text
LOG_ENTRY_RE = re.compile(r"^- \*\*(\d{2}:\d{2})\*\* (.+)$")


def parse_sections(content: str) -> dict[str, str]:
    """Split markdown content into {section_name: content}."""
    sections: dict[str, str] = {}
    matches = list(SECTION_RE.finditer(content))

    for i, match in enumerate(matches):
        name = match.group(1)
        start = match.end()
        end = matches[i + 1].start() if i + 1 < len(matches) else len(content)
        sections[name] = content[start:end].strip()

    return sections


def parse_notes(text: str) -> list[Note]:
    """Parse Notes section into Note objects."""
    notes = []
    for line in text.split("\n"):
        line = line.strip()
        if not line:
            continue
        m = NOTE_RE.match(line)
        if m:
            notes.append(Note(text=m.group(1), hash=m.group(2)))
    return notes


def parse_todos(text: str) -> list[Todo]:
    """Parse Todo section into Todo objects."""
    todos = []
    for line in text.split("\n"):
        line = line.strip()
        if not line:
            continue
        m = TODO_RE.match(line)
        if m:
            todos.append(Todo(text=m.group(2), hash=m.group(3), checked=m.group(1) == "x"))
    return todos


def parse_log(text: str) -> dict[str, list[LogEntry]]:
    """Parse Log section into {date: [LogEntry]}."""
    log: dict[str, list[LogEntry]] = {}
    current_date: str | None = None

    for line in text.split("\n"):
        line = line.rstrip()
        if not line:
            continue

        date_match = LOG_DATE_RE.match(line)
        if date_match:
            current_date = date_match.group(1)
            if current_date not in log:
                log[current_date] = []
            continue

        if current_date:
            entry_match = LOG_ENTRY_RE.match(line)
            if entry_match:
                log[current_date].append(LogEntry(time=entry_match.group(1), text=entry_match.group(2)))

    return log


def load_thread(path: Path) -> Thread:
    """Load a thread from a markdown file."""
    post = frontmatter.load(path)

    # Parse sections from content
    sections = parse_sections(post.content)

    # Build Thread object
    thread = Thread(
        id=post.get("id", ""),
        name=post.get("name", post.get("title", "")),
        desc=post.get("desc", ""),
        status=post.get("status", "idea"),
        body=sections.get("Body", ""),
        notes=parse_notes(sections.get("Notes", "")),
        todos=parse_todos(sections.get("Todo", "")),
        log=parse_log(sections.get("Log", "")),
        file_path=path,
    )

    return thread


def serialize_notes(notes: list[Note]) -> str:
    """Serialize notes to markdown."""
    if not notes:
        return ""
    lines = []
    for note in notes:
        lines.append(f"- {note.text}  <!-- {note.hash} -->")
    return "\n".join(lines)


def serialize_todos(todos: list[Todo]) -> str:
    """Serialize todos to markdown."""
    if not todos:
        return ""
    lines = []
    for todo in todos:
        check = "x" if todo.checked else " "
        lines.append(f"- [{check}] {todo.text}  <!-- {todo.hash} -->")
    return "\n".join(lines)


def serialize_log(log: dict[str, list[LogEntry]]) -> str:
    """Serialize log to markdown."""
    if not log:
        return ""
    lines = []
    # Sort dates descending (most recent first)
    for date in sorted(log.keys(), reverse=True):
        lines.append(f"### {date}")
        lines.append("")
        for entry in log[date]:
            lines.append(f"- **{entry.time}** {entry.text}")
    return "\n".join(lines)


def serialize_thread(thread: Thread) -> str:
    """Serialize thread to markdown string."""
    # Build frontmatter
    metadata = {
        "id": thread.id,
        "name": thread.name,
        "desc": thread.desc,
        "status": thread.status,
    }

    # Build sections
    sections = []

    # Body section
    sections.append("## Body")
    sections.append("")
    if thread.body:
        sections.append(thread.body)
        sections.append("")

    # Notes section (only if notes exist)
    if thread.notes:
        sections.append("## Notes")
        sections.append("")
        sections.append(serialize_notes(thread.notes))
        sections.append("")

    # Todo section
    sections.append("## Todo")
    sections.append("")
    if thread.todos:
        sections.append(serialize_todos(thread.todos))
        sections.append("")

    # Log section
    sections.append("## Log")
    sections.append("")
    if thread.log:
        sections.append(serialize_log(thread.log))

    content = "\n".join(sections)

    # Use frontmatter to serialize
    post = frontmatter.Post(content, **metadata)
    return frontmatter.dumps(post)


def save_thread(thread: Thread) -> None:
    """Save thread to its file path atomically.

    Uses write-to-temp-then-rename pattern to ensure the file is never
    left in a partially-written state.
    """
    if thread.file_path is None:
        raise ValueError("Thread has no file_path set")

    content = serialize_thread(thread)
    dir_path = thread.file_path.parent

    # Write to temp file in same directory, then atomic rename
    with tempfile.NamedTemporaryFile(
        mode="w", dir=dir_path, delete=False, suffix=".tmp"
    ) as f:
        f.write(content)
        temp_path = f.name

    os.rename(temp_path, thread.file_path)  # Atomic on POSIX


def _is_git_root(path: Path) -> bool:
    """Check if a directory contains a .git folder."""
    return (path / ".git").is_dir()


def _find_threads_recursive(
    dir_path: Path, git_root: Path, threads: list[Path]
) -> None:
    """Recursively find .threads directories and collect thread files.

    Stops at nested git repositories (directories containing .git).
    """
    # Check for .threads directory here
    threads_dir = dir_path / ".threads"
    if threads_dir.is_dir():
        for entry in threads_dir.iterdir():
            if entry.is_file() and entry.suffix == ".md":
                # Skip archive subdirectory
                if "/archive/" not in str(entry):
                    threads.append(entry)

    # Recurse into subdirectories
    try:
        for entry in dir_path.iterdir():
            if not entry.is_dir():
                continue

            name = entry.name

            # Skip hidden directories (except we already handled .threads)
            if name.startswith("."):
                continue

            # Stop at nested git repos (unless it's the root itself)
            if entry != git_root and _is_git_root(entry):
                continue

            _find_threads_recursive(entry, git_root, threads)
    except PermissionError:
        pass  # Silently skip unreadable directories


def find_threads(git_root: Path) -> list[Path]:
    """Find all thread files in git repository.

    Scans recursively, respecting git boundaries (stops at nested git repos).
    """
    threads: list[Path] = []
    _find_threads_recursive(git_root, git_root, threads)

    # Sort by modification time (most recent first)
    return sorted(threads, key=lambda p: p.stat().st_mtime, reverse=True)


def _collect_threads_at_path(dir_path: Path, threads: list[Path]) -> None:
    """Collect threads from .threads directory at the given path."""
    threads_dir = dir_path / ".threads"
    if threads_dir.is_dir():
        for entry in threads_dir.iterdir():
            if entry.is_file() and entry.suffix == ".md":
                # Skip archive subdirectory
                if "/archive/" not in str(entry):
                    threads.append(entry)


def _find_threads_down(
    dir_path: Path,
    git_root: Path,
    threads: list[Path],
    current_depth: int,
    max_depth: int,  # -1 = unlimited
    cross_git_boundaries: bool,
) -> None:
    """Recursively find threads going down into subdirectories."""
    # Check depth limit
    if max_depth >= 0 and current_depth >= max_depth:
        return

    try:
        for entry in dir_path.iterdir():
            if not entry.is_dir():
                continue

            name = entry.name

            # Skip hidden directories
            if name.startswith("."):
                continue

            # Check git boundary
            if not cross_git_boundaries and entry != git_root and _is_git_root(entry):
                continue

            # Collect threads at this level
            _collect_threads_at_path(entry, threads)

            # Continue recursing
            _find_threads_down(
                entry, git_root, threads, current_depth + 1, max_depth, cross_git_boundaries
            )
    except PermissionError:
        pass


def _find_threads_up(
    dir_path: Path,
    git_root: Path,
    threads: list[Path],
    current_depth: int,
    max_depth: int,  # -1 = unlimited
    cross_git_boundaries: bool,
) -> None:
    """Find threads going up into parent directories."""
    # Check depth limit
    if max_depth >= 0 and current_depth >= max_depth:
        return

    parent = dir_path.parent
    if parent == dir_path:
        return  # reached filesystem root

    parent_resolved = parent.resolve()
    git_root_resolved = git_root.resolve()

    # Check git boundary: stop at git root unless crossing is allowed
    if not cross_git_boundaries:
        try:
            parent_resolved.relative_to(git_root_resolved)
        except ValueError:
            return  # parent is outside git root

    # Collect threads at parent
    _collect_threads_at_path(parent_resolved, threads)

    # Continue up
    _find_threads_up(
        parent_resolved, git_root, threads, current_depth + 1, max_depth, cross_git_boundaries
    )


def find_threads_with_options(
    start_path: Path, git_root: Path, options: FindOptions
) -> list[Path]:
    """Find threads with options for direction and boundary controls.

    This is the primary search function supporting --up, --down, and boundary flags.
    """
    threads: list[Path] = []
    start_resolved = start_path.resolve()

    # Always collect threads at start_path
    _collect_threads_at_path(start_resolved, threads)

    # Search down (subdirectories)
    if options.has_down():
        max_depth = options.down if options.down is not None else -1
        _find_threads_down(
            start_resolved, git_root, threads, 0, max_depth, options.no_git_bound_down
        )

    # Search up (parent directories)
    if options.has_up():
        max_depth = options.up if options.up is not None else -1
        _find_threads_up(
            start_resolved, git_root, threads, 0, max_depth, options.no_git_bound_up
        )

    # Sort and deduplicate
    threads = sorted(set(threads), key=lambda p: p.stat().st_mtime, reverse=True)
    return threads

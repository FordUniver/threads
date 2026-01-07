"""File I/O and section parsing for threads."""

import re
from pathlib import Path

import frontmatter

from .models import LogEntry, Note, Thread, Todo

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
    """Save thread to its file path."""
    if thread.file_path is None:
        raise ValueError("Thread has no file_path set")

    content = serialize_thread(thread)
    thread.file_path.write_text(content)


def find_threads(workspace: Path) -> list[Path]:
    """Find all thread files in workspace."""
    threads = []
    # Check workspace, category, and project levels
    patterns = [
        workspace / ".threads" / "*.md",
        workspace / "*" / ".threads" / "*.md",
        workspace / "*" / "*" / ".threads" / "*.md",
    ]
    for pattern in patterns:
        threads.extend(pattern.parent.glob(pattern.name) if pattern.parent.exists() else [])

    # Filter out archive directories
    threads = [t for t in threads if "/archive/" not in str(t)]

    return sorted(threads, key=lambda p: p.stat().st_mtime, reverse=True)

"""CRUD commands: new, remove, update."""

import re
import sys
from datetime import datetime
from pathlib import Path

from ..models import LogEntry, Thread, validate_status
from ..storage import find_threads, save_thread, load_thread
from ..workspace import find_thread_by_ref, get_workspace, infer_scope


def generate_id(workspace: Path) -> str:
    """Generate a unique 6-char hex ID."""
    import secrets

    # Collect existing IDs
    existing_ids = set()
    for path in find_threads(workspace):
        filename = path.stem
        if re.match(r"^[0-9a-f]{6}-", filename):
            existing_ids.add(filename[:6])

    # Generate unique ID
    for _ in range(10):
        candidate = secrets.token_hex(3)
        if candidate not in existing_ids:
            return candidate

    raise RuntimeError("Could not generate unique ID after 10 attempts")


def slugify(title: str) -> str:
    """Convert title to kebab-case slug."""
    slug = title.lower()
    slug = re.sub(r"[^a-z0-9]", "-", slug)
    slug = re.sub(r"-+", "-", slug)
    slug = slug.strip("-")
    return slug


def cmd_new(
    title: str,
    path: str | None = None,
    desc: str = "",
    status: str = "idea",
    body: str | None = None,
    do_commit: bool = False,
    message: str | None = None,
) -> str:
    """Create a new thread.

    Returns:
        The generated thread ID.
    """
    # Validate status before proceeding
    validate_status(status)

    workspace = get_workspace()

    # Warn if no description
    if not desc:
        print("Warning: No --desc provided. Add one with: thread <id> update --desc \"...\"", file=sys.stderr)

    # Determine scope
    scope = infer_scope(path, workspace)

    # Generate ID and filename
    tid = generate_id(workspace)
    slug = slugify(title)
    if not slug:
        raise ValueError("Title produces empty slug")

    # Ensure threads directory exists
    scope.threads_dir.mkdir(parents=True, exist_ok=True)

    filepath = scope.threads_dir / f"{tid}-{slug}.md"
    if filepath.exists():
        raise ValueError(f"Thread already exists: {filepath}")

    # Read body from stdin if not provided and stdin has data
    if body is None and not sys.stdin.isatty():
        import select
        # Only read if stdin has data available (non-blocking check)
        if select.select([sys.stdin], [], [], 0.0)[0]:
            body = sys.stdin.read()

    # Create thread
    now = datetime.now()
    today = now.strftime("%Y-%m-%d")
    timestamp = now.strftime("%H:%M")

    thread = Thread(
        id=tid,
        name=title,
        desc=desc,
        status=status,
        body=body or "",
        log={today: [LogEntry(time=timestamp, text="Created thread.")]},
        file_path=filepath,
    )

    save_thread(thread)

    rel_path = filepath.relative_to(workspace)
    print(f"Created {scope.level_desc}: {tid}")
    print(f"  → {rel_path}")

    if not body:
        print(f'Hint: Add body with: echo "content" | thread body {tid} --set', file=sys.stderr)

    if do_commit:
        from .lifecycle import auto_commit
        if message is None:
            message = f"threads: add {tid}-{slug}"
        auto_commit(filepath, message, workspace)
    else:
        print(f"Note: Thread {tid} has uncommitted changes. Use 'threads commit {tid}' when ready.")

    return tid


def cmd_remove(ref: str, do_commit: bool = False, message: str | None = None) -> None:
    """Remove a thread."""
    workspace = get_workspace()
    file_path = find_thread_by_ref(ref, workspace)

    thread = load_thread(file_path)
    rel_path = file_path.relative_to(workspace)

    # Check if tracked in git
    from ..git import is_tracked
    was_tracked = is_tracked(file_path, workspace)

    # Delete file
    file_path.unlink()
    print(f"Removed: {file_path}")

    if not was_tracked:
        print("Note: Thread was never committed to git, no commit needed.")
    elif do_commit:
        from .lifecycle import auto_commit_remove
        if message is None:
            message = f"threads: remove '{thread.name}'"
        auto_commit_remove(rel_path, message, workspace)
    else:
        print("Note: To commit this removal, run:")
        print(f'  git -C "$WORKSPACE" add "{rel_path}" && git -C "$WORKSPACE" commit -m "threads: remove \'{thread.name}\'"')


def cmd_update(
    ref: str,
    title: str | None = None,
    desc: str | None = None,
    do_commit: bool = False,
    message: str | None = None,
) -> None:
    """Update thread title and/or description."""
    if title is None and desc is None:
        raise ValueError("Specify --title and/or --desc")

    workspace = get_workspace()
    file_path = find_thread_by_ref(ref, workspace)
    thread = load_thread(file_path)

    if title is not None:
        thread.name = title
        print(f"Title updated: {title}")

    if desc is not None:
        thread.desc = desc
        print(f"Description updated: {desc}")

    save_thread(thread)
    print(f"Updated: {file_path}")

    if do_commit:
        from .lifecycle import auto_commit
        if message is None:
            message = f"threads: update {file_path.stem}"
        auto_commit(file_path, message, workspace)
    else:
        print(f"Note: Thread {ref} has uncommitted changes. Use 'threads commit {ref}' when ready.")

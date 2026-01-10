"""Section commands: body, note, todo, log."""

import hashlib
import sys
import time
from datetime import datetime

from ..models import LogEntry, Note, Thread, Todo
from ..storage import load_thread, save_thread
from ..workspace import find_thread_by_ref, get_workspace


def generate_hash(text: str) -> str:
    """Generate 4-char hash for item identification."""
    data = f"{text}{time.time_ns()}"
    return hashlib.sha256(data.encode()).hexdigest()[:4]


def add_log_entry(thread: Thread, entry_text: str) -> None:
    """Add a timestamped log entry to thread."""
    now = datetime.now()
    today = now.strftime("%Y-%m-%d")
    timestamp = now.strftime("%H:%M")

    if today not in thread.log:
        thread.log[today] = []

    # Insert at beginning of today's entries
    thread.log[today].insert(0, LogEntry(time=timestamp, text=entry_text))


def cmd_body(
    ref: str,
    mode: str = "set",
    do_commit: bool = False,
    message: str | None = None,
) -> None:
    """Set or append to body section.

    Content is read from stdin.
    """
    workspace = get_workspace()
    file_path = find_thread_by_ref(ref, workspace)
    thread = load_thread(file_path)

    # Read content from stdin
    if sys.stdin.isatty():
        raise ValueError("No content provided (use stdin)")

    import select
    # Check if stdin has data available (non-blocking)
    if not select.select([sys.stdin], [], [], 0.0)[0]:
        raise ValueError("No content provided (stdin empty)")

    content = sys.stdin.read()
    if not content:
        raise ValueError("No content provided (use stdin)")

    if mode == "set":
        thread.body = content.strip()
    else:  # append
        if thread.body:
            thread.body = thread.body + "\n\n" + content.strip()
        else:
            thread.body = content.strip()

    save_thread(thread)
    print(f"Body {mode}: {file_path}")

    if do_commit:
        from .lifecycle import auto_commit
        if message is None:
            message = f"threads: update {file_path.stem}"
        auto_commit(file_path, message, workspace)
    else:
        print(f"Note: Thread {ref} has uncommitted changes. Use 'threads commit {ref}' when ready.")


def cmd_note(
    ref: str,
    action: str,
    text_or_hash: str,
    new_text: str | None = None,
    do_commit: bool = False,
    message: str | None = None,
) -> None:
    """Add, edit, or remove a note."""
    workspace = get_workspace()
    file_path = find_thread_by_ref(ref, workspace)
    thread = load_thread(file_path)

    log_entry: str | None = None

    if action == "add":
        note_hash = generate_hash(text_or_hash)
        thread.notes.insert(0, Note(text=text_or_hash, hash=note_hash))
        log_entry = f"Added note: {text_or_hash}"
        print(f"Added note: {text_or_hash} (id: {note_hash})")

    elif action == "edit":
        if new_text is None:
            raise ValueError("Edit requires new text")

        # Find note by hash
        found = False
        for note in thread.notes:
            if note.hash.startswith(text_or_hash):
                note.text = new_text
                found = True
                log_entry = f"Edited note {text_or_hash}"
                print(f"Edited note {text_or_hash}")
                break

        if not found:
            raise ValueError(f"No note with hash '{text_or_hash}' found")

    elif action == "remove":
        # Find and remove note by hash
        original_len = len(thread.notes)
        thread.notes = [n for n in thread.notes if not n.hash.startswith(text_or_hash)]

        if len(thread.notes) == original_len:
            raise ValueError(f"No note with hash '{text_or_hash}' found")

        log_entry = f"Removed note {text_or_hash}"
        print(f"Removed note {text_or_hash}")

    else:
        raise ValueError(f"Unknown action '{action}'. Use: add, edit, remove")

    # Auto-log the note operation
    if log_entry:
        add_log_entry(thread, log_entry)

    save_thread(thread)

    if do_commit:
        from .lifecycle import auto_commit
        if message is None:
            message = f"threads: update {file_path.stem}"
        auto_commit(file_path, message, workspace)
    else:
        print(f"Note: Thread {ref} has uncommitted changes. Use 'threads commit {ref}' when ready.")


def cmd_todo(
    ref: str,
    action: str,
    item_or_hash: str,
    do_commit: bool = False,
    message: str | None = None,
) -> None:
    """Add, check, uncheck, or remove a todo item."""
    workspace = get_workspace()
    file_path = find_thread_by_ref(ref, workspace)
    thread = load_thread(file_path)

    if action == "add":
        item_hash = generate_hash(item_or_hash)
        thread.todos.insert(0, Todo(text=item_or_hash, hash=item_hash, checked=False))
        print(f"Added to Todo: {item_or_hash} (id: {item_hash})")

    elif action in ("check", "complete", "done"):
        # Find unchecked item by hash
        found = False
        for todo in thread.todos:
            if todo.hash.startswith(item_or_hash) and not todo.checked:
                todo.checked = True
                found = True
                print(f"Checked item {item_or_hash}")
                break

        if not found:
            raise ValueError(f"No unchecked item with hash '{item_or_hash}' found")

    elif action == "uncheck":
        # Find checked item by hash
        found = False
        for todo in thread.todos:
            if todo.hash.startswith(item_or_hash) and todo.checked:
                todo.checked = False
                found = True
                print(f"Unchecked item {item_or_hash}")
                break

        if not found:
            raise ValueError(f"No checked item with hash '{item_or_hash}' found")

    elif action == "remove":
        # Find and remove item by hash
        original_len = len(thread.todos)
        thread.todos = [t for t in thread.todos if not t.hash.startswith(item_or_hash)]

        if len(thread.todos) == original_len:
            raise ValueError(f"No item with hash '{item_or_hash}' found")

        print(f"Removed item {item_or_hash}")

    else:
        raise ValueError(f"Unknown action '{action}'. Use: add, check, uncheck, remove")

    save_thread(thread)

    if do_commit:
        from .lifecycle import auto_commit
        if message is None:
            message = f"threads: update {file_path.stem}"
        auto_commit(file_path, message, workspace)
    else:
        print(f"Note: Thread {ref} has uncommitted changes. Use 'threads commit {ref}' when ready.")


def cmd_log(
    ref: str,
    entry: str | None = None,
    do_commit: bool = False,
    message: str | None = None,
) -> None:
    """Add a log entry to thread."""
    workspace = get_workspace()
    file_path = find_thread_by_ref(ref, workspace)
    thread = load_thread(file_path)

    # Read entry from stdin if not provided
    if entry is None and not sys.stdin.isatty():
        import select
        # Only read if stdin has data available (non-blocking check)
        if select.select([sys.stdin], [], [], 0.0)[0]:
            entry = sys.stdin.read().strip()

    if not entry:
        raise ValueError("No log entry provided")

    add_log_entry(thread, entry)
    save_thread(thread)

    print(f"Logged to: {file_path}")

    if do_commit:
        from .lifecycle import auto_commit
        if message is None:
            message = f"threads: update {file_path.stem}"
        auto_commit(file_path, message, workspace)
    else:
        print(f"Note: Thread {ref} has uncommitted changes. Use 'threads commit {ref}' when ready.")

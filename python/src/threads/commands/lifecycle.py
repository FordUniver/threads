"""Lifecycle commands: status, resolve, reopen, move, commit."""

import sys
from datetime import datetime
from pathlib import Path

from ..git import git_add, git_commit, git_pull_rebase, git_push, is_modified, is_tracked, get_file_status
from ..models import LogEntry, Thread
from ..storage import find_threads, load_thread, save_thread
from ..workspace import find_thread_by_ref, get_workspace, infer_scope, parse_thread_path


def auto_commit(file: Path, message: str, workspace: Path) -> None:
    """Stage, commit, and push a file."""
    try:
        git_add(file, workspace)
        git_commit(message, workspace)
        if not git_pull_rebase(workspace) or not git_push(workspace):
            print("WARNING: git push failed (commit succeeded)", file=sys.stderr)
    except Exception as e:
        print(f"ERROR: git operation failed: {e}", file=sys.stderr)
        raise


def auto_commit_remove(rel_path: Path, message: str, workspace: Path) -> None:
    """Stage removal, commit, and push."""
    try:
        git_add(rel_path, workspace)
        git_commit(message, workspace)
        if not git_pull_rebase(workspace) or not git_push(workspace):
            print("WARNING: git push failed (commit succeeded)", file=sys.stderr)
    except Exception as e:
        print(f"ERROR: git operation failed: {e}", file=sys.stderr)
        raise


def generate_commit_message(files: list[Path], workspace: Path) -> str:
    """Generate commit message for thread changes."""
    added = []
    modified = []
    deleted = []

    for file in files:
        name = file.stem
        status = get_file_status(file, workspace)
        if status == "added":
            added.append(name)
        elif status == "deleted":
            deleted.append(name)
        else:
            modified.append(name)

    total = len(added) + len(modified) + len(deleted)

    if total == 1:
        if added:
            return f"threads: add {added[0]}"
        elif modified:
            return f"threads: update {modified[0]}"
        else:
            return f"threads: remove {deleted[0]}"
    elif total <= 3:
        all_ids = [name.split("-")[0] for name in added + modified + deleted]
        action = "update"
        if len(added) == total:
            action = "add"
        elif len(deleted) == total:
            action = "remove"
        return f"threads: {action} {' '.join(all_ids)}"
    else:
        action = "update"
        if len(added) == total:
            action = "add"
        elif len(deleted) == total:
            action = "remove"
        return f"threads: {action} {total} threads"


def add_log_entry(thread: Thread, entry_text: str) -> None:
    """Add a timestamped log entry."""
    now = datetime.now()
    today = now.strftime("%Y-%m-%d")
    timestamp = now.strftime("%H:%M")

    if today not in thread.log:
        thread.log[today] = []

    thread.log[today].insert(0, LogEntry(time=timestamp, text=entry_text))


def cmd_status(
    ref: str,
    new_status: str,
    do_commit: bool = False,
    message: str | None = None,
) -> None:
    """Change thread status."""
    workspace = get_workspace()
    file_path = find_thread_by_ref(ref, workspace)
    thread = load_thread(file_path)

    old_status = thread.status
    thread.status = new_status

    save_thread(thread)
    print(f"Status changed: {old_status} → {new_status} ({file_path})")

    if do_commit:
        if message is None:
            message = f"threads: update {file_path.stem}"
        auto_commit(file_path, message, workspace)
    else:
        print(f"Note: Thread {ref} has uncommitted changes. Use 'thread commit {ref}' when ready.")


def cmd_resolve(ref: str, do_commit: bool = False, message: str | None = None) -> None:
    """Mark thread as resolved."""
    workspace = get_workspace()
    file_path = find_thread_by_ref(ref, workspace)
    thread = load_thread(file_path)

    old_status = thread.status
    thread.status = "resolved"

    add_log_entry(thread, "Resolved.")

    save_thread(thread)
    print(f"Resolved: {old_status} → resolved ({file_path})")

    if do_commit:
        if message is None:
            message = f"threads: update {file_path.stem}"
        auto_commit(file_path, message, workspace)
    else:
        print(f"Note: Thread {ref} has uncommitted changes. Use 'thread commit {ref}' when ready.")


def cmd_reopen(
    ref: str,
    new_status: str = "active",
    do_commit: bool = False,
    message: str | None = None,
) -> None:
    """Reopen a resolved thread."""
    workspace = get_workspace()
    file_path = find_thread_by_ref(ref, workspace)
    thread = load_thread(file_path)

    old_status = thread.status
    thread.status = new_status

    add_log_entry(thread, "Reopened.")

    save_thread(thread)
    print(f"Reopened: {old_status} → {new_status} ({file_path})")

    if do_commit:
        if message is None:
            message = f"threads: update {file_path.stem}"
        auto_commit(file_path, message, workspace)
    else:
        print(f"Note: Thread {ref} has uncommitted changes. Use 'thread commit {ref}' when ready.")


def cmd_move(
    ref: str,
    new_path: str,
    do_commit: bool = False,
    message: str | None = None,
) -> None:
    """Move thread to a new location."""
    workspace = get_workspace()
    src_file = find_thread_by_ref(ref, workspace)

    # Resolve destination
    scope = infer_scope(new_path, workspace)

    # Ensure dest .threads/ exists
    scope.threads_dir.mkdir(parents=True, exist_ok=True)

    # Move file
    dest_file = scope.threads_dir / src_file.name

    if dest_file.exists():
        raise ValueError(f"Thread already exists at destination: {dest_file}")

    src_file.rename(dest_file)
    rel_dest = dest_file.relative_to(workspace)

    print(f"Moved to {scope.level_desc}")
    print(f"  → {rel_dest}")

    if do_commit:
        rel_src = src_file.relative_to(workspace)
        git_add(rel_src, workspace)
        git_add(dest_file, workspace)
        if message is None:
            message = f"threads: move {src_file.stem} to {scope.level_desc}"
        git_commit(message, workspace)
        git_pull_rebase(workspace) and git_push(workspace)
    else:
        print("Note: Use --commit to commit this move")


def cmd_commit(
    refs: list[str] | None = None,
    pending: bool = False,
    message: str | None = None,
    auto_msg: bool = False,
) -> None:
    """Commit thread changes."""
    workspace = get_workspace()
    files: list[Path] = []

    if pending:
        # Collect all modified thread files
        for path in find_threads(workspace):
            if is_modified(path, workspace):
                files.append(path)
    else:
        if not refs:
            raise ValueError("Provide thread IDs or use --pending")

        for ref in refs:
            file_path = find_thread_by_ref(ref, workspace)
            if not is_modified(file_path, workspace):
                print(f"No changes in thread: {ref}")
                continue
            files.append(file_path)

    if not files:
        print("No threads to commit.")
        return

    # Generate commit message if not provided
    if message is None:
        message = generate_commit_message(files, workspace)
        print(f"Generated message: {message}")
        if not auto_msg and sys.stdin.isatty():
            confirm = input("Proceed? [Y/n] ")
            if confirm.lower().startswith("n"):
                print("Aborted.")
                return

    # Stage and commit
    for file in files:
        git_add(file, workspace)

    git_commit(message, workspace)
    if not git_pull_rebase(workspace) or not git_push(workspace):
        print("WARNING: git push failed (commit succeeded)", file=sys.stderr)

    print(f"Committed {len(files)} thread(s).")


def cmd_git() -> None:
    """Show pending thread changes."""
    workspace = get_workspace()
    modified = []

    for path in find_threads(workspace):
        if is_modified(path, workspace):
            modified.append(path.relative_to(workspace))

    if not modified:
        print("No pending thread changes.")
        return

    print("Pending thread changes:")
    for f in modified:
        print(f"  {f}")
    print()
    print("Suggested:")
    files_str = " ".join(str(f) for f in modified)
    print(f'  git add {files_str} && git commit -m "threads: update" && git push')


def cmd_validate(path: str | None = None, recursive: bool = False) -> bool:
    """Validate thread files. Returns True if all valid."""
    from ..models import ALL_STATUSES

    workspace = get_workspace()
    errors = 0

    if path:
        p = Path(path) if Path(path).is_absolute() else workspace / path
        if p.is_file():
            files = [p]
        elif p.is_dir():
            # Validate threads in this directory
            if recursive:
                files = list(p.glob("**/.threads/*.md"))
            else:
                files = list(p.glob(".threads/*.md"))
        else:
            raise ValueError(f"File not found: {path}")
    else:
        if recursive:
            files = find_threads(workspace)
        else:
            files = list(workspace.glob(".threads/*.md"))

    for file in files:
        rel_path = file.relative_to(workspace)
        thread = load_thread(file)

        issues = []
        if not thread.name:
            issues.append("missing name/title field")
        if not thread.status:
            issues.append("missing status field")
        elif thread.base_status() not in ALL_STATUSES:
            issues.append(f"invalid status '{thread.base_status()}'")

        if issues:
            print(f"WARN: {rel_path}: {', '.join(issues)}")
            errors += 1
        else:
            print(f"OK: {rel_path}")

    return errors == 0

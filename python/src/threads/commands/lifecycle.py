"""Lifecycle commands: status, resolve, reopen, move, commit."""

import sys
from datetime import datetime
from pathlib import Path

from ..git import git_add, git_commit, is_modified, get_file_status
from ..models import LogEntry, Thread, validate_status
from ..storage import find_threads, load_thread, save_thread
from ..workspace import (
    find_thread_by_ref,
    get_workspace,
    infer_scope,
    path_relative_to_git_root,
)


def auto_commit(file: Path, message: str, git_root: Path) -> None:
    """Stage and commit a file (push is opt-in)."""
    try:
        git_add(file, git_root)
        git_commit(message, git_root)
    except Exception as e:
        print(f"ERROR: git operation failed: {e}", file=sys.stderr)
        raise


def auto_commit_remove(rel_path: Path, message: str, git_root: Path) -> None:
    """Stage removal and commit (push is opt-in)."""
    try:
        git_add(rel_path, git_root)
        git_commit(message, git_root)
    except Exception as e:
        print(f"ERROR: git operation failed: {e}", file=sys.stderr)
        raise


def generate_commit_message(files: list[Path], git_root: Path) -> str:
    """Generate commit message for thread changes."""
    added = []
    modified = []
    deleted = []

    for file in files:
        name = file.stem
        status = get_file_status(file, git_root)
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
    # Validate status before proceeding
    validate_status(new_status)

    git_root = get_workspace()
    file_path = find_thread_by_ref(ref, git_root)
    thread = load_thread(file_path)

    old_status = thread.status
    thread.status = new_status

    save_thread(thread)
    print(f"Status changed: {old_status} → {new_status} ({file_path})")

    if do_commit:
        if message is None:
            message = f"threads: update {file_path.stem}"
        auto_commit(file_path, message, git_root)
    else:
        print(f"Note: Thread {ref} has uncommitted changes. Use 'threads commit {ref}' when ready.", file=sys.stderr)


def cmd_resolve(ref: str, do_commit: bool = False, message: str | None = None) -> None:
    """Mark thread as resolved."""
    git_root = get_workspace()
    file_path = find_thread_by_ref(ref, git_root)
    thread = load_thread(file_path)

    old_status = thread.status
    thread.status = "resolved"

    add_log_entry(thread, "Resolved.")

    save_thread(thread)
    print(f"Resolved: {old_status} → resolved ({file_path})")

    if do_commit:
        if message is None:
            message = f"threads: update {file_path.stem}"
        auto_commit(file_path, message, git_root)
    else:
        print(f"Note: Thread {ref} has uncommitted changes. Use 'threads commit {ref}' when ready.", file=sys.stderr)


def cmd_reopen(
    ref: str,
    new_status: str = "active",
    do_commit: bool = False,
    message: str | None = None,
) -> None:
    """Reopen a resolved thread."""
    # Validate status before proceeding
    validate_status(new_status)

    git_root = get_workspace()
    file_path = find_thread_by_ref(ref, git_root)
    thread = load_thread(file_path)

    old_status = thread.status
    thread.status = new_status

    add_log_entry(thread, "Reopened.")

    save_thread(thread)
    print(f"Reopened: {old_status} → {new_status} ({file_path})")

    if do_commit:
        if message is None:
            message = f"threads: update {file_path.stem}"
        auto_commit(file_path, message, git_root)
    else:
        print(f"Note: Thread {ref} has uncommitted changes. Use 'threads commit {ref}' when ready.", file=sys.stderr)


def cmd_move(
    ref: str,
    new_path: str,
    do_commit: bool = False,
    message: str | None = None,
) -> None:
    """Move thread to a new location."""
    git_root = get_workspace()
    src_file = find_thread_by_ref(ref, git_root)

    # Resolve destination using new path resolution
    scope = infer_scope(new_path, git_root)

    # Ensure dest .threads/ exists
    scope.threads_dir.mkdir(parents=True, exist_ok=True)

    # Move file
    dest_file = scope.threads_dir / src_file.name

    if dest_file.exists():
        raise ValueError(f"Thread already exists at destination: {dest_file}")

    src_file.rename(dest_file)
    rel_dest = path_relative_to_git_root(git_root, dest_file)

    print(f"Moved to {scope.level_desc}")
    print(f"  → {rel_dest}")

    if do_commit:
        rel_src = path_relative_to_git_root(git_root, src_file)
        git_add(Path(rel_src), git_root)
        git_add(dest_file, git_root)
        if message is None:
            message = f"threads: move {src_file.stem} to {scope.level_desc}"
        git_commit(message, git_root)
        print("Note: Changes are local. Push with 'git push' when ready.")
    else:
        print("Note: Use --commit to commit this move", file=sys.stderr)


def cmd_commit(
    refs: list[str] | None = None,
    pending: bool = False,
    message: str | None = None,
    auto_msg: bool = False,
) -> None:
    """Commit thread changes."""
    git_root = get_workspace()
    files: list[Path] = []

    if pending:
        # Collect all modified thread files
        for path in find_threads(git_root):
            if is_modified(path, git_root):
                files.append(path)
    else:
        if not refs:
            raise ValueError("Provide thread IDs or use --pending")

        for ref in refs:
            file_path = find_thread_by_ref(ref, git_root)
            if not is_modified(file_path, git_root):
                print(f"No changes in thread: {ref}")
                continue
            files.append(file_path)

    if not files:
        print("No threads to commit.")
        return

    # Generate commit message if not provided
    if message is None:
        message = generate_commit_message(files, git_root)
        print(f"Generated message: {message}")
        if not auto_msg and sys.stdin.isatty():
            confirm = input("Proceed? [Y/n] ")
            if confirm.lower().startswith("n"):
                print("Aborted.")
                return

    # Stage and commit
    for file in files:
        git_add(file, git_root)

    git_commit(message, git_root)
    print(f"Committed {len(files)} thread(s).")


def cmd_git() -> None:
    """Show pending thread changes."""
    git_root = get_workspace()
    modified = []

    for path in find_threads(git_root):
        if is_modified(path, git_root):
            rel = path_relative_to_git_root(git_root, path)
            modified.append(rel)

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


def cmd_validate(
    path: str | None = None,
    recursive: bool = False,
    format_str: str = "fancy",
    json_output: bool = False,
) -> bool:
    """Validate thread files. Returns True if all valid."""
    import json
    import yaml

    from ..models import ALL_STATUSES
    from ..output import OutputFormat, parse_format, resolve_format

    # Determine output format
    if json_output:
        fmt = OutputFormat.JSON
    else:
        fmt = resolve_format(parse_format(format_str))

    git_root = get_workspace()

    if path:
        p = Path(path) if Path(path).is_absolute() else git_root / path
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
            files = find_threads(git_root)
        else:
            files = list(git_root.glob(".threads/*.md"))

    results = []
    error_count = 0

    for file in files:
        rel_path = path_relative_to_git_root(git_root, file)
        thread = load_thread(file)

        issues = []
        if not thread.name:
            issues.append("missing name/title field")
        if not thread.status:
            issues.append("missing status field")
        elif thread.base_status() not in ALL_STATUSES:
            issues.append(f"invalid status '{thread.base_status()}'")

        valid = len(issues) == 0
        if not valid:
            error_count += 1

        results.append({
            "path": rel_path,
            "valid": valid,
            "issues": issues,
        })

    # Output based on format
    if fmt in (OutputFormat.FANCY, OutputFormat.PLAIN):
        for r in results:
            if r["valid"]:
                print(f"OK: {r['path']}")
            else:
                print(f"WARN: {r['path']}: {', '.join(r['issues'])}")
    elif fmt == OutputFormat.JSON:
        data = {
            "total": len(results),
            "errors": error_count,
            "results": results,
        }
        print(json.dumps(data, indent=2))
    elif fmt == OutputFormat.YAML:
        data = {
            "total": len(results),
            "errors": error_count,
            "results": results,
        }
        print(yaml.dump(data, default_flow_style=False, sort_keys=False), end="")

    return error_count == 0

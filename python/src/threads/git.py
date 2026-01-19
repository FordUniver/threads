"""Git operations for threads."""

import subprocess
from pathlib import Path
from typing import Optional


# Cache for batched git status
_status_cache: dict[Path, dict[Path, str]] = {}


def run_git(
    args: list[str], workspace: Path, check: bool = True
) -> subprocess.CompletedProcess[str]:
    """Run a git command in the workspace."""
    return subprocess.run(
        ["git", "-C", str(workspace), *args],
        capture_output=True,
        text=True,
        check=check,
    )


def git_add(file: Path, workspace: Path) -> None:
    """Stage a file for commit. Skips non-existent files (assumed to be already-staged deletions)."""
    if file.is_absolute():
        full_path = file
        rel_path = file.relative_to(workspace)
    else:
        full_path = workspace / file
        rel_path = file

    # Skip non-existent files (deletions already staged)
    if not full_path.exists():
        return

    run_git(["add", str(rel_path)], workspace)


def git_commit(message: str, workspace: Path, files: list[Path] | None = None) -> None:
    """Create a commit with the given message, optionally limited to specific files."""
    args = ["commit", "-m", message]
    if files:
        args.extend(str(f.relative_to(workspace) if f.is_absolute() else f) for f in files)
    run_git(args, workspace)


def git_pull_rebase(workspace: Path) -> bool:
    """Pull with rebase. Returns True on success."""
    result = run_git(["pull", "--rebase"], workspace, check=False)
    return result.returncode == 0


def git_push(workspace: Path) -> bool:
    """Push to remote. Returns True on success."""
    result = run_git(["push"], workspace, check=False)
    return result.returncode == 0


def get_all_git_status(workspace: Path, refresh: bool = False) -> dict[Path, str]:
    """Get git status for all files in workspace (batched operation).

    Returns a dict mapping relative paths to status codes:
    - 'M' = modified (staged or unstaged)
    - 'A' = added (staged)
    - 'D' = deleted
    - '?' = untracked
    - 'R' = renamed
    - 'C' = copied

    Uses caching to avoid repeated subprocess calls.
    """
    global _status_cache

    workspace_resolved = workspace.resolve()
    if not refresh and workspace_resolved in _status_cache:
        return _status_cache[workspace_resolved]

    result = run_git(["status", "--porcelain", "--untracked-files=all"], workspace, check=False)
    if result.returncode != 0:
        return {}

    status_map: dict[Path, str] = {}
    for line in result.stdout.splitlines():
        if len(line) < 4:
            continue
        # Format: XY PATH or XY PATH -> NEWPATH (for renames)
        index_status = line[0]
        worktree_status = line[1]
        path_part = line[3:]

        # Handle renames/copies: "R  old -> new"
        if " -> " in path_part:
            path_part = path_part.split(" -> ")[1]

        rel_path = Path(path_part)

        # Determine effective status (prioritize index status, then worktree)
        if index_status == "?" or worktree_status == "?":
            status_map[rel_path] = "?"
        elif index_status == "A":
            status_map[rel_path] = "A"
        elif index_status == "D" or worktree_status == "D":
            status_map[rel_path] = "D"
        elif index_status == "R" or worktree_status == "R":
            status_map[rel_path] = "R"
        elif index_status == "M" or worktree_status == "M":
            status_map[rel_path] = "M"
        elif index_status != " " or worktree_status != " ":
            status_map[rel_path] = index_status if index_status != " " else worktree_status

    _status_cache[workspace_resolved] = status_map
    return status_map


def clear_status_cache(workspace: Optional[Path] = None) -> None:
    """Clear the git status cache for a workspace or all workspaces."""
    global _status_cache
    if workspace is None:
        _status_cache.clear()
    else:
        _status_cache.pop(workspace.resolve(), None)


def is_tracked(file: Path, workspace: Path) -> bool:
    """Check if file is tracked by git."""
    rel_path = file.relative_to(workspace) if file.is_absolute() else file

    # Untracked files show as '?' in status; if not in status at all, it's tracked and clean
    status_map = get_all_git_status(workspace)
    status = status_map.get(rel_path)

    if status == "?":
        return False

    # If file has any status or is not in the map, check ls-files for certainty
    if status is None:
        # File not in status output - either tracked+clean or truly untracked
        result = run_git(["ls-files", "--error-unmatch", str(rel_path)], workspace, check=False)
        return result.returncode == 0

    # File has a status (M, A, D, R) - it's tracked
    return True


def is_modified(file: Path, workspace: Path) -> bool:
    """Check if file has uncommitted changes (staged, unstaged, or untracked).

    Uses batched git status for efficiency.
    """
    rel_path = file.relative_to(workspace) if file.is_absolute() else file
    status_map = get_all_git_status(workspace)

    # Any status code means the file has changes
    return rel_path in status_map


def get_file_status(file: Path, workspace: Path) -> str:
    """Get git status of file: 'added', 'modified', 'deleted', or 'untracked'.

    Uses batched git status for efficiency.
    """
    rel_path = file.relative_to(workspace) if file.is_absolute() else file
    status_map = get_all_git_status(workspace)
    status = status_map.get(rel_path)

    if status == "?":
        return "untracked"
    elif status == "A":
        return "added"
    elif status == "D":
        return "deleted"
    elif status in ("M", "R", "C"):
        return "modified"
    else:
        # Not in status map - check if it exists in HEAD
        result = run_git(["cat-file", "-e", f"HEAD:{rel_path}"], workspace, check=False)
        in_head = result.returncode == 0

        if in_head:
            if file.exists():
                return "modified"
            else:
                return "deleted"
        else:
            return "added"


def find_deleted_thread_files(workspace: Path) -> list[Path]:
    """Find deleted thread files that are staged or in working tree.

    Returns paths of files matching .threads/*.md that show as deleted (D) in git status.
    """
    status_map = get_all_git_status(workspace, refresh=True)
    deleted = []

    for rel_path, status in status_map.items():
        if status == "D" and ".threads/" in str(rel_path) and str(rel_path).endswith(".md"):
            deleted.append(rel_path)

    return deleted

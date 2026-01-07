"""Git operations for threads."""

import subprocess
from pathlib import Path


def run_git(args: list[str], workspace: Path, check: bool = True) -> subprocess.CompletedProcess:
    """Run a git command in the workspace."""
    return subprocess.run(
        ["git", "-C", str(workspace), *args],
        capture_output=True,
        text=True,
        check=check,
    )


def git_add(file: Path, workspace: Path) -> None:
    """Stage a file for commit."""
    rel_path = file.relative_to(workspace) if file.is_absolute() else file
    run_git(["add", str(rel_path)], workspace)


def git_commit(message: str, workspace: Path) -> None:
    """Create a commit with the given message."""
    run_git(["commit", "-m", message], workspace)


def git_pull_rebase(workspace: Path) -> bool:
    """Pull with rebase. Returns True on success."""
    result = run_git(["pull", "--rebase"], workspace, check=False)
    return result.returncode == 0


def git_push(workspace: Path) -> bool:
    """Push to remote. Returns True on success."""
    result = run_git(["push"], workspace, check=False)
    return result.returncode == 0


def is_tracked(file: Path, workspace: Path) -> bool:
    """Check if file is tracked by git."""
    rel_path = file.relative_to(workspace) if file.is_absolute() else file
    result = run_git(["ls-files", "--error-unmatch", str(rel_path)], workspace, check=False)
    return result.returncode == 0


def is_modified(file: Path, workspace: Path) -> bool:
    """Check if file has uncommitted changes (staged, unstaged, or untracked)."""
    rel_path = file.relative_to(workspace) if file.is_absolute() else file

    # Check for staged changes
    result = run_git(["diff", "--cached", "--quiet", "--", str(rel_path)], workspace, check=False)
    if result.returncode != 0:
        return True

    # Check for unstaged changes
    result = run_git(["diff", "--quiet", "--", str(rel_path)], workspace, check=False)
    if result.returncode != 0:
        return True

    # Check if untracked
    if not is_tracked(file, workspace):
        return True

    return False


def get_file_status(file: Path, workspace: Path) -> str:
    """Get git status of file: 'added', 'modified', 'deleted', or 'untracked'."""
    rel_path = file.relative_to(workspace) if file.is_absolute() else file

    # Check if file exists in HEAD
    result = run_git(["cat-file", "-e", f"HEAD:{rel_path}"], workspace, check=False)
    in_head = result.returncode == 0

    if in_head:
        if file.exists():
            return "modified"
        else:
            return "deleted"
    else:
        return "added"

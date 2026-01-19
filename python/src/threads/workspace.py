"""Workspace utilities: git root detection, path resolution, thread finding."""

import re
import subprocess
from dataclasses import dataclass
from pathlib import Path


@dataclass
class Scope:
    """Represents thread placement information. Path is relative to git root."""

    threads_dir: Path  # path to .threads directory (absolute)
    path: str  # path relative to git root (e.g., "src/models", "." for root)
    level_desc: str  # human-readable description


def get_workspace() -> Path:
    """Find the git repository root from current directory.

    Returns the git root path.
    Raises ValueError if not in a git repository.
    """
    return find_git_root()


def find_git_root() -> Path:
    """Find the git repository root using 'git rev-parse --show-toplevel'."""
    try:
        result = subprocess.run(
            ["git", "rev-parse", "--show-toplevel"],
            capture_output=True,
            text=True,
            check=True,
        )
        root = result.stdout.strip()
        if not root:
            raise ValueError("Git root is empty")
        return Path(root)
    except subprocess.CalledProcessError:
        raise ValueError(
            "Not in a git repository. threads requires a git repo to define scope."
        )


def find_git_root_for_path(path: Path) -> Path:
    """Find the git root for a specific path."""
    try:
        result = subprocess.run(
            ["git", "-C", str(path), "rev-parse", "--show-toplevel"],
            capture_output=True,
            text=True,
            check=True,
        )
        return Path(result.stdout.strip())
    except subprocess.CalledProcessError:
        raise ValueError(f"Not in a git repository at: {path}")


def is_git_root(path: Path) -> bool:
    """Check if a directory contains a .git folder."""
    return (path / ".git").is_dir()


def infer_scope(path_arg: str | None, git_root: Path | None = None) -> Scope:
    """Determine the threads directory and scope from a path specification.

    Path resolution rules:
    - None or "": PWD
    - ".": PWD (explicit)
    - "./X/Y": PWD-relative
    - "/X/Y": Absolute
    - "X/Y" (no leading ./ or /): Git-root-relative

    Args:
        path_arg: Path specification (optional)
        git_root: Git root path (will be detected if not provided)

    Returns:
        Scope object with threads_dir, path, and level_desc
    """
    if git_root is None:
        git_root = find_git_root()

    pwd = Path.cwd()

    if path_arg is None or path_arg == "":
        # No path argument: use PWD
        target_path = pwd
    elif path_arg == ".":
        # Explicit ".": use PWD
        target_path = pwd
    elif path_arg.startswith("./"):
        # PWD-relative path: ./X/Y
        rel = path_arg[2:]
        target_path = pwd / rel
    elif path_arg.startswith("/"):
        # Absolute path
        target_path = Path(path_arg)
    else:
        # Git-root-relative path: X/Y
        target_path = git_root / path_arg

    # Resolve to absolute for consistent comparison
    target_path = target_path.resolve()
    git_root_resolved = git_root.resolve()

    # Check if directory exists
    if not target_path.is_dir():
        raise ValueError(f"Path not found or not a directory: {target_path}")

    # Verify target is within the git repo
    try:
        target_path.relative_to(git_root_resolved)
    except ValueError:
        raise ValueError(
            f"Path must be within git repository: {target_path} (git root: {git_root})"
        )

    # Check if target is inside a nested git repo
    if target_path != git_root_resolved:
        check_path = target_path
        while check_path != git_root_resolved:
            if is_git_root(check_path):
                raise ValueError(
                    f"Path is inside a nested git repository at: {check_path}"
                )
            check_path = check_path.parent

    # Compute path relative to git root
    try:
        rel_path = str(target_path.relative_to(git_root_resolved))
    except ValueError:
        rel_path = "."

    if rel_path == "" or rel_path == ".":
        rel_path = "."

    # Build description
    level_desc = "repo root" if rel_path == "." else rel_path

    # Build threads directory path
    threads_dir = target_path / ".threads"

    return Scope(
        threads_dir=threads_dir,
        path=rel_path,
        level_desc=level_desc,
    )


def parse_thread_path(file_path: Path, git_root: Path) -> str:
    """Extract the git-relative path component from a thread file path.

    Returns the path relative to git root (e.g., "src/models").
    """
    git_root_resolved = git_root.resolve()
    path_resolved = file_path.resolve()

    # Get path relative to git root
    try:
        rel = str(path_resolved.relative_to(git_root_resolved))
    except ValueError:
        return "."

    # Extract the directory containing .threads
    # Pattern: <path>/.threads/file.md -> return <path>
    parent = str(Path(rel).parent)
    if parent.endswith("/.threads"):
        grandparent = str(Path(parent).parent)
        return "." if grandparent in ("", ".") else grandparent
    if parent == ".threads":
        return "."

    return "."


def path_relative_to_git_root(git_root: Path, path: Path) -> str:
    """Return the path relative to git root for display purposes."""
    git_root_resolved = git_root.resolve()
    path_resolved = path.resolve()

    try:
        rel = str(path_resolved.relative_to(git_root_resolved))
        return "." if rel == "" else rel
    except ValueError:
        return str(path)


def pwd_relative_to_git_root(git_root: Path) -> str:
    """Return the current working directory relative to git root."""
    return path_relative_to_git_root(git_root, Path.cwd())


def find_thread_by_ref(ref: str, git_root: Path | None = None) -> Path:
    """Find a thread by ID or name (with fuzzy matching).

    Args:
        ref: Thread reference (ID or name)
        git_root: Git root path (detected if not provided)

    Returns:
        Path to the thread file

    Raises:
        ValueError: If thread not found or ambiguous
    """
    from .storage import find_threads, load_thread

    if git_root is None:
        git_root = get_workspace()

    threads = find_threads(git_root)

    # Fast path: exact ID match (6-char hex)
    if re.match(r"^[0-9a-f]{6}$", ref):
        for path in threads:
            if path.stem.startswith(ref + "-"):
                return path

    # Slow path: name matching
    ref_lower = ref.lower()
    substring_matches: list[Path] = []

    for path in threads:
        name = extract_name_from_path(path)

        # Exact name match
        if name == ref:
            return path

        # Substring match (case-insensitive)
        if ref_lower in name.lower():
            substring_matches.append(path)

    if len(substring_matches) == 1:
        return substring_matches[0]

    if len(substring_matches) > 1:
        ids = [
            f"{path.stem[:6]} ({extract_name_from_path(path)})"
            for path in substring_matches
        ]
        raise ValueError(
            f"Ambiguous reference '{ref}' matches {len(substring_matches)} threads: "
            + ", ".join(ids)
        )

    raise ValueError(f"Thread not found: {ref}")


def extract_name_from_path(path: Path) -> str:
    """Extract the name portion from a thread filename.

    Filename format: <6-char-id>-<name>.md
    Returns the name portion.
    """
    stem = path.stem
    if len(stem) > 7 and stem[6] == "-":
        return stem[7:]
    return stem


def extract_id_from_path(path: Path) -> str | None:
    """Extract the ID portion from a thread filename.

    Filename format: <6-char-id>-<name>.md
    Returns the ID if valid, None otherwise.
    """
    stem = path.stem
    if len(stem) >= 6 and re.match(r"^[0-9a-f]{6}$", stem[:6]):
        return stem[:6]
    return None

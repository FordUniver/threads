"""Workspace and path resolution."""

import os
import re
from dataclasses import dataclass
from pathlib import Path

from .storage import load_thread


@dataclass
class Scope:
    """Resolved scope for thread operations."""

    threads_dir: Path
    category: str | None  # None = workspace level
    project: str | None  # None = category level
    level_desc: str  # Human-readable description


def get_workspace() -> Path:
    """Get workspace path from WORKSPACE environment variable."""
    ws = os.environ.get("WORKSPACE", "")
    if not ws:
        raise RuntimeError("WORKSPACE environment variable not set")
    path = Path(ws)
    if not path.is_dir():
        raise RuntimeError(f"WORKSPACE directory does not exist: {ws}")
    return path


def infer_scope(path: str | None, workspace: Path | None = None) -> Scope:
    """Infer scope from a path specification.

    Args:
        path: Path notation - "." for workspace, "admin" for category, "admin/ssot" for project,
              or None to infer from cwd.
        workspace: Workspace root (detected if not provided).

    Returns:
        Scope with threads_dir, category, project, and level_desc.
    """
    if workspace is None:
        workspace = get_workspace()

    # Handle explicit "." for workspace
    if path == ".":
        return Scope(
            threads_dir=workspace / ".threads",
            category=None,
            project=None,
            level_desc="workspace-level thread",
        )

    # Handle None - infer from cwd
    if path is None:
        cwd = Path.cwd().resolve()
        workspace_resolved = workspace.resolve()
        try:
            rel = cwd.relative_to(workspace_resolved)
        except ValueError:
            # cwd outside workspace
            return Scope(
                threads_dir=workspace / ".threads",
                category=None,
                project=None,
                level_desc="workspace-level thread",
            )

        parts = rel.parts
        if not parts:
            return Scope(
                threads_dir=workspace / ".threads",
                category=None,
                project=None,
                level_desc="workspace-level thread",
            )
        elif len(parts) == 1:
            return Scope(
                threads_dir=workspace / parts[0] / ".threads",
                category=parts[0],
                project=None,
                level_desc=f"category-level thread ({parts[0]})",
            )
        else:
            return Scope(
                threads_dir=workspace / parts[0] / parts[1] / ".threads",
                category=parts[0],
                project=parts[1],
                level_desc=f"project-level thread ({parts[0]}/{parts[1]})",
            )

    # Handle explicit path
    # Try as absolute path first, always resolve for symlink safety
    if Path(path).is_absolute():
        abs_path = Path(path).resolve()
    elif (workspace / path).is_dir():
        abs_path = (workspace / path).resolve()
    elif Path(path).is_dir():
        abs_path = Path(path).resolve()
    else:
        raise ValueError(f"Path not found: {path}")

    workspace_resolved = workspace.resolve()
    try:
        rel = abs_path.relative_to(workspace_resolved)
    except ValueError:
        raise ValueError(f"Path must be within workspace: {path}")

    parts = rel.parts
    if not parts:
        return Scope(
            threads_dir=workspace / ".threads",
            category=None,
            project=None,
            level_desc="workspace-level thread",
        )
    elif len(parts) == 1:
        return Scope(
            threads_dir=workspace / parts[0] / ".threads",
            category=parts[0],
            project=None,
            level_desc=f"category-level thread ({parts[0]})",
        )
    else:
        return Scope(
            threads_dir=workspace / parts[0] / parts[1] / ".threads",
            category=parts[0],
            project=parts[1],
            level_desc=f"project-level thread ({parts[0]}/{parts[1]})",
        )


def parse_thread_path(file_path: Path, workspace: Path) -> tuple[str | None, str | None]:
    """Extract category and project from thread file path.

    Returns:
        (category, project) where None means workspace/category level respectively.
    """
    try:
        rel = file_path.resolve().relative_to(workspace.resolve())
    except ValueError:
        return None, None

    # Pattern: .threads/xxx.md -> workspace level
    # Pattern: cat/.threads/xxx.md -> category level
    # Pattern: cat/proj/.threads/xxx.md -> project level
    parts = rel.parts

    if len(parts) == 2 and parts[0] == ".threads":
        return None, None
    elif len(parts) == 3 and parts[1] == ".threads":
        return parts[0], None
    elif len(parts) == 4 and parts[2] == ".threads":
        return parts[0], parts[1]

    return None, None


def find_thread_by_ref(ref: str, workspace: Path | None = None) -> Path:
    """Find a thread by ID or name reference.

    Args:
        ref: 6-char hex ID or substring of thread name.
        workspace: Workspace root.

    Returns:
        Path to thread file.

    Raises:
        ValueError: If thread not found or ambiguous.
    """
    from .storage import find_threads

    if workspace is None:
        workspace = get_workspace()

    # Fast path: try glob by ID prefix
    if re.match(r"^[0-9a-f]{6}$", ref):
        matches = []
        for pattern in [
            f".threads/{ref}-*.md",
            f"*/.threads/{ref}-*.md",
            f"*/*/.threads/{ref}-*.md",
        ]:
            matches.extend(workspace.glob(pattern))

        if len(matches) == 1:
            return matches[0]
        elif len(matches) > 1:
            raise ValueError(f"Ambiguous ID prefix: {ref}")

    # Slow path: search by name
    all_threads = find_threads(workspace)
    substring_matches: list[tuple[str, str, Path]] = []

    for path in all_threads:
        thread = load_thread(path)
        thread_id = thread.id or "????"

        # Extract slug from filename (e.g., "abc123-my-thread.md" → "my-thread")
        filename = path.stem  # Remove .md
        slug = filename[7:] if len(filename) > 7 and filename[6] == "-" else filename

        # Exact match by name or slug
        if thread.name == ref or slug == ref:
            return path

        # Substring match (case-insensitive) against both name and slug
        ref_lower = ref.lower()
        if ref_lower in thread.name.lower() or ref_lower in slug.lower():
            substring_matches.append((thread_id, thread.name, path))

    if len(substring_matches) == 1:
        return substring_matches[0][2]
    elif len(substring_matches) > 1:
        matches_str = "\n".join(f"  {tid}  {name}" for tid, name, _ in substring_matches)
        raise ValueError(f"Ambiguous reference '{ref}' matches {len(substring_matches)} threads:\n{matches_str}")

    raise ValueError(f"Thread not found: {ref}")

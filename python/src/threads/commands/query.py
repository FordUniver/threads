"""Query commands: list, read, stats."""

import json
from collections import Counter
from pathlib import Path

from ..models import TERMINAL_STATUSES, Thread
from ..storage import find_threads, load_thread
from ..workspace import get_workspace, parse_thread_path


def cmd_list(
    path: str | None = None,
    recursive: bool = False,
    include_closed: bool = False,
    search: str | None = None,
    status_filter: str | None = None,
    json_output: bool = False,
) -> None:
    """List threads with filtering options."""
    workspace = get_workspace()

    # Parse path filter into category/project
    category_filter: str | None = None
    project_filter: str | None = None

    if path:
        if (workspace / path).is_dir():
            parts = path.split("/")
            category_filter = parts[0] if parts else None
            project_filter = parts[1] if len(parts) > 1 else None
        else:
            # Treat as search if path doesn't exist
            search = path
            path = None

    # Collect matching threads
    threads: list[tuple[Thread, str | None, str | None]] = []

    for file_path in find_threads(workspace):
        thread = load_thread(file_path)
        category, project = parse_thread_path(file_path, workspace)

        # Store for filtering
        thread.category = category
        thread.project = project

        # Category filter
        if category_filter and category != category_filter:
            continue

        # Project filter
        if project_filter and project != project_filter:
            continue

        # Non-recursive: only threads at current hierarchy level
        if not recursive:
            if project_filter:
                pass  # Already at project level
            elif category_filter:
                if project is not None:
                    continue
            else:
                if category is not None:
                    continue

        # Search filter
        if search:
            search_lower = search.lower()
            if not any(
                search_lower in (s or "").lower()
                for s in [thread.name, thread.desc]
            ):
                continue

        # Status filter
        base = thread.base_status()
        if status_filter:
            if base not in status_filter.split(","):
                continue
        else:
            if not include_closed and thread.is_terminal():
                continue

        threads.append((thread, category, project))

    # Output
    if json_output:
        output_json(threads)
    else:
        output_table(threads, category_filter, project_filter, recursive, status_filter, include_closed)


def output_json(threads: list[tuple[Thread, str | None, str | None]]) -> None:
    """Output threads as JSON."""
    data = []
    for thread, category, project in threads:
        data.append({
            "id": thread.id,
            "status": thread.status,
            "category": category or "-",
            "project": project or "-",
            "name": thread.name,
            "title": thread.name,
            "desc": thread.desc,
        })
    print(json.dumps(data, indent=2))


def output_table(
    threads: list[tuple[Thread, str | None, str | None]],
    category_filter: str | None,
    project_filter: str | None,
    recursive: bool,
    status_filter: str | None,
    include_closed: bool,
) -> None:
    """Output threads as formatted table."""
    # Header
    if project_filter and category_filter:
        level_desc = f"project-level ({category_filter}/{project_filter})"
    elif category_filter:
        level_desc = f"category-level ({category_filter})"
    else:
        level_desc = "workspace-level"

    recursive_suffix = " (including nested)" if recursive else ""

    if status_filter:
        status_desc = status_filter
    elif include_closed:
        status_desc = ""
    else:
        status_desc = "active"

    if status_desc:
        print(f"Showing {len(threads)} {status_desc} {level_desc} threads{recursive_suffix}")
    else:
        print(f"Showing {len(threads)} {level_desc} threads (all statuses){recursive_suffix}")
    print()

    if not threads:
        if not recursive:
            print("Hint: use -r to include nested categories/projects")
        return

    # Table header
    print(f"{'ID':<6} {'STATUS':<10} {'CATEGORY':<18} {'PROJECT':<22} NAME")
    print(f"{'--':<6} {'------':<10} {'--------':<18} {'-------':<22} ----")

    # Table rows
    for thread, category, project in threads:
        cat_str = (category or "-")[:16] + ("…" if category and len(category) > 16 else "")
        proj_str = (project or "-")[:20] + ("…" if project and len(project) > 20 else "")
        print(f"{thread.id:<6} {thread.base_status():<10} {cat_str:<18} {proj_str:<22} {thread.name}")


def cmd_read(ref: str) -> None:
    """Read and print a thread's content."""
    from ..workspace import find_thread_by_ref

    workspace = get_workspace()

    # Try to find by ID/name
    try:
        file_path = find_thread_by_ref(ref, workspace)
    except ValueError:
        # Try as direct path
        if Path(ref).is_file():
            file_path = Path(ref)
        elif (workspace / ref).is_file():
            file_path = workspace / ref
        else:
            raise

    print(file_path.read_text())


def cmd_path(ref: str) -> None:
    """Print the absolute path of a thread file."""
    from ..workspace import find_thread_by_ref

    workspace = get_workspace()

    # Try to find by ID/name
    try:
        file_path = find_thread_by_ref(ref, workspace)
    except ValueError:
        # Try as direct path
        if Path(ref).is_file():
            file_path = Path(ref)
        elif (workspace / ref).is_file():
            file_path = workspace / ref
        else:
            raise

    print(file_path.resolve())


def cmd_stats(path: str | None = None, recursive: bool = False) -> None:
    """Show thread count by status."""
    workspace = get_workspace()

    # Parse path filter
    category_filter: str | None = None
    project_filter: str | None = None

    if path and (workspace / path).is_dir():
        parts = path.split("/")
        category_filter = parts[0] if parts else None
        project_filter = parts[1] if len(parts) > 1 else None

    # Count by status
    counts: Counter[str] = Counter()

    for file_path in find_threads(workspace):
        category, project = parse_thread_path(file_path, workspace)

        # Category filter
        if category_filter and category != category_filter:
            continue

        # Project filter
        if project_filter and project != project_filter:
            continue

        # Non-recursive filtering
        if not recursive:
            if project_filter:
                pass
            elif category_filter:
                if project is not None:
                    continue
            else:
                if category is not None:
                    continue

        thread = load_thread(file_path)
        counts[thread.base_status()] += 1

    # Header
    if project_filter and category_filter:
        scope_desc = f"project-level ({category_filter}/{project_filter})"
    elif category_filter:
        scope_desc = f"category-level ({category_filter})"
    else:
        scope_desc = "workspace-level"

    recursive_suffix = " (including nested)" if recursive else ""
    print(f"Stats for {scope_desc} threads{recursive_suffix}")
    print()

    total = sum(counts.values())
    if total == 0:
        print("No threads found.")
        if not recursive:
            print("Hint: use -r to include nested categories/projects")
        return

    # Table
    print("| Status     | Count |")
    print("|------------|-------|")
    for status, count in counts.most_common():
        print(f"| {status:<10} | {count:>5} |")
    print("|------------|-------|")
    print(f"| {'Total':<10} | {total:>5} |")

"""Query commands: list, read, stats."""

import json
import os
from collections import Counter
from dataclasses import dataclass
from pathlib import Path
from typing import Optional

import yaml

from ..models import Thread
from ..output import OutputFormat, parse_format, resolve_format
from ..storage import FindOptions, find_threads, find_threads_with_options, load_thread
from ..workspace import (
    get_workspace,
    infer_scope,
    parse_thread_path,
    pwd_relative_to_git_root,
)


@dataclass
class SearchDirection:
    """Describes the search direction for output display."""

    has_down: bool = False
    down_depth: int = -1  # -1 = unlimited
    has_up: bool = False
    up_depth: int = -1  # -1 = unlimited (to git root)

    def description(self) -> str:
        parts = []

        if self.has_down:
            if self.down_depth < 0:
                parts.append("recursive")
            else:
                parts.append(f"down {self.down_depth}")

        if self.has_up:
            if self.up_depth < 0:
                parts.append("up")
            else:
                parts.append(f"up {self.up_depth}")

        if not parts:
            return ""
        return f" ({', '.join(parts)})"

    def is_searching(self) -> bool:
        return self.has_down or self.has_up


def cmd_list(
    path: str | None = None,
    down: Optional[int] = None,
    recursive: bool = False,
    up: Optional[int] = None,
    no_git_bound_down: bool = False,
    no_git_bound_up: bool = False,
    include_closed: bool = False,
    search: str | None = None,
    status_filter: str | None = None,
    format_str: str = "fancy",
    json_output: bool = False,
) -> None:
    """List threads with filtering options."""
    git_root = get_workspace()

    # Determine output format (handle --json shorthand)
    if json_output:
        fmt = OutputFormat.JSON
    else:
        fmt = resolve_format(parse_format(format_str))

    # Resolve the scope
    scope = infer_scope(path, git_root)
    filter_path = scope.path
    start_path = scope.threads_dir.parent

    # Determine search direction: --down/-d takes priority, then -r as alias
    has_down = down is not None or recursive
    down_depth = down if down is not None else -1

    has_up = up is not None
    up_depth = up if up is not None else -1

    # Build FindOptions
    options = FindOptions.new()
    options.no_git_bound_down = no_git_bound_down
    options.no_git_bound_up = no_git_bound_up

    if has_down:
        options.down = down_depth
    if has_up:
        options.up = up_depth

    # Track search direction for output
    search_dir = SearchDirection(
        has_down=has_down,
        down_depth=down_depth,
        has_up=has_up,
        up_depth=up_depth,
    )

    # Get PWD relative path for comparison
    pwd_rel = pwd_relative_to_git_root(git_root)

    # Determine if we need absolute paths (for json/yaml)
    include_absolute = fmt in (OutputFormat.JSON, OutputFormat.YAML)

    # Find threads using options
    all_threads = find_threads_with_options(start_path, git_root, options)

    # Collect matching threads
    threads_info: list[dict] = []

    for file_path in all_threads:
        thread = load_thread(file_path)
        rel_path = parse_thread_path(file_path, git_root)

        # Path filter: if not searching, only show threads at the specified level
        if not search_dir.is_searching():
            if rel_path != filter_path:
                continue
        # Note: find_threads_with_options already handles direction/depth filtering

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
        if status_filter is not None:
            if status_filter == "":
                continue
            if base not in status_filter.split(","):
                continue
        else:
            if not include_closed and thread.is_terminal():
                continue

        is_pwd = rel_path == pwd_rel

        info = {
            "id": thread.id,
            "status": base,
            "path": rel_path,
            "name": thread.name,
            "title": thread.name,
            "desc": thread.desc,
            "is_pwd": is_pwd,
        }

        if include_absolute:
            info["path_absolute"] = str(file_path)

        threads_info.append(info)

    # Output
    if fmt == OutputFormat.FANCY:
        output_fancy(threads_info, git_root, filter_path, pwd_rel, search_dir, status_filter, include_closed)
    elif fmt == OutputFormat.PLAIN:
        output_plain(threads_info, git_root, filter_path, pwd_rel, search_dir, status_filter, include_closed)
    elif fmt == OutputFormat.JSON:
        output_json_list(threads_info, git_root, pwd_rel)
    elif fmt == OutputFormat.YAML:
        output_yaml_list(threads_info, git_root, pwd_rel)


def output_fancy(
    threads: list[dict],
    git_root: Path,
    filter_path: str,
    pwd_rel: str,
    search_dir: SearchDirection,
    status_filter: str | None,
    include_closed: bool,
) -> None:
    """Output threads in fancy format."""
    # Fancy header: repo-name (rel/path/to/pwd)
    repo_name = git_root.name

    path_desc = "" if filter_path == "." else f" ({filter_path})"
    pwd_marker = " ← PWD" if filter_path == pwd_rel else ""

    print(f"{repo_name}{path_desc}{pwd_marker}")
    print()

    if status_filter:
        status_desc = f"{status_filter} "
    elif include_closed:
        status_desc = ""
    else:
        status_desc = "active "

    search_suffix = search_dir.description()

    print(f"Showing {len(threads)} {status_desc}threads{search_suffix}")
    print()

    if not threads:
        if not search_dir.is_searching():
            print("Hint: use -r to include nested directories, -u to search parents")
        return

    # Table header
    print(f"{'ID':<6} {'STATUS':<10} {'PATH':<24} NAME")
    print(f"{'--':<6} {'------':<10} {'----':<24} ----")

    # Table rows
    for t in threads:
        path_display = t["path"][:22] + "…" if len(t["path"]) > 22 else t["path"]
        pwd_marker = " ←" if t["is_pwd"] else ""
        print(f"{t['id']:<6} {t['status']:<10} {path_display:<24} {t['title']}{pwd_marker}")


def output_plain(
    threads: list[dict],
    git_root: Path,
    filter_path: str,
    pwd_rel: str,
    search_dir: SearchDirection,
    status_filter: str | None,
    include_closed: bool,
) -> None:
    """Output threads in plain format with explicit headers."""
    # Plain header: explicit context
    pwd = os.getcwd()
    print(f"PWD: {pwd}")
    print(f"Git root: {git_root}")
    print(f"PWD (git-relative): {pwd_rel}")
    print()

    path_desc = "repo root" if filter_path == "." else filter_path

    if status_filter:
        status_desc = status_filter
    elif include_closed:
        status_desc = ""
    else:
        status_desc = "active"

    search_suffix = search_dir.description()
    pwd_suffix = " ← PWD" if filter_path == pwd_rel else ""

    if status_desc:
        print(f"Showing {len(threads)} {status_desc} threads in {path_desc}{search_suffix}{pwd_suffix}")
    else:
        print(f"Showing {len(threads)} threads in {path_desc} (all statuses){search_suffix}{pwd_suffix}")
    print()

    if not threads:
        if not search_dir.is_searching():
            print("Hint: use -r to include nested directories, -u to search parents")
        return

    # Table header
    print(f"{'ID':<6} {'STATUS':<10} {'PATH':<24} NAME")
    print(f"{'--':<6} {'------':<10} {'----':<24} ----")

    # Table rows
    for t in threads:
        path_display = t["path"][:22] + "…" if len(t["path"]) > 22 else t["path"]
        pwd_marker = " ← PWD" if t["is_pwd"] else ""
        print(f"{t['id']:<6} {t['status']:<10} {path_display:<24} {t['title']}{pwd_marker}")


def output_json_list(threads: list[dict], git_root: Path, pwd_rel: str) -> None:
    """Output threads as JSON."""
    pwd = os.getcwd()
    data = {
        "pwd": pwd,
        "git_root": str(git_root),
        "pwd_relative": pwd_rel,
        "threads": threads,
    }
    print(json.dumps(data, indent=2))


def output_yaml_list(threads: list[dict], git_root: Path, pwd_rel: str) -> None:
    """Output threads as YAML."""
    pwd = os.getcwd()
    data = {
        "pwd": pwd,
        "git_root": str(git_root),
        "pwd_relative": pwd_rel,
        "threads": threads,
    }
    print(yaml.dump(data, default_flow_style=False, sort_keys=False), end="")


def cmd_read(ref: str) -> None:
    """Read and print a thread's content."""
    from ..workspace import find_thread_by_ref

    git_root = get_workspace()

    # Try to find by ID/name
    try:
        file_path = find_thread_by_ref(ref, git_root)
    except ValueError:
        # Try as direct path
        if Path(ref).is_file():
            file_path = Path(ref)
        elif (git_root / ref).is_file():
            file_path = git_root / ref
        else:
            raise

    print(file_path.read_text())


def cmd_path(ref: str) -> None:
    """Print the absolute path of a thread file."""
    from ..workspace import find_thread_by_ref

    git_root = get_workspace()

    # Try to find by ID/name
    try:
        file_path = find_thread_by_ref(ref, git_root)
    except ValueError:
        # Try as direct path
        if Path(ref).is_file():
            file_path = Path(ref)
        elif (git_root / ref).is_file():
            file_path = git_root / ref
        else:
            raise

    print(file_path.resolve())


def cmd_stats(
    path: str | None = None,
    down: Optional[int] = None,
    recursive: bool = False,
    up: Optional[int] = None,
    no_git_bound_down: bool = False,
    no_git_bound_up: bool = False,
    format_str: str = "fancy",
    json_output: bool = False,
) -> None:
    """Show thread count by status."""
    git_root = get_workspace()

    # Determine output format (handle --json shorthand)
    if json_output:
        fmt = OutputFormat.JSON
    else:
        fmt = resolve_format(parse_format(format_str))

    # Resolve the scope
    scope = infer_scope(path, git_root)
    filter_path = scope.path
    start_path = scope.threads_dir.parent

    # Determine search direction
    has_down = down is not None or recursive
    down_depth = down if down is not None else -1

    has_up = up is not None
    up_depth = up if up is not None else -1

    # Build FindOptions
    options = FindOptions.new()
    options.no_git_bound_down = no_git_bound_down
    options.no_git_bound_up = no_git_bound_up

    if has_down:
        options.down = down_depth
    if has_up:
        options.up = up_depth

    # Track search direction for output
    search_dir = SearchDirection(
        has_down=has_down,
        down_depth=down_depth,
        has_up=has_up,
        up_depth=up_depth,
    )

    # Find threads using options
    all_threads = find_threads_with_options(start_path, git_root, options)

    # Count by status
    counts: Counter[str] = Counter()

    for file_path in all_threads:
        rel_path = parse_thread_path(file_path, git_root)

        # Path filter: if not searching, only count threads at the specified level
        if not search_dir.is_searching():
            if rel_path != filter_path:
                continue
        # Note: find_threads_with_options already handles direction/depth filtering

        thread = load_thread(file_path)
        counts[thread.base_status()] += 1

    # Output based on format
    if fmt == OutputFormat.FANCY:
        stats_output_fancy(counts, filter_path, search_dir)
    elif fmt == OutputFormat.PLAIN:
        stats_output_plain(counts, git_root, filter_path, search_dir)
    elif fmt == OutputFormat.JSON:
        stats_output_json(counts, git_root, filter_path)
    elif fmt == OutputFormat.YAML:
        stats_output_yaml(counts, git_root, filter_path)


def stats_output_fancy(counts: Counter[str], filter_path: str, search_dir: SearchDirection) -> None:
    """Output stats in fancy format."""
    # Header
    path_desc = "repo root" if filter_path == "." else filter_path
    search_suffix = search_dir.description()

    print(f"Stats for threads in {path_desc}{search_suffix}")
    print()

    total = sum(counts.values())
    if total == 0:
        print("No threads found.")
        if not search_dir.is_searching():
            print("Hint: use -r to include nested directories, -u to search parents")
        return

    # Table
    print("| Status     | Count |")
    print("|------------|-------|")
    for status, count in counts.most_common():
        print(f"| {status:<10} | {count:>5} |")
    print("|------------|-------|")
    print(f"| {'Total':<10} | {total:>5} |")


def stats_output_plain(counts: Counter[str], git_root: Path, filter_path: str, search_dir: SearchDirection) -> None:
    """Output stats in plain format."""
    pwd = os.getcwd()
    print(f"PWD: {pwd}")
    print(f"Git root: {git_root}")
    print()

    path_desc = "repo root" if filter_path == "." else filter_path
    search_suffix = search_dir.description()

    print(f"Stats for threads in {path_desc}{search_suffix}")
    print()

    total = sum(counts.values())
    if total == 0:
        print("No threads found.")
        if not search_dir.is_searching():
            print("Hint: use -r to include nested directories, -u to search parents")
        return

    # Table
    print("| Status     | Count |")
    print("|------------|-------|")
    for status, count in counts.most_common():
        print(f"| {status:<10} | {count:>5} |")
    print("|------------|-------|")
    print(f"| {'Total':<10} | {total:>5} |")


def stats_output_json(counts: Counter[str], git_root: Path, filter_path: str) -> None:
    """Output stats as JSON."""
    data = {
        "git_root": str(git_root),
        "path": filter_path,
        "counts": [{"status": status, "count": count} for status, count in counts.most_common()],
        "total": sum(counts.values()),
    }
    print(json.dumps(data, indent=2))


def stats_output_yaml(counts: Counter[str], git_root: Path, filter_path: str) -> None:
    """Output stats as YAML."""
    data = {
        "git_root": str(git_root),
        "path": filter_path,
        "counts": [{"status": status, "count": count} for status, count in counts.most_common()],
        "total": sum(counts.values()),
    }
    print(yaml.dump(data, default_flow_style=False, sort_keys=False), end="")

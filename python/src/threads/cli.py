"""CLI entry point for threads."""

import argparse
import subprocess
import sys
from typing import NoReturn

import argcomplete


def thread_id_completer(prefix, parsed_args, **kwargs):
    """Complete thread IDs with their names as descriptions."""
    try:
        from .storage import find_threads, load_thread
        from .workspace import get_workspace

        ws = get_workspace()
        threads = find_threads(ws)
        completions = []
        for path in threads:
            thread = load_thread(path)
            thread_id = thread.id or "????"
            completions.append(thread_id)
        return completions
    except Exception:
        return []


class ArgumentParserExitCode1(argparse.ArgumentParser):
    """ArgumentParser that exits with code 1 instead of 2 for consistency with other implementations."""

    def error(self, message: str) -> NoReturn:
        """Print error and exit with code 1."""
        self.print_usage(sys.stderr)
        self.exit(1, f"{self.prog}: error: {message}\n")


USAGE = """\
Usage: threads <command> [options]

Workspace operations:
  threads list [path] [-r] [--search=X]    List threads (current level by default)
  threads new [path] "Title" [opts]        Create new thread (infers from cwd)
  threads move <id> <path>                 Move thread to new location
  threads commit --pending [-m msg]        Commit all modified threads
  threads validate [path]                  Validate thread files
  threads git                              Show pending thread changes
  threads stats [path] [-r]                Show thread count by status

Single-thread operations:
  threads read <id>                        Read thread content
  threads path <id>                        Print thread file path
  threads status <id> <new-status>         Change thread status
  threads update <id> --title=X            Update thread title/desc
  threads body <id> [--set|--append]       Edit Body section (stdin)
  threads note <id> add "text"             Add note (returns hash)
  threads todo <id> add "item"             Add todo item (returns hash)
  threads log <id> "entry"                 Add log entry
  threads resolve <id>                     Mark thread resolved
  threads reopen <id> [--status=X]         Reopen resolved thread
  threads remove <id>                      Remove thread entirely (alias: rm)
  threads commit <id> [-m msg]             Commit single thread

Options for 'list':
  -r, --recursive       Include nested categories/projects
  --search=X            Search name/title/desc (substring)
  --include-closed      Include resolved/terminal threads

Options for 'new':
  --status=X      Initial status (default: idea)
  --desc=X        One-line description
  --body=X        Initial body content

Path notation:
  .               Workspace level
  admin           Category level
  admin/ssot      Project level
  (no path)       Infer from current directory

Status values:
  Active:   idea, planning, active, blocked, paused
  Terminal: resolved, superseded, deferred
"""


def build_parser() -> argparse.ArgumentParser:
    """Build the argument parser."""
    parser = ArgumentParserExitCode1(
        prog="threads",
        description="Thread management CLI for LLM workflows",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=USAGE,
    )
    subparsers = parser.add_subparsers(dest="command")

    # list
    p_list = subparsers.add_parser("list", help="List threads")
    p_list.add_argument("path", nargs="?", help="Filter by path")
    p_list.add_argument("-r", "--recursive", action="store_true", help="Include nested")
    p_list.add_argument("--include-closed", action="store_true", dest="include_closed", help="Include terminal")
    p_list.add_argument("-s", "--search", help="Search filter")
    p_list.add_argument("--status", help="Status filter")
    p_list.add_argument("-f", "--format", choices=["fancy", "plain", "json", "yaml"], default="fancy", dest="format_str", help="Output format")
    p_list.add_argument("--json", action="store_true", dest="json_output", help="JSON output (shorthand for --format=json)")

    # ls (alias for list)
    p_ls = subparsers.add_parser("ls", help="List threads (alias for list)")
    p_ls.add_argument("path", nargs="?", help="Filter by path")
    p_ls.add_argument("-r", "--recursive", action="store_true", help="Include nested")
    p_ls.add_argument("--include-closed", action="store_true", dest="include_closed", help="Include terminal")
    p_ls.add_argument("-s", "--search", help="Search filter")
    p_ls.add_argument("--status", help="Status filter")
    p_ls.add_argument("-f", "--format", choices=["fancy", "plain", "json", "yaml"], default="fancy", dest="format_str", help="Output format")
    p_ls.add_argument("--json", action="store_true", dest="json_output", help="JSON output (shorthand for --format=json)")

    # new
    p_new = subparsers.add_parser("new", help="Create new thread")
    p_new.add_argument("path_or_title", nargs="?", help="Path or title")
    p_new.add_argument("title", nargs="?", help="Title if path given")
    p_new.add_argument("--desc", default="", help="Description")
    p_new.add_argument("--status", default="idea", help="Initial status")
    p_new.add_argument("--body", help="Initial body content")
    p_new.add_argument("--commit", action="store_true", dest="do_commit", help="Auto-commit")
    p_new.add_argument("-m", dest="message", help="Commit message")

    # read
    p_read = subparsers.add_parser("read", help="Read thread content")
    p_read.add_argument("ref", help="Thread ID or name").completer = thread_id_completer

    # path
    p_path = subparsers.add_parser("path", help="Print thread file path")
    p_path.add_argument("ref", help="Thread ID or name").completer = thread_id_completer

    # status
    p_status = subparsers.add_parser("status", help="Change thread status")
    p_status.add_argument("ref", help="Thread ID or name").completer = thread_id_completer
    p_status.add_argument("new_status", help="New status")
    p_status.add_argument("--commit", action="store_true", dest="do_commit", help="Auto-commit")
    p_status.add_argument("-m", dest="message", help="Commit message")

    # update
    p_update = subparsers.add_parser("update", help="Update thread title/desc")
    p_update.add_argument("ref", help="Thread ID or name").completer = thread_id_completer
    p_update.add_argument("--title", help="New title")
    p_update.add_argument("--desc", help="New description")
    p_update.add_argument("--commit", action="store_true", dest="do_commit", help="Auto-commit")
    p_update.add_argument("-m", dest="message", help="Commit message")

    # body
    p_body = subparsers.add_parser("body", help="Edit body section (stdin)")
    p_body.add_argument("ref", help="Thread ID or name").completer = thread_id_completer
    p_body.add_argument("--set", action="store_const", const="set", dest="mode", default="set")
    p_body.add_argument("--append", action="store_const", const="append", dest="mode")
    p_body.add_argument("--commit", action="store_true", dest="do_commit", help="Auto-commit")
    p_body.add_argument("-m", dest="message", help="Commit message")

    # note
    p_note = subparsers.add_parser("note", help="Manage notes")
    p_note.add_argument("ref", help="Thread ID or name").completer = thread_id_completer
    p_note.add_argument("action", choices=["add", "edit", "remove"], help="Action")
    p_note.add_argument("text_or_hash", help="Note text or hash")
    p_note.add_argument("new_text", nargs="?", help="New text for edit")
    p_note.add_argument("--commit", action="store_true", dest="do_commit", help="Auto-commit")
    p_note.add_argument("-m", dest="message", help="Commit message")

    # todo
    p_todo = subparsers.add_parser("todo", help="Manage todo items")
    p_todo.add_argument("ref", help="Thread ID or name").completer = thread_id_completer
    p_todo.add_argument("action", choices=["add", "check", "complete", "done", "uncheck", "remove"])
    p_todo.add_argument("item_or_hash", help="Item text or hash")
    p_todo.add_argument("--commit", action="store_true", dest="do_commit", help="Auto-commit")
    p_todo.add_argument("-m", dest="message", help="Commit message")

    # log
    p_log = subparsers.add_parser("log", help="Add log entry")
    p_log.add_argument("ref", help="Thread ID or name").completer = thread_id_completer
    p_log.add_argument("entry", nargs="?", help="Log entry text")
    p_log.add_argument("--commit", action="store_true", dest="do_commit", help="Auto-commit")
    p_log.add_argument("-m", dest="message", help="Commit message")

    # resolve
    p_resolve = subparsers.add_parser("resolve", help="Mark thread resolved")
    p_resolve.add_argument("ref", help="Thread ID or name").completer = thread_id_completer
    p_resolve.add_argument("--commit", action="store_true", dest="do_commit", help="Auto-commit")
    p_resolve.add_argument("-m", dest="message", help="Commit message")

    # reopen
    p_reopen = subparsers.add_parser("reopen", help="Reopen resolved thread")
    p_reopen.add_argument("ref", help="Thread ID or name").completer = thread_id_completer
    p_reopen.add_argument("--status", default="active", dest="new_status", help="New status")
    p_reopen.add_argument("--commit", action="store_true", dest="do_commit", help="Auto-commit")
    p_reopen.add_argument("-m", dest="message", help="Commit message")

    # remove / rm
    p_remove = subparsers.add_parser("remove", help="Remove thread")
    p_remove.add_argument("ref", help="Thread ID or name").completer = thread_id_completer
    p_remove.add_argument("--commit", action="store_true", dest="do_commit", help="Auto-commit")
    p_remove.add_argument("-m", dest="message", help="Commit message")

    p_rm = subparsers.add_parser("rm", help="Remove thread (alias)")
    p_rm.add_argument("ref", help="Thread ID or name").completer = thread_id_completer
    p_rm.add_argument("--commit", action="store_true", dest="do_commit", help="Auto-commit")
    p_rm.add_argument("-m", dest="message", help="Commit message")

    # move
    p_move = subparsers.add_parser("move", help="Move thread to new location")
    p_move.add_argument("ref", help="Thread ID or name").completer = thread_id_completer
    p_move.add_argument("new_path", help="Destination path")
    p_move.add_argument("--commit", action="store_true", dest="do_commit", help="Auto-commit")
    p_move.add_argument("-m", dest="message", help="Commit message")

    # commit
    p_commit = subparsers.add_parser("commit", help="Commit thread changes")
    p_commit.add_argument("refs", nargs="*", help="Thread IDs").completer = thread_id_completer
    p_commit.add_argument("--pending", action="store_true", help="Commit all modified")
    p_commit.add_argument("-m", dest="message", help="Commit message")
    p_commit.add_argument("--auto", action="store_true", dest="auto_msg", help="Skip confirmation")

    # git
    subparsers.add_parser("git", help="Show pending thread changes")

    # stats
    p_stats = subparsers.add_parser("stats", help="Show thread count by status")
    p_stats.add_argument("path", nargs="?", help="Filter by path")
    p_stats.add_argument("-r", "--recursive", action="store_true", help="Include nested")
    p_stats.add_argument("-f", "--format", choices=["fancy", "plain", "json", "yaml"], default="fancy", dest="format_str", help="Output format")
    p_stats.add_argument("--json", action="store_true", dest="json_output", help="JSON output (shorthand for --format=json)")

    # validate
    p_validate = subparsers.add_parser("validate", help="Validate thread files")
    p_validate.add_argument("path", nargs="?", help="Specific file or directory")
    p_validate.add_argument("-r", "--recursive", action="store_true", help="Validate recursively")

    # completion
    p_completion = subparsers.add_parser("completion", help="Generate shell completion script")
    p_completion.add_argument(
        "shell",
        choices=["bash", "zsh", "fish"],
        help="Shell to generate completions for",
    )

    return parser


def _generate_completion(shell: str) -> int:
    """Generate shell completion script using register-python-argcomplete."""
    try:
        result = subprocess.run(
            ["register-python-argcomplete", "--shell", shell, "threads"],
            capture_output=True,
            text=True,
            check=True,
        )
        print(result.stdout, end="")
        return 0
    except FileNotFoundError:
        print("Error: register-python-argcomplete not found", file=sys.stderr)
        print("Install with: pip install argcomplete", file=sys.stderr)
        return 1
    except subprocess.CalledProcessError as e:
        print(f"Error: {e.stderr}", file=sys.stderr)
        return 1


def main() -> int:
    """Main entry point."""
    parser = build_parser()
    argcomplete.autocomplete(parser)
    args = parser.parse_args()

    if args.command is None:
        print(USAGE)
        return 1

    try:
        if args.command == "list" or args.command == "ls":
            from .commands.query import cmd_list
            cmd_list(
                path=args.path,
                recursive=args.recursive,
                include_closed=args.include_closed,
                search=args.search,
                status_filter=args.status,
                format_str=args.format_str,
                json_output=args.json_output,
            )

        elif args.command == "new":
            from .commands.crud import cmd_new
            # Handle [path] title positional args
            if args.title:
                path = args.path_or_title
                title = args.title
            else:
                path = None
                title = args.path_or_title

            if not title:
                print("Usage: threads new [path] <title> [--status=X] [--desc=X]", file=sys.stderr)
                return 1

            cmd_new(
                title=title,
                path=path,
                desc=args.desc,
                status=args.status,
                body=args.body,
                do_commit=args.do_commit,
                message=args.message,
            )

        elif args.command == "read":
            from .commands.query import cmd_read
            cmd_read(args.ref)

        elif args.command == "path":
            from .commands.query import cmd_path
            cmd_path(args.ref)

        elif args.command == "status":
            from .commands.lifecycle import cmd_status
            cmd_status(
                ref=args.ref,
                new_status=args.new_status,
                do_commit=args.do_commit,
                message=args.message,
            )

        elif args.command == "update":
            from .commands.crud import cmd_update
            cmd_update(
                ref=args.ref,
                title=args.title,
                desc=args.desc,
                do_commit=args.do_commit,
                message=args.message,
            )

        elif args.command == "body":
            from .commands.sections import cmd_body
            cmd_body(
                ref=args.ref,
                mode=args.mode,
                do_commit=args.do_commit,
                message=args.message,
            )

        elif args.command == "note":
            from .commands.sections import cmd_note
            cmd_note(
                ref=args.ref,
                action=args.action,
                text_or_hash=args.text_or_hash,
                new_text=args.new_text,
                do_commit=args.do_commit,
                message=args.message,
            )

        elif args.command == "todo":
            from .commands.sections import cmd_todo
            cmd_todo(
                ref=args.ref,
                action=args.action,
                item_or_hash=args.item_or_hash,
                do_commit=args.do_commit,
                message=args.message,
            )

        elif args.command == "log":
            from .commands.sections import cmd_log
            cmd_log(
                ref=args.ref,
                entry=args.entry,
                do_commit=args.do_commit,
                message=args.message,
            )

        elif args.command == "resolve":
            from .commands.lifecycle import cmd_resolve
            cmd_resolve(
                ref=args.ref,
                do_commit=args.do_commit,
                message=args.message,
            )

        elif args.command == "reopen":
            from .commands.lifecycle import cmd_reopen
            cmd_reopen(
                ref=args.ref,
                new_status=args.new_status,
                do_commit=args.do_commit,
                message=args.message,
            )

        elif args.command in ("remove", "rm"):
            from .commands.crud import cmd_remove
            cmd_remove(
                ref=args.ref,
                do_commit=args.do_commit,
                message=args.message,
            )

        elif args.command == "move":
            from .commands.lifecycle import cmd_move
            cmd_move(
                ref=args.ref,
                new_path=args.new_path,
                do_commit=args.do_commit,
                message=args.message,
            )

        elif args.command == "commit":
            from .commands.lifecycle import cmd_commit
            cmd_commit(
                refs=args.refs if args.refs else None,
                pending=args.pending,
                message=args.message,
                auto_msg=args.auto_msg,
            )

        elif args.command == "git":
            from .commands.lifecycle import cmd_git
            cmd_git()

        elif args.command == "stats":
            from .commands.query import cmd_stats
            cmd_stats(
                path=args.path,
                recursive=args.recursive,
                format_str=args.format_str,
                json_output=args.json_output,
            )

        elif args.command == "validate":
            from .commands.lifecycle import cmd_validate
            if not cmd_validate(args.path, recursive=args.recursive):
                return 1

        elif args.command == "completion":
            return _generate_completion(args.shell)

        else:
            print(f"Unknown command: {args.command}", file=sys.stderr)
            return 1

    except ValueError as e:
        print(f"ERROR: {e}", file=sys.stderr)
        return 1
    except Exception as e:
        print(f"ERROR: {e}", file=sys.stderr)
        return 1

    return 0


if __name__ == "__main__":
    sys.exit(main())

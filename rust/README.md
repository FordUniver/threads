# threads CLI - Rust Implementation

A high-performance Rust implementation of the `threads` CLI for managing markdown-based persistent threads in LLM workflows.

## Building

```bash
# Debug build
cargo build

# Release build (optimized, stripped)
cargo build --release

# Binary location
./target/release/threads
```

## Features

- Fast startup (native binary)
- Full compatibility with shell/Go implementations
- All 78 integration tests pass

## Commands

### Workspace Operations

```bash
threads list [path] [-r] [--search=X] [--status=X] [--all] [--json]
threads new [path] "Title" [--status=X] [--desc=X]
threads move <id> <path>
threads commit [--pending | <id>] [-m msg]
threads validate [path] [-r]
threads git
threads stats [path] [-r]
```

### Thread Operations

```bash
threads read <id>
threads status <id> <new-status>
threads update <id> [--title=X] [--desc=X]
threads body <id> [--set|--append]      # reads from stdin
threads note <id> add|edit|remove <text|hash>
threads todo <id> add|check|uncheck|remove <text|hash>
threads log <id> "entry"
threads resolve <id>
threads reopen <id> [--status=X]
threads remove <id>                     # alias: rm
```

## Running Tests

```bash
cd /path/to/threads.glab.repo
./test/run_tests.sh "./rust/target/release/threads"
```

## Project Structure

```
rust/
├── Cargo.toml
├── README.md
└── src/
    ├── main.rs          # CLI entry point, argument parsing
    ├── workspace.rs     # Workspace discovery and thread finding
    ├── thread.rs        # Thread parsing, section manipulation
    ├── git.rs           # Git operations
    └── cmd/             # Subcommand implementations
        ├── mod.rs
        ├── list.rs
        ├── new.rs
        ├── read.rs
        ├── status.rs
        ├── update.rs
        ├── body.rs
        ├── note.rs
        ├── todo.rs
        ├── log.rs
        ├── resolve.rs
        ├── reopen.rs
        ├── remove.rs
        ├── move_cmd.rs
        ├── commit.rs
        ├── validate.rs
        ├── git_cmd.rs
        └── stats.rs
```

## Dependencies

- `clap` - Command-line argument parsing
- `serde` + `serde_yaml` - YAML frontmatter parsing
- `serde_json` - JSON output for list command
- `chrono` - Date/time formatting for log entries
- `rand` - ID generation
- `regex` - Pattern matching for sections
- `md-5` - Hash generation for notes/todos
- `libc` - TTY detection for stdin handling

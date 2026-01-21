# threads

CLI tool for managing persistent context threads in LLM-assisted development workflows.

Threads are markdown files in `.threads/` directories. Each thread tracks a single topic: a feature, bug, exploration, or decision. Threads support structured sections for body content, notes, todos, and a chronological log.

## Installation

```bash
# Build from source (requires Rust)
cargo build --release
cp target/release/threads ~/.local/bin/

# Or install directly
cargo install --git https://git.zib.de/cspiegel/threads.git
```

## Quick Start

```bash
# Create a thread
threads new "Add user authentication"

# List threads at current level
threads list

# Add a todo item
threads todo abc123 add "Implement JWT validation"

# Log progress
threads log abc123 "Completed initial implementation"

# Mark resolved
threads resolve abc123
```

## Thread Format

Threads are markdown files with YAML frontmatter:

```markdown
---
id: abc123
name: Add user authentication
desc: Implement JWT-based auth for API endpoints
status: active
---

## Body

Implementation notes and context...

## Todo

- [ ] Implement JWT validation <!-- abc1 -->
- [x] Set up middleware <!-- def2 -->

## Notes

- Consider refresh token rotation <!-- 1a2b -->

## Log

### 2026-01-20

- **14:30** Created thread.
- **15:45** Completed middleware setup.
```

## Path Resolution

Threads uses the git repository root as the workspace boundary. Path arguments follow these rules:

| Pattern | Resolution |
|---------|------------|
| (none) | Current directory |
| `.` | Current directory (explicit) |
| `./X/Y` | Relative to current directory |
| `/X/Y` | Absolute path |
| `X/Y` | Relative to git root |

Nested git repositories are respected as boundaries: the tool won't traverse into or out of them.

## Commands

### Workspace Operations

| Command | Description |
|---------|-------------|
| `list [path]` | List threads (aliases: `ls`) |
| `new [path] <title>` | Create a new thread |
| `move <id> <path>` | Move thread to new location |
| `commit [ids...]` | Commit thread changes |
| `git` | Show pending thread changes |
| `stats [path]` | Show thread count by status |
| `validate [path]` | Validate thread files |

### Thread Operations

| Command | Description |
|---------|-------------|
| `read <id>` | Read thread content |
| `path <id>` | Print thread file path |
| `status <id> <status>` | Change thread status |
| `update <id>` | Update thread title/desc |
| `body <id>` | Edit body section (stdin) |
| `note <id> <action>` | Manage notes (add/edit/remove) |
| `todo <id> <action>` | Manage todos (add/check/uncheck/remove) |
| `log <id> <entry>` | Add timestamped log entry |
| `resolve <id>` | Mark thread resolved |
| `reopen <id>` | Reopen resolved thread |
| `remove <id>` | Remove thread entirely |

### Directional Search

The `list` and `stats` commands support directional search:

```bash
# Search subdirectories (N levels, 0=unlimited)
threads list --down 2
threads list -d 0

# Search parent directories (N levels, 0=to git root)
threads list --up 1
threads list -u 0

# Recursive alias (unlimited depth down)
threads list -r
```

### Status Values

**Active:** `idea`, `planning`, `active`, `blocked`, `paused`

**Terminal:** `resolved`, `superseded`, `deferred`, `rejected`

Blocked status supports reasons: `blocked (waiting on review)`

## Output Formats

All commands support multiple output formats:

```bash
threads list --format fancy   # Default: colored table
threads list --format plain   # Plain text
threads list --format json    # JSON
threads list --format yaml    # YAML
threads list --json           # Shorthand for --format=json
```

## Shell Completion

Generate completion scripts for your shell:

```bash
# Bash
eval "$(threads completion bash)"

# Zsh
eval "$(threads completion zsh)"

# Fish
threads completion fish | source
```

## Development

```bash
# Run all tests
make test

# Run integration tests only
make integration-test

# Run benchmarks
make benchmark
```

### Project Structure

```
threads/
├── src/
│   ├── main.rs             # Entry point and CLI
│   ├── cmd/                # Command implementations
│   ├── git.rs              # Git operations
│   ├── output.rs           # Output formatting
│   ├── thread.rs           # Thread parsing
│   └── workspace.rs        # Path resolution
├── test/
│   ├── cases/              # Integration test cases
│   ├── lib/                # Test utilities
│   └── benchmark/          # Performance benchmarks
├── Cargo.toml
├── Makefile
└── .gitlab-ci.yml
```

## Design Principles

- **Git-native:** Repository root defines the workspace; nested repos are boundaries
- **Hierarchical:** Place `.threads/` at any level for appropriate scope
- **Hash-addressable:** Notes and todos use short hashes for stable references
- **Structured sections:** Body, Todo, Notes, Log with well-defined semantics
- **Machine-readable:** JSON/YAML output for scripting and tooling integration

## License

MIT

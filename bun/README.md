# threads CLI (Bun/TypeScript)

A thread management CLI for LLM workflows, implemented in Bun/TypeScript.

## Installation

Requires [Bun](https://bun.sh) v1.0+.

```bash
cd bun
bun install
```

## Usage

```bash
./bin/threads <command> [options]
```

### Commands

**Workspace operations:**
```bash
threads list [path] [-r] [--search=X] [--status=X] [--all] [--json]
threads new [path] "Title" [--status=X] [--desc=X] [--body=X]
threads move <id> <path>
threads commit [--pending | <id>] [-m msg]
threads validate [path]
threads git
threads stats [path] [-r]
```

**Single-thread operations:**
```bash
threads read <id>
threads status <id> <new-status>
threads update <id> [--title=X] [--desc=X]
threads body <id> [--set|--append]      # reads from stdin
threads note <id> add "text"            # prints hash
threads todo <id> add|toggle|remove <ref>
threads log <id> "entry"
threads resolve <id>
threads reopen <id> [--status=X]
threads remove <id>                     # alias: rm
```

## Thread File Format

Threads are markdown files with YAML frontmatter in `.threads/` directories:

```markdown
---
id: abc123
name: Thread title
desc: One-line description
status: active
---

## Body

Optional body content.

## Todo

- [ ] Uncompleted item
- [x] Completed item

## Log

### 2024-01-09

- **14:30** Log entry text.
```

### Key details

- **File naming**: `{id}-{slug}.md` (e.g., `abc123-my-thread.md`)
- **ID**: 6-character lowercase hex, extracted from filename
- **Status values**: `idea`, `planning`, `active`, `blocked`, `paused` (active) or `resolved`, `superseded`, `deferred` (terminal)
- **Status with reason**: `blocked (waiting for X)`

## Workspace Structure

```
$WORKSPACE/
├── .threads/                    # workspace-level threads
├── category/.threads/           # category-level threads
└── category/project/.threads/   # project-level threads
```

The `WORKSPACE` environment variable must point to the root directory.

## Running Tests

```bash
cd /path/to/threads.glab.repo
./test/run_tests.sh ./bun/bin/threads
```

## Project Structure

```
bun/
├── package.json
├── bin/
│   └── threads           # CLI entry point
└── src/
    ├── thread.ts         # Thread parsing and manipulation
    ├── workspace.ts      # Workspace discovery and thread lookup
    ├── section.ts        # Markdown section manipulation
    └── git.ts            # Git operations
```

## License

Same as parent repository.

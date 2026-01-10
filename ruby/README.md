# threads (Ruby Implementation)

A Ruby implementation of the `threads` CLI for managing markdown-based persistent context threads in LLM-assisted development workflows.

## Installation

```bash
# Make executable
chmod +x ruby/bin/threads

# Add to PATH (optional)
export PATH="/path/to/threads.glab.repo/ruby/bin:$PATH"
```

## Requirements

- Ruby 2.7+ (standard library only, no gems required)
- Git (for commit operations)

## Usage

```bash
# Set workspace (required)
export WORKSPACE=/path/to/workspace

# Create a new thread
threads new . "Implement feature X" --desc="Add new feature" --status=active

# List threads
threads list                    # workspace-level, active only
threads list -r                 # recursive (include nested)
threads list --all              # include terminal statuses
threads list --status=blocked   # filter by status
threads list --search="feature" # search in name/title/desc
threads list --json             # JSON output

# Read thread content
threads read abc123

# Update thread
threads status abc123 blocked
threads update abc123 --title="New Title" --desc="New description"

# Body editing (reads from stdin)
echo "New content" | threads body abc123 --set
echo "More content" | threads body abc123 --append

# Notes management
threads note abc123 add "Important note"
threads note abc123 edit da6d "Updated note"
threads note abc123 remove da6d

# Todo management
threads todo abc123 add "Task item"
threads todo abc123 check f1a2
threads todo abc123 uncheck f1a2
threads todo abc123 remove f1a2

# Log entries
threads log abc123 "Made progress on implementation"

# Lifecycle
threads resolve abc123
threads reopen abc123 --status=active
threads remove abc123

# Move thread
threads move abc123 category/project

# Git operations
threads git                     # show pending changes
threads commit abc123           # commit specific thread
threads commit --pending        # commit all pending
threads commit abc123 -m "msg"  # custom message

# Validation
threads validate                # validate all threads
threads validate path/file.md   # validate specific file

# Statistics
threads stats                   # workspace-level
threads stats -r                # recursive
threads stats category          # specific path
```

## Thread File Format

Threads are markdown files with YAML frontmatter stored in `.threads/` directories:

```markdown
---
id: abc123
name: Thread Title
desc: One-line description
status: active
---

## Body

Optional body content.

## Notes

- Note text  <!-- hash -->

## Todo

- [ ] Uncompleted item  <!-- hash -->
- [x] Completed item  <!-- hash -->

## Log

### 2026-01-09

- **14:30** Log entry text.
```

## Workspace Structure

```
$WORKSPACE/
├── .threads/                    # workspace-level threads
├── category/.threads/           # category-level threads
└── category/project/.threads/   # project-level threads
```

## Status Values

**Active statuses:** idea, planning, active, blocked, paused

**Terminal statuses:** resolved, superseded, deferred

Status can include a reason suffix: `blocked (waiting for review)`

## Exit Codes

- 0: Success
- 1: Error
- 2: Ambiguous match (multiple threads match reference)

## Architecture

```
ruby/
├── bin/
│   └── threads              # Main executable
└── lib/
    └── threads/
        ├── workspace.rb     # Workspace detection and utilities
        ├── thread.rb        # Thread parsing and manipulation
        ├── section.rb       # Section manipulation utilities
        ├── git.rb           # Git integration
        └── commands.rb      # Command implementations
```

## Testing

```bash
# Run full test suite
./test/run_tests.sh "./ruby/bin/threads"

# Run specific test file
./test/run_tests.sh "./ruby/bin/threads" test_new.sh
```

## License

Same as parent repository.

# threads (Swift)

Swift implementation of the `threads` CLI for managing markdown-based persistent threads.

## Building

```bash
cd swift
swift build -c release
```

The binary will be at `.build/release/threads`.

## Dependencies

- [swift-argument-parser](https://github.com/apple/swift-argument-parser) - CLI argument parsing
- [Yams](https://github.com/jpsim/Yams) - YAML parsing

## Test Suite

Run the test suite against the built binary:

```bash
cd ..
./test/run_tests.sh "./swift/.build/release/threads"
```

## Commands

### Workspace Operations

```
threads list [path] [-r] [--search=X] [--status=X] [--all] [--json]
threads new [path] "Title" [--status=X] [--desc=X] [--body=X]
threads move <id> <path>
threads commit [--pending | <id>] [-m msg]
threads validate [path] [-r]
threads git
threads stats [path] [-r]
```

### Single-Thread Operations

```
threads read <id>
threads status <id> <new-status>
threads update <id> [--title=X] [--desc=X]
threads body <id> [--set|--append]      # reads from stdin
threads note <id> add "text"            # prints hash
threads todo <id> add|check|uncheck|remove <ref>
threads log <id> "entry"
threads resolve <id>
threads reopen <id> [--status=X]
threads remove <id>                     # alias: rm
```

## Exit Codes

- `0` - Success
- `1` - Error (not found, validation error, etc.)
- `2` - Ambiguous reference (multiple threads match)

## Known Limitations

- Empty string option values must use space syntax (`--desc ""`) rather than equals syntax (`--desc=""`). This is a limitation of Swift ArgumentParser.

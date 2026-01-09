#!/usr/bin/env bash
# Test workspace setup and teardown

# Global test workspace path
TEST_WS=""
_ORIGINAL_PWD=""

# Create isolated test workspace
# Sets TEST_WS and creates .threads/ directory
setup_test_workspace() {
    _ORIGINAL_PWD="$PWD"
    TEST_WS=$(mktemp -d "${TMPDIR:-/tmp}/threads-test.XXXXXX")
    mkdir -p "$TEST_WS/.threads"
    export TEST_WS
    export WORKSPACE="$TEST_WS"
    cd "$TEST_WS" || exit 1

    if [[ -n "${DEBUG:-}" ]]; then
        echo "# DEBUG: Created test workspace at $TEST_WS" >&2
    fi
}

# Create nested category/project structure
# Usage: setup_nested_workspace
# Creates: TEST_WS/.threads/, TEST_WS/cat1/.threads/, TEST_WS/cat1/proj1/.threads/
setup_nested_workspace() {
    setup_test_workspace
    mkdir -p "$TEST_WS/cat1/.threads"
    mkdir -p "$TEST_WS/cat1/proj1/.threads"
    mkdir -p "$TEST_WS/cat2/.threads"
}

# Clean up test workspace
teardown_test_workspace() {
    if [[ -n "$_ORIGINAL_PWD" ]]; then
        cd "$_ORIGINAL_PWD" || true
    fi
    if [[ -n "$TEST_WS" && -d "$TEST_WS" ]]; then
        rm -rf "$TEST_WS"
        if [[ -n "${DEBUG:-}" ]]; then
            echo "# DEBUG: Cleaned up test workspace at $TEST_WS" >&2
        fi
    fi
    TEST_WS=""
    unset WORKSPACE
}

# Run command in test workspace context
# Usage: run_in_workspace "$THREADS_BIN" list
run_in_workspace() {
    (
        cd "$TEST_WS" || exit 1
        "$@"
    )
}

# Capture stdout from command in workspace
# Usage: output=$(capture_stdout "$THREADS_BIN" list)
capture_stdout() {
    run_in_workspace "$@" 2>/dev/null
}

# Capture stderr from command in workspace
# Usage: errors=$(capture_stderr "$THREADS_BIN" bad-command)
capture_stderr() {
    run_in_workspace "$@" 2>&1 >/dev/null
}

# Capture both stdout and stderr
# Usage: all_output=$(capture_all "$THREADS_BIN" list)
capture_all() {
    run_in_workspace "$@" 2>&1
}

# Get exit code from command (without failing the test)
# Usage: code=$(get_exit_code "$THREADS_BIN" bad-command)
get_exit_code() {
    run_in_workspace "$@" >/dev/null 2>&1
    echo $?
}

# Register cleanup trap
# Call this at the start of each test file
register_cleanup() {
    trap teardown_test_workspace EXIT
}

# Skip test with message
skip_test() {
    local reason="${1:-skipped}"
    echo "# SKIP: $reason"
    return 0
}

# Create test workspace with git repo initialized
# Usage: setup_git_workspace
# Sets up workspace and initializes git with test user config
setup_git_workspace() {
    setup_test_workspace
    git -C "$TEST_WS" init -q
    git -C "$TEST_WS" config user.email "test@threads.test"
    git -C "$TEST_WS" config user.name "Test User"
    # Initial commit so we have a valid HEAD
    git -C "$TEST_WS" add .
    git -C "$TEST_WS" commit -q -m "Initial test workspace" --allow-empty
}

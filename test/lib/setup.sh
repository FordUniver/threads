#!/usr/bin/env bash
# Test workspace setup and teardown

# Global test workspace path
TEST_WS=""
_ORIGINAL_PWD=""

# Create isolated test workspace
# Sets TEST_WS and creates .threads/ directory
# Initializes git repo (required by implementations)
# Automatically registers EXIT trap for cleanup
setup_test_workspace() {
    _ORIGINAL_PWD="$PWD"
    TEST_WS=$(mktemp -d "${TMPDIR:-/tmp}/threads-test.XXXXXX")
    mkdir -p "$TEST_WS/.threads"
    export TEST_WS
    export WORKSPACE="$TEST_WS"
    cd "$TEST_WS" || exit 1

    # Initialize git repo (implementations require git root)
    git init -q
    git config user.email "test@threads.test"
    git config user.name "Test User"
    git add .
    git commit -q -m "Initial test workspace" --allow-empty

    # Auto-register cleanup trap (idempotent - safe to call multiple times)
    trap teardown_test_workspace EXIT

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

# Register cleanup trap (now called automatically by setup_test_workspace)
# Kept for backwards compatibility - safe to call multiple times
register_cleanup() {
    trap teardown_test_workspace EXIT
}

# Skip test with message
skip_test() {
    local reason="${1:-skipped}"
    local test_num=$((_TEST_PASSED + _TEST_FAILED + 1))
    ((_TEST_PASSED++))  # Skips count as "passed" in TAP
    echo -e "${YELLOW}ok${NC} $test_num - $_TEST_CURRENT # SKIP $reason"

    # Reset test state (same as end_test)
    _TEST_CURRENT=""
    _CURRENT_TEST_FAILED=""
    _DIAGNOSTIC_OUTPUT=""
}

# Create test workspace with git repo initialized
# Usage: setup_git_workspace
# Note: Now identical to setup_test_workspace (kept for backwards compatibility)
setup_git_workspace() {
    setup_test_workspace
}

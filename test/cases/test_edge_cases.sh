#!/usr/bin/env bash
# Edge case tests - derived from historical bugs

# Test: special characters in thread name
test_special_chars_in_name() {
    begin_test "special chars in name handled"
    setup_test_workspace

    # Create thread with special chars via CLI
    local output
    output=$($THREADS_BIN new . "Thread with 'quotes' and \`backticks\`" 2>/dev/null) || true

    local id
    id=$(extract_id_from_output "$output")

    if [[ -n "$id" ]]; then
        local name
        name=$(get_thread_field "$id" "name")
        assert_contains "$name" "quotes" "should handle special chars"
    fi

    teardown_test_workspace
    end_test
}

# Test: partial ID not found (shell requires exact 6-char ID)
test_partial_id_not_found() {
    begin_test "partial ID returns not found"
    setup_test_workspace

    create_thread "abc123" "Test Thread" "active"

    # Try to read with partial ID (shell requires exact match)
    local exit_code=0
    $THREADS_BIN read abc >/dev/null 2>&1 || exit_code=$?

    # Shell requires exact 6-char ID, partial returns not found
    assert_eq "1" "$exit_code" "partial ID should return exit code 1 (not found)"

    teardown_test_workspace
    end_test
}

# Test: not found returns exit code 1
test_not_found_error() {
    begin_test "not found returns exit code 1"
    setup_test_workspace

    # Try to read non-existent thread
    local exit_code
    $THREADS_BIN read nonexistent >/dev/null 2>&1 || exit_code=$?

    assert_eq "1" "$exit_code" "not found should return exit code 1"

    teardown_test_workspace
    end_test
}

# Test: --help flag shows usage (from fdfbb7)
test_help_flag() {
    begin_test "--help shows usage"
    setup_test_workspace

    local output
    output=$($THREADS_BIN --help 2>&1) || true

    assert_contains "$output" "threads" "help should mention threads"
    # Should show some command names
    assert_contains "$output" "list" "help should show list command"

    teardown_test_workspace
    end_test
}

# Run all tests
test_special_chars_in_name
test_partial_id_not_found
test_not_found_error
test_help_flag

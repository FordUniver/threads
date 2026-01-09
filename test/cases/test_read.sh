#!/usr/bin/env bash
# Tests for 'threads read' command

# Test: read by full ID
test_read_by_id() {
    begin_test "read by full ID"
    setup_test_workspace

    create_thread "abc123" "Test Thread" "active" "A test thread"

    local exit_code
    $THREADS_BIN read abc123 >/dev/null 2>&1
    exit_code=$?

    assert_eq "0" "$exit_code" "read should succeed with valid ID"

    teardown_test_workspace
    end_test
}

# Test: read outputs full content
test_read_outputs_content() {
    begin_test "read outputs thread content"
    setup_test_workspace

    create_thread "abc123" "Test Thread" "active" "A test thread"

    local output
    output=$($THREADS_BIN read abc123 2>/dev/null)

    assert_contains "$output" "Test Thread" "should contain thread name"
    assert_contains "$output" "## Body" "should contain Body section"
    assert_contains "$output" "## Todo" "should contain Todo section"

    teardown_test_workspace
    end_test
}

# Test: read requires exact 6-char ID
test_read_exact_id_required() {
    begin_test "read requires exact 6-char ID"
    setup_test_workspace

    create_thread "abc123" "Test Thread" "active"

    # Full ID works
    local output
    output=$($THREADS_BIN read abc123 2>/dev/null)
    assert_contains "$output" "Test Thread" "full ID should find thread"

    # Partial ID does not work (shell behavior)
    local exit_code=0
    $THREADS_BIN read abc >/dev/null 2>&1 || exit_code=$?
    assert_eq "1" "$exit_code" "partial ID should fail"

    teardown_test_workspace
    end_test
}

# Run all tests
test_read_by_id
test_read_outputs_content
test_read_exact_id_required

#!/usr/bin/env bash
# Tests for 'threads path' command

# Test: path by full ID
test_path_by_id() {
    begin_test "path by full ID"
    setup_test_workspace

    create_thread "abc123" "Test Thread" "active" "A test thread"

    local exit_code
    $THREADS_BIN path abc123 >/dev/null 2>&1
    exit_code=$?

    assert_eq "0" "$exit_code" "path should succeed with valid ID"

    teardown_test_workspace
    end_test
}

# Test: path outputs absolute file path
test_path_outputs_absolute_path() {
    begin_test "path outputs absolute file path"
    setup_test_workspace

    create_thread "abc123" "Test Thread" "active" "A test thread"

    local output
    output=$($THREADS_BIN path abc123 2>/dev/null)

    # Should contain the absolute path
    assert_contains "$output" "$TEST_WORKSPACE" "should contain workspace path"
    assert_contains "$output" ".threads" "should contain .threads directory"
    assert_contains "$output" "abc123-test-thread.md" "should contain thread filename"

    # Should not contain newlines or extra content (just the path)
    local line_count
    line_count=$(echo "$output" | wc -l | tr -d ' ')
    assert_eq "1" "$line_count" "should output exactly one line"

    teardown_test_workspace
    end_test
}

# Test: path with invalid ID fails
test_path_invalid_id() {
    begin_test "path with invalid ID fails"
    setup_test_workspace

    create_thread "abc123" "Test Thread" "active"

    # Non-existent ID should fail
    local exit_code=0
    $THREADS_BIN path nonexistent >/dev/null 2>&1 || exit_code=$?
    assert_eq "1" "$exit_code" "invalid ID should fail"

    teardown_test_workspace
    end_test
}

# Test: path with name reference
test_path_by_name() {
    begin_test "path by name reference"
    setup_test_workspace

    create_thread "abc123" "Test Thread" "active" "A test thread"

    # Try finding by name
    local exit_code
    $THREADS_BIN path "test-thread" >/dev/null 2>&1
    exit_code=$?

    assert_eq "0" "$exit_code" "path should work with name reference"

    local output
    output=$($THREADS_BIN path "test-thread" 2>/dev/null)
    assert_contains "$output" "abc123-test-thread.md" "should contain thread filename"

    teardown_test_workspace
    end_test
}

# Run all tests
test_path_by_id
test_path_outputs_absolute_path
test_path_invalid_id
test_path_by_name

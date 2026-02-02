#!/usr/bin/env bash
# Tests for 'threads body' command
# Derived from fbee2c (stdin body bug)

# Test: body --set replaces content
test_body_set_replaces() {
    begin_test "body --set replaces content"
    setup_test_workspace

    create_thread "abc123" "Test Thread" "active"

    echo "New body content" | $THREADS_BIN body abc123 --set >/dev/null 2>&1

    local body
    body=$(get_thread_section abc123 Body)
    assert_contains "$body" "New body content" "body should contain new content"

    teardown_test_workspace
    end_test
}

# Test: body --append adds to existing
test_body_append_adds() {
    begin_test "body --append adds content"
    setup_test_workspace

    create_thread "abc123" "Test Thread" "active"

    echo "First content" | $THREADS_BIN body abc123 --set >/dev/null 2>&1
    echo "Second content" | $THREADS_BIN body abc123 --append >/dev/null 2>&1

    local body
    body=$(get_thread_section abc123 Body)

    assert_contains "$body" "First content" "should keep first content"
    assert_contains "$body" "Second content" "should add second content"

    teardown_test_workspace
    end_test
}

# Test: body reads from stdin correctly (fbee2c)
test_body_stdin() {
    begin_test "body reads from stdin (fbee2c)"
    setup_test_workspace

    create_thread "abc123" "Test Thread" "active"

    # Pipe content via stdin
    echo "Content from stdin" | $THREADS_BIN body abc123 --set >/dev/null 2>&1

    local body
    body=$(get_thread_section abc123 Body)
    assert_contains "$body" "Content from stdin" "should read from stdin"

    teardown_test_workspace
    end_test
}

# Test: body preserves multiline content
test_body_multiline_stdin() {
    begin_test "body preserves multiline content"
    setup_test_workspace

    create_thread "abc123" "Test Thread" "active"

    # Multiline content
    printf "Line one\nLine two\nLine three" | $THREADS_BIN body abc123 --set >/dev/null 2>&1

    local body
    body=$(get_thread_section abc123 Body)

    assert_contains "$body" "Line one" "should contain line one"
    assert_contains "$body" "Line two" "should contain line two"
    assert_contains "$body" "Line three" "should contain line three"

    teardown_test_workspace
    end_test
}

# Test: empty pipe should fail (not silently enter read mode)
test_body_empty_pipe_fails() {
    begin_test "body with empty pipe should fail"
    setup_test_workspace

    create_thread "abc123" "Test Thread" "active"
    echo "Existing content" | $THREADS_BIN body abc123 --set >/dev/null 2>&1

    # Pipe empty content - should error, not silently succeed
    local exit_code
    printf '' | $THREADS_BIN body abc123 >/dev/null 2>&1
    exit_code=$?

    assert_eq "1" "$exit_code" "empty pipe should return exit code 1"

    teardown_test_workspace
    end_test
}

# Test: empty pipe with --set flag should also fail
test_body_empty_pipe_with_flag_fails() {
    begin_test "body --set with empty pipe should fail"
    setup_test_workspace

    create_thread "abc123" "Test Thread" "active"

    local exit_code
    printf '' | $THREADS_BIN body abc123 --set >/dev/null 2>&1
    exit_code=$?

    assert_eq "1" "$exit_code" "empty pipe with --set should fail"

    teardown_test_workspace
    end_test
}

# Run all tests
test_body_set_replaces
test_body_append_adds
test_body_stdin
test_body_multiline_stdin
test_body_empty_pipe_fails
test_body_empty_pipe_with_flag_fails

#!/usr/bin/env bash
# Tests for 'threads log' command

# Test: log adds timestamped entry
test_log_adds_entry() {
    begin_test "log adds entry"
    setup_test_workspace

    create_thread "abc123" "Test Thread" "active"

    $THREADS_BIN log abc123 "Test log entry" >/dev/null 2>&1

    local log
    log=$(get_thread_section abc123 Log)

    assert_contains "$log" "Test log entry" "should add log entry text"

    teardown_test_workspace
    end_test
}

# Test: log creates full timestamp entry
test_log_creates_timestamp_entry() {
    begin_test "log creates timestamp entry"
    setup_test_workspace

    create_thread "abc123" "Test Thread" "active"

    $THREADS_BIN log abc123 "First entry" >/dev/null 2>&1

    local path
    path=$(get_thread_path abc123)
    local content
    content=$(cat "$path")

    # Should have full timestamp in format: - **YYYY-MM-DD HH:MM:SS** text
    assert_matches "\*\*[0-9]{4}-[0-9]{2}-[0-9]{2} [0-9]{2}:[0-9]{2}:[0-9]{2}\*\*" "$content" "should have full timestamp"

    teardown_test_workspace
    end_test
}

# Test: log entry format is list item with bold timestamp
test_log_entry_format() {
    begin_test "log entry is list item with bold timestamp"
    setup_test_workspace

    create_thread "abc123" "Test Thread" "active"

    $THREADS_BIN log abc123 "Formatted entry" >/dev/null 2>&1

    local path
    path=$(get_thread_path abc123)
    local content
    content=$(cat "$path")

    # Should have format: - **YYYY-MM-DD HH:MM:SS** text
    assert_matches "- \*\*[0-9]{4}-[0-9]{2}-[0-9]{2} [0-9]{2}:[0-9]{2}:[0-9]{2}\*\* Formatted entry" "$content" "should have list item with bold timestamp"

    teardown_test_workspace
    end_test
}

# Run all tests
test_log_adds_entry
test_log_creates_timestamp_entry
test_log_entry_format

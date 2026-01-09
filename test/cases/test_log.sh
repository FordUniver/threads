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

# Test: log creates date header if needed
test_log_creates_date_header() {
    begin_test "log creates date header"
    setup_test_workspace

    create_thread "abc123" "Test Thread" "active"

    $THREADS_BIN log abc123 "First entry" >/dev/null 2>&1

    local path
    path=$(get_thread_path abc123)
    local content
    content=$(cat "$path")

    # Should have date header in format ### YYYY-MM-DD
    assert_matches "### [0-9]{4}-[0-9]{2}-[0-9]{2}" "$content" "should have date header"

    teardown_test_workspace
    end_test
}

# Test: log entry has time prefix format
test_log_entry_format() {
    begin_test "log entry has time prefix"
    setup_test_workspace

    create_thread "abc123" "Test Thread" "active"

    $THREADS_BIN log abc123 "Formatted entry" >/dev/null 2>&1

    local path
    path=$(get_thread_path abc123)
    local content
    content=$(cat "$path")

    # Should have format: - **HH:MM** text
    assert_matches "\*\*[0-9]{2}:[0-9]{2}\*\*" "$content" "should have time prefix in bold"

    teardown_test_workspace
    end_test
}

# Run all tests
test_log_adds_entry
test_log_creates_date_header
test_log_entry_format

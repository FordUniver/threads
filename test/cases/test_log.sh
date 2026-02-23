#!/usr/bin/env bash
# Tests for 'threads log' command

# Test: log adds timestamped entry
test_log_adds_entry() {
    begin_test "log adds entry"
    setup_test_workspace

    create_thread "abc123" "Test Thread" "active"

    $THREADS_BIN log abc123 "Test log entry" >/dev/null 2>&1

    local content
    content=$(cat "$(get_thread_path abc123)")

    assert_contains "$content" "Test log entry" "should add log entry text"

    teardown_test_workspace
    end_test
}

# Test: log creates full timestamp entry (stored in YAML as ts: YYYY-MM-DD HH:MM:SS)
test_log_creates_timestamp_entry() {
    begin_test "log creates timestamp entry"
    setup_test_workspace

    create_thread "abc123" "Test Thread" "active"

    $THREADS_BIN log abc123 "First entry" >/dev/null 2>&1

    local content
    content=$(cat "$(get_thread_path abc123)")

    # Timestamp stored in YAML frontmatter as: ts: YYYY-MM-DD HH:MM:SS
    assert_matches "ts: [0-9]{4}-[0-9]{2}-[0-9]{2} [0-9]{2}:[0-9]{2}:[0-9]{2}" "$content" "should have timestamp"

    teardown_test_workspace
    end_test
}

# Test: log entry format in YAML frontmatter (ts + text fields)
test_log_entry_format() {
    begin_test "log entry is list item with bracket timestamp"
    setup_test_workspace

    create_thread "abc123" "Test Thread" "active"

    $THREADS_BIN log abc123 "Formatted entry" >/dev/null 2>&1

    local content
    content=$(cat "$(get_thread_path abc123)")

    # YAML format: ts: YYYY-MM-DD HH:MM:SS followed by text: Formatted entry
    assert_matches "ts: [0-9]{4}-[0-9]{2}-[0-9]{2} [0-9]{2}:[0-9]{2}:[0-9]{2}" "$content" "should have timestamp"
    assert_contains "$content" "Formatted entry" "should have entry text"

    teardown_test_workspace
    end_test
}

# Run all tests
test_log_adds_entry
test_log_creates_timestamp_entry
test_log_entry_format

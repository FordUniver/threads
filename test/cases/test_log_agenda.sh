#!/usr/bin/env bash
# Tests for 'threads log' agenda mode (no id → cross-scope view)

# Test: no log entries → empty message
test_log_agenda_empty() {
    begin_test "log agenda: empty workspace"
    setup_test_workspace

    create_thread "abc123" "Empty Thread" "active"

    local output
    output=$(cd "$TEST_WS" && $THREADS_BIN log 2>/dev/null)

    assert_contains "$output" "No log entries found." "should report no log entries"

    teardown_test_workspace
    end_test
}

# Test: log entry in open thread appears in agenda
test_log_agenda_open_entry() {
    begin_test "log agenda: entry in open thread appears"
    setup_test_workspace

    create_thread "abc123" "My Thread" "active"
    $THREADS_BIN log abc123 "Work in progress" >/dev/null 2>&1

    local output
    output=$(cd "$TEST_WS" && $THREADS_BIN log 2>/dev/null)

    assert_contains "$output" "Work in progress" "should show log entry text"
    assert_contains "$output" "abc123" "should show thread id"

    teardown_test_workspace
    end_test
}

# Test: entry in resolved thread absent by default, present with --include-closed
test_log_agenda_closed_thread() {
    begin_test "log agenda: resolved thread skipped by default"
    setup_test_workspace

    create_thread "abc123" "Open Thread" "active"
    create_thread "def456" "Resolved Thread" "resolved"

    $THREADS_BIN log abc123 "Open entry" >/dev/null 2>&1
    $THREADS_BIN log def456 "Resolved entry" >/dev/null 2>&1

    local output_default output_closed
    output_default=$(cd "$TEST_WS" && $THREADS_BIN log 2>/dev/null)
    output_closed=$(cd "$TEST_WS" && $THREADS_BIN log --include-closed 2>/dev/null)

    assert_contains "$output_default" "Open entry" "open thread entry present"
    assert_not_contains "$output_default" "Resolved entry" "resolved thread entry absent by default"
    assert_contains "$output_closed" "Resolved entry" "resolved thread entry present with --include-closed"

    teardown_test_workspace
    end_test
}

# Test: --down collects from subdirectory threads
test_log_agenda_down() {
    begin_test "log agenda: --down collects subdirectory entries"
    setup_nested_workspace

    create_thread "abc123" "Root Thread" "active"
    create_thread_at_category "def456" "Cat Thread" "cat1" "active"

    $THREADS_BIN log abc123 "Root entry" >/dev/null 2>&1
    $THREADS_BIN log def456 "Cat entry" >/dev/null 2>&1

    local output
    output=$(cd "$TEST_WS" && $THREADS_BIN log --down 2>/dev/null)

    assert_contains "$output" "Root entry" "should show root log entry"
    assert_contains "$output" "Cat entry" "should show subdirectory entry with --down"

    teardown_test_workspace
    end_test
}

# Test: --json output has correct fields
test_log_agenda_json() {
    begin_test "log agenda: --json output has correct fields"
    setup_test_workspace

    create_thread "abc123" "JSON Thread" "active"
    $THREADS_BIN log abc123 "JSON entry" >/dev/null 2>&1

    local output
    output=$(cd "$TEST_WS" && $THREADS_BIN log --json 2>/dev/null)

    assert_json_valid "$output" "output should be valid JSON"
    assert_contains "$(echo "$output" | jq -r '.[0].text' 2>/dev/null)" "JSON entry" "text field should contain entry"
    assert_json_field "$output" ".[0].thread_id" "abc123" "thread_id should match"
    assert_json_field_not_empty "$output" ".[0].thread_name" "thread_name field should be present"
    assert_json_field_not_empty "$output" ".[0].thread_path" "thread_path field should be present"

    teardown_test_workspace
    end_test
}

# Test: multiple threads → entries from all aggregated (most recent first)
test_log_agenda_multiple_threads() {
    begin_test "log agenda: aggregates entries from multiple threads"
    setup_test_workspace

    create_thread "abc123" "Thread One" "active"
    create_thread "def456" "Thread Two" "active"

    $THREADS_BIN log abc123 "Entry from one" >/dev/null 2>&1
    $THREADS_BIN log def456 "Entry from two" >/dev/null 2>&1

    local output
    output=$(cd "$TEST_WS" && $THREADS_BIN log 2>/dev/null)

    assert_contains "$output" "Entry from one" "should show entry from thread one"
    assert_contains "$output" "Entry from two" "should show entry from thread two"

    teardown_test_workspace
    end_test
}

# Test: single-thread add still works when id provided
test_log_single_thread_add() {
    begin_test "log single-thread add unaffected"
    setup_test_workspace

    create_thread "abc123" "My Thread" "active"

    local output
    output=$($THREADS_BIN log abc123 "Direct entry" 2>/dev/null)

    assert_contains "$output" "Logged to:" "should confirm log was written"

    teardown_test_workspace
    end_test
}

# Run all tests
test_log_agenda_empty
test_log_agenda_open_entry
test_log_agenda_closed_thread
test_log_agenda_down
test_log_agenda_json
test_log_agenda_multiple_threads
test_log_single_thread_add

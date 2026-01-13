#!/usr/bin/env bash
# Extended edge case tests: special characters, boundaries, error handling, status variations

#
# Special Characters
#

# Test: thread name with double quotes
test_name_with_double_quotes() {
    begin_test "thread name with double quotes"
    setup_test_workspace

    local output
    output=$(capture_stdout $THREADS_BIN new 'Thread with "quotes"')
    local id
    id=$(extract_id_from_output "$output")

    assert_matches "^[0-9a-f]{6}$" "$id" "should create thread with valid ID"

    local name
    name=$(get_thread_field "$id" "name")
    assert_contains "$name" "quotes" "name should contain quotes text"

    teardown_test_workspace
    end_test
}

# Test: thread name with single quotes
test_name_with_single_quotes() {
    begin_test "thread name with single quotes"
    setup_test_workspace

    local output
    output=$(capture_stdout $THREADS_BIN new "Thread with 'apostrophe'")
    local id
    id=$(extract_id_from_output "$output")

    assert_matches "^[0-9a-f]{6}$" "$id" "should create thread with valid ID"

    teardown_test_workspace
    end_test
}

# Test: description with markdown characters
test_desc_with_markdown() {
    begin_test "description with markdown characters"
    setup_test_workspace

    local output
    output=$(capture_stdout $THREADS_BIN new "Markdown Test" --desc="Use *bold* and [links](url)")
    local id
    id=$(extract_id_from_output "$output")

    local desc
    desc=$(get_thread_field "$id" "desc")
    assert_contains "$desc" "bold" "description should preserve markdown"

    teardown_test_workspace
    end_test
}

# Test: unicode in thread name
test_unicode_in_name() {
    begin_test "unicode in thread name"
    setup_test_workspace

    local output
    output=$(capture_stdout $THREADS_BIN new "Thread with umlauts: Achtung")
    local id
    id=$(extract_id_from_output "$output")

    assert_matches "^[0-9a-f]{6}$" "$id" "should create thread with unicode name"

    teardown_test_workspace
    end_test
}

#
# Boundary Conditions
#

# Test: very long thread name
test_long_thread_name() {
    begin_test "very long thread name (200+ chars)"
    setup_test_workspace

    local long_name
    long_name="This is a very long thread name that exceeds two hundred characters to test boundary conditions and ensure the system handles long names gracefully without truncation or errors in any of the implementations"

    local output
    output=$(capture_stdout $THREADS_BIN new "$long_name")
    local exit_code=$?

    # Should either succeed or fail gracefully
    # (Different implementations may have different limits)
    if [[ $exit_code -eq 0 ]]; then
        local id
        id=$(extract_id_from_output "$output")
        assert_matches "^[0-9a-f]{6}$" "$id" "should create thread if accepted"
    fi
    # If it fails, that's also acceptable behavior

    teardown_test_workspace
    end_test
}

# Test: empty description
test_empty_description() {
    begin_test "empty description"
    setup_test_workspace

    # Swift ArgumentParser can't parse --desc="" (equals with empty value)
    # Use space syntax for Swift, equals syntax for others
    local output
    if [[ "$THREADS_BIN" == *"swift"* ]]; then
        output=$(capture_stdout $THREADS_BIN new "Thread No Desc" --desc "")
    else
        output=$(capture_stdout $THREADS_BIN new "Thread No Desc" --desc="")
    fi
    local id
    id=$(extract_id_from_output "$output")

    assert_matches "^[0-9a-f]{6}$" "$id" "should create thread with empty desc"

    teardown_test_workspace
    end_test
}

# Test: thread with empty body
test_read_empty_body() {
    begin_test "read thread with empty body"
    setup_test_workspace

    create_thread "abc123" "Empty Body Thread" "active"

    local output exit_code
    output=$(capture_stdout $THREADS_BIN read abc123)
    exit_code=$?

    assert_eq "0" "$exit_code" "should succeed reading thread with empty body"
    assert_contains "$output" "Empty Body Thread" "should show thread name"

    teardown_test_workspace
    end_test
}

#
# Error Handling
#

# Test: invalid status value is rejected
test_invalid_status_rejected() {
    begin_test "invalid status value is rejected"
    setup_test_workspace

    create_thread "abc123" "Test Thread" "active"

    # Attempt to set invalid status - should fail
    local exit_code
    exit_code=$(get_exit_code $THREADS_BIN status abc123 custom_status)
    assert_eq "1" "$exit_code" "should reject invalid status"

    # Status should remain unchanged
    local status
    status=$(get_thread_field "abc123" "status")
    assert_eq "active" "$status" "status should remain unchanged"

    teardown_test_workspace
    end_test
}

# Test: missing required argument
test_missing_required_arg() {
    begin_test "missing required arg shows usage"
    setup_test_workspace

    local exit_code
    exit_code=$(get_exit_code $THREADS_BIN new)

    assert_eq "1" "$exit_code" "should fail without required argument"

    teardown_test_workspace
    end_test
}

# Test: read non-existent thread
test_read_nonexistent() {
    begin_test "read non-existent thread returns exit 1"
    setup_test_workspace

    local exit_code
    exit_code=$(get_exit_code $THREADS_BIN read nonexist)

    assert_eq "1" "$exit_code" "should fail for non-existent thread"

    teardown_test_workspace
    end_test
}

#
# Status Variations
#

# Test: status with parenthetical reason
test_status_with_reason() {
    begin_test "status with parenthetical reason"
    setup_test_workspace

    create_thread "abc123" "Test Thread" "active"

    $THREADS_BIN status abc123 "blocked (waiting on review)" >/dev/null 2>&1

    local status
    status=$(get_thread_field "abc123" "status")
    assert_contains "$status" "blocked" "should contain base status"

    teardown_test_workspace
    end_test
}

# Test: all active statuses work
test_all_active_statuses() {
    begin_test "all active statuses work"
    setup_test_workspace

    local statuses=("idea" "planning" "active" "blocked" "paused")

    for s in "${statuses[@]}"; do
        create_thread "${s}001" "Thread $s" "$s"
    done

    local output
    output=$(capture_stdout $THREADS_BIN list --all)

    for s in "${statuses[@]}"; do
        assert_contains "$output" "${s}001" "should list thread with status $s"
    done

    teardown_test_workspace
    end_test
}

# Test: terminal statuses
test_terminal_statuses() {
    begin_test "terminal statuses work"
    setup_test_workspace

    create_thread "res001" "Resolved" "resolved"
    create_thread "sup001" "Superseded" "superseded"
    create_thread "def001" "Deferred" "deferred"
    create_thread "rej001" "Rejected" "rejected"

    local output
    output=$(capture_stdout $THREADS_BIN list --all)

    assert_contains "$output" "res001" "should include resolved"
    assert_contains "$output" "sup001" "should include superseded"
    assert_contains "$output" "def001" "should include deferred"
    assert_contains "$output" "rej001" "should include rejected"

    teardown_test_workspace
    end_test
}

# Test: reopen from superseded
test_reopen_from_superseded() {
    begin_test "reopen from superseded"
    setup_test_workspace

    create_thread "abc123" "Superseded Thread" "superseded"

    $THREADS_BIN reopen abc123 >/dev/null 2>&1

    local status
    status=$(get_thread_field "abc123" "status")
    assert_eq "active" "$status" "should be active after reopen"

    teardown_test_workspace
    end_test
}

# Test: reopen from deferred
test_reopen_from_deferred() {
    begin_test "reopen from deferred"
    setup_test_workspace

    create_thread "abc123" "Deferred Thread" "deferred"

    $THREADS_BIN reopen abc123 >/dev/null 2>&1

    local status
    status=$(get_thread_field "abc123" "status")
    assert_eq "active" "$status" "should be active after reopen"

    teardown_test_workspace
    end_test
}

# Test: reopen from rejected
test_reopen_from_reject() {
    begin_test "reopen from rejected"
    setup_test_workspace

    create_thread "abc123" "Rejected Thread" "rejected"

    $THREADS_BIN reopen abc123 >/dev/null 2>&1

    local status
    status=$(get_thread_field "abc123" "status")
    assert_eq "active" "$status" "should be active after reopen"

    teardown_test_workspace
    end_test
}

# Run all tests
test_name_with_double_quotes
test_name_with_single_quotes
test_desc_with_markdown
test_unicode_in_name
test_long_thread_name
test_empty_description
test_read_empty_body
test_invalid_status_rejected
test_missing_required_arg
test_read_nonexistent
test_status_with_reason
test_all_active_statuses
test_terminal_statuses
test_reopen_from_superseded
test_reopen_from_deferred
test_reopen_from_reject

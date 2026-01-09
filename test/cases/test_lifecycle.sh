#!/usr/bin/env bash
# Tests for thread lifecycle commands: status, resolve, reopen, remove

# Test: status command changes status field
test_status_change() {
    begin_test "status changes status field"
    setup_test_workspace

    create_thread "abc123" "Test Thread" "idea"

    $THREADS_BIN status abc123 active >/dev/null 2>&1

    assert_eq "active" "$(get_thread_field abc123 status)" "status should be active"

    teardown_test_workspace
    end_test
}

# Test: resolve sets status to resolved
test_resolve_sets_resolved() {
    begin_test "resolve sets status to resolved"
    setup_test_workspace

    create_thread "abc123" "Test Thread" "active"

    $THREADS_BIN resolve abc123 >/dev/null 2>&1

    assert_eq "resolved" "$(get_thread_field abc123 status)" "status should be resolved"

    teardown_test_workspace
    end_test
}

# Test: reopen changes from resolved to active
test_reopen_sets_active() {
    begin_test "reopen sets status to active"
    setup_test_workspace

    create_thread "abc123" "Test Thread" "resolved"

    $THREADS_BIN reopen abc123 >/dev/null 2>&1

    assert_eq "active" "$(get_thread_field abc123 status)" "status should be active after reopen"

    teardown_test_workspace
    end_test
}

# Test: reopen with custom status
test_reopen_custom_status() {
    begin_test "reopen with custom status"
    setup_test_workspace

    create_thread "abc123" "Test Thread" "resolved"

    $THREADS_BIN reopen abc123 --status=blocked >/dev/null 2>&1

    assert_eq "blocked" "$(get_thread_field abc123 status)" "status should be blocked"

    teardown_test_workspace
    end_test
}

# Test: remove deletes thread file
test_remove_deletes_file() {
    begin_test "remove deletes thread file"
    setup_test_workspace

    create_thread "abc123" "Test Thread" "active"

    local path
    path=$(get_thread_path abc123)
    assert_file_exists "$path" "thread file should exist before remove"

    $THREADS_BIN remove abc123 >/dev/null 2>&1

    assert_file_not_exists "$path" "thread file should not exist after remove"

    teardown_test_workspace
    end_test
}

# Run all tests
test_status_change
test_resolve_sets_resolved
test_reopen_sets_active
test_reopen_custom_status
test_remove_deletes_file

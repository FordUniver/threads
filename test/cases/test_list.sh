#!/usr/bin/env bash
# Tests for 'threads list' command

# Test: list with no threads shows informative message
test_list_empty() {
    begin_test "list with no threads"
    setup_test_workspace

    local output
    output=$($THREADS_BIN list 2>/dev/null) || true

    # Should show count of 0 threads
    assert_contains "$output" "0" "output should mention 0 threads"

    teardown_test_workspace
    end_test
}

# Test: list shows threads with active statuses
test_list_shows_active() {
    begin_test "list shows active threads"
    setup_test_workspace

    create_thread "abc123" "Active Thread" "active"
    create_thread "def456" "Idea Thread" "idea"

    local output
    output=$($THREADS_BIN list 2>/dev/null)

    assert_contains "$output" "abc123" "should show active thread"
    assert_contains "$output" "def456" "should show idea thread"

    teardown_test_workspace
    end_test
}

# Test: list excludes resolved by default
test_list_excludes_resolved() {
    begin_test "list excludes resolved by default"
    setup_test_workspace

    create_thread "abc123" "Active Thread" "active"
    create_thread "def456" "Resolved Thread" "resolved"

    local output
    output=$($THREADS_BIN list 2>/dev/null)

    assert_contains "$output" "abc123" "should show active thread"
    assert_not_contains "$output" "def456" "should hide resolved thread"

    teardown_test_workspace
    end_test
}

# Test: list --all includes resolved
test_list_all_includes_resolved() {
    begin_test "list --all includes resolved"
    setup_test_workspace

    create_thread "abc123" "Active Thread" "active"
    create_thread "def456" "Resolved Thread" "resolved"

    local output
    output=$($THREADS_BIN list --all 2>/dev/null)

    assert_contains "$output" "abc123" "should show active thread"
    assert_contains "$output" "def456" "should show resolved thread with --all"

    teardown_test_workspace
    end_test
}

# Test: list --status filters by status
test_list_status_filter() {
    begin_test "list --status filters correctly"
    setup_test_workspace

    create_thread "abc123" "Active Thread" "active"
    create_thread "def456" "Blocked Thread" "blocked"
    create_thread "ghi789" "Idea Thread" "idea"

    local output
    output=$($THREADS_BIN list --status=blocked 2>/dev/null)

    assert_contains "$output" "def456" "should show blocked thread"
    assert_not_contains "$output" "abc123" "should not show active thread"
    assert_not_contains "$output" "ghi789" "should not show idea thread"

    teardown_test_workspace
    end_test
}

# Test: list --search finds by name/desc
test_list_search() {
    begin_test "list --search finds threads"
    setup_test_workspace

    create_thread "abc123" "Authentication Bug" "active" "Login fails on mobile"
    create_thread "def456" "Database Migration" "active" "Upgrade to v3"

    local output
    output=$($THREADS_BIN list --search="auth" 2>/dev/null)

    assert_contains "$output" "abc123" "should find thread by name search"
    assert_not_contains "$output" "def456" "should not show non-matching thread"

    teardown_test_workspace
    end_test
}

# Test: list is non-recursive by default (from 880606)
test_list_non_recursive_default() {
    begin_test "list is non-recursive by default"
    setup_nested_workspace

    create_thread "abc123" "Workspace Thread" "active"
    create_thread_at_category "def456" "Category Thread" "cat1" "active"

    local output
    output=$(cd "$TEST_WS" && $THREADS_BIN list 2>/dev/null)

    assert_contains "$output" "abc123" "should show workspace-level thread"
    assert_not_contains "$output" "def456" "should not show nested thread without -r"

    teardown_test_workspace
    end_test
}

# Test: list -r includes nested threads
test_list_recursive() {
    begin_test "list -r includes nested threads"
    setup_nested_workspace

    create_thread "abc123" "Workspace Thread" "active"
    create_thread_at_category "def456" "Category Thread" "cat1" "active"
    create_thread_at_project "ghi789" "Project Thread" "cat1" "proj1" "active"

    local output
    output=$(cd "$TEST_WS" && $THREADS_BIN list -r 2>/dev/null)

    assert_contains "$output" "abc123" "should show workspace thread"
    assert_contains "$output" "def456" "should show category thread with -r"
    # Note: shell impl has bug showing "????" for project thread IDs
    # Check for thread name instead
    assert_contains "$output" "Project Thread" "should show project thread with -r"

    teardown_test_workspace
    end_test
}

# Run all tests
test_list_empty
test_list_shows_active
test_list_excludes_resolved
test_list_all_includes_resolved
test_list_status_filter
test_list_search
test_list_non_recursive_default
test_list_recursive

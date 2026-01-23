#!/usr/bin/env bash
# Tests for stats command: thread count by status

# Test: stats shows count by status
test_stats_shows_counts() {
    begin_test "stats shows count by status"
    setup_test_workspace

    create_thread "aaa001" "Active Thread" "active"
    create_thread "aaa002" "Another Active" "active"
    create_thread "bbb001" "Blocked Thread" "blocked"
    create_thread "ccc001" "Resolved Thread" "resolved"

    # Default: shows only open threads (excludes concluded like resolved)
    local output
    output=$(capture_stdout $THREADS_BIN stats)

    assert_contains "$output" "active" "should show active status"
    assert_contains "$output" "2" "should show 2 active threads"
    assert_contains "$output" "blocked" "should show blocked status"
    assert_not_contains "$output" "resolved" "should not show resolved (concluded) by default"

    # With --include-concluded: shows all statuses
    output=$(capture_stdout $THREADS_BIN stats -c)

    assert_contains "$output" "active" "should show active status"
    assert_contains "$output" "blocked" "should show blocked status"
    assert_contains "$output" "resolved" "should show resolved with -c flag"

    teardown_test_workspace
    end_test
}

# Test: stats with empty workspace
test_stats_empty_workspace() {
    begin_test "stats with empty workspace shows zeros or empty"
    setup_test_workspace

    local output
    output=$(capture_stdout $THREADS_BIN stats)
    local exit_code=$?

    # Should succeed (exit 0) even with no threads
    assert_eq "0" "$exit_code" "should succeed with empty workspace"

    teardown_test_workspace
    end_test
}

# Test: stats -r includes nested threads
test_stats_recursive() {
    begin_test "stats -r includes nested threads"
    setup_nested_workspace

    # Create threads at different levels
    create_thread "aaa001" "Root Thread" "active"
    create_thread_at_category "bbb001" "Category Thread" "cat1" "blocked"
    create_thread_at_project "ccc001" "Project Thread" "cat1" "proj1" "idea"

    local output
    output=$(capture_stdout $THREADS_BIN stats -r)

    # Should include all 3 threads in total
    assert_contains "$output" "active" "should include active"
    assert_contains "$output" "blocked" "should include blocked"
    assert_contains "$output" "idea" "should include idea"

    teardown_test_workspace
    end_test
}

# Test: stats with specific path
test_stats_specific_path() {
    begin_test "stats with specific path"
    setup_nested_workspace

    # Create threads at different levels
    create_thread "aaa001" "Root Thread" "active"
    create_thread_at_category "bbb001" "Category Thread" "cat1" "blocked"

    local output
    output=$(capture_stdout $THREADS_BIN stats cat1)

    # Should only show category thread (blocked), not root (active)
    assert_contains "$output" "blocked" "should include blocked from cat1"
    assert_not_contains "$output" "active" "should not include active from root"

    teardown_test_workspace
    end_test
}

# Run all tests
test_stats_shows_counts
test_stats_empty_workspace
test_stats_recursive
test_stats_specific_path

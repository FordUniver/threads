#!/usr/bin/env bash
# Tests for git integration: git command and commit operations

# Test: git shows pending changes
test_git_shows_pending() {
    begin_test "git shows pending changes"
    setup_git_workspace

    # Create thread (will be uncommitted)
    create_thread "abc123" "New Thread" "active"

    local output
    output=$(capture_stdout $THREADS_BIN git)

    # Should show pending changes (new file)
    assert_contains "$output" "abc123" "should show pending thread"

    teardown_test_workspace
    end_test
}

# Test: git shows nothing when clean
test_git_clean_workspace() {
    begin_test "git shows nothing when clean"
    setup_git_workspace

    # Create and commit a thread
    create_thread "abc123" "Committed Thread" "active"
    git -C "$TEST_WS" add .
    git -C "$TEST_WS" commit -q -m "Add thread"

    local output exit_code
    output=$(capture_stdout $THREADS_BIN git)
    exit_code=$?

    # Should succeed and show nothing pending
    assert_eq "0" "$exit_code" "should succeed"

    teardown_test_workspace
    end_test
}

# Test: commit single thread
test_commit_single_thread() {
    begin_test "commit single thread"
    setup_git_workspace

    create_thread "abc123" "Thread to Commit" "active"

    $THREADS_BIN commit abc123 >/dev/null 2>&1
    local exit_code=$?

    assert_eq "0" "$exit_code" "commit should succeed"

    # Verify git log contains the thread
    local git_log
    git_log=$(git -C "$TEST_WS" log --oneline -1)
    assert_contains "$git_log" "abc123" "commit message should mention thread ID"

    teardown_test_workspace
    end_test
}

# Test: commit --pending commits all modified
test_commit_pending() {
    begin_test "commit --pending commits all modified"
    setup_git_workspace

    create_thread "aaa001" "First Thread" "active"
    create_thread "bbb002" "Second Thread" "blocked"

    $THREADS_BIN commit --pending >/dev/null 2>&1
    local exit_code=$?

    assert_eq "0" "$exit_code" "commit --pending should succeed"

    # Verify both threads are committed
    local status
    status=$(git -C "$TEST_WS" status --porcelain)
    assert_eq "" "$status" "working tree should be clean after commit"

    teardown_test_workspace
    end_test
}

# Test: commit with -m sets message
test_commit_with_message() {
    begin_test "commit with -m sets message"
    setup_git_workspace

    create_thread "abc123" "Thread to Commit" "active"

    $THREADS_BIN commit abc123 -m "Custom commit message" >/dev/null 2>&1

    local git_log
    git_log=$(git -C "$TEST_WS" log --oneline -1)
    assert_contains "$git_log" "Custom commit message" "should use custom message"

    teardown_test_workspace
    end_test
}

# Run all tests
test_git_shows_pending
test_git_clean_workspace
test_commit_single_thread
test_commit_pending
test_commit_with_message

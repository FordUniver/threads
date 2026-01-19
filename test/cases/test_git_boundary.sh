#!/usr/bin/env bash
# Tests for git boundary behavior in direction traversal
# Simplified model: nested repos are always excluded, repo root is ceiling

# ====================================================================================
# Default boundary behavior (downward)
# ====================================================================================

# Test: --down stops at nested git repos by default
test_down_stops_at_nested_git() {
    begin_test "--down stops at nested git repos"
    setup_test_workspace

    # Create nested git repo with a thread
    create_nested_repo_with_threads "$TEST_WS/nested-repo"
    create_thread "abc123" "Root Thread" "active"
    create_thread "def456" "Nested Thread" "active" "" "$TEST_WS/nested-repo"

    local output
    output=$(cd "$TEST_WS" && $THREADS_BIN list --down=0 2>/dev/null)

    assert_contains "$output" "abc123" "should show root thread"
    assert_not_contains "$output" "def456" "should not show thread in nested git repo"

    teardown_test_workspace
    end_test
}

# Test: threads in nested repo are invisible
test_nested_repo_invisible() {
    begin_test "threads in nested repo invisible"
    setup_test_workspace

    create_nested_repo_with_threads "$TEST_WS/subdir/nested"
    create_thread "abc123" "Visible Thread" "active"
    create_thread "def456" "Hidden Thread" "active" "" "$TEST_WS/subdir/nested"

    # Also create a non-repo subdir thread to compare
    mkdir -p "$TEST_WS/subdir/.threads"
    create_thread "ghi789" "Subdir Thread" "active" "" "$TEST_WS/subdir"

    local output
    output=$(cd "$TEST_WS" && $THREADS_BIN list --down=0 2>/dev/null)

    assert_contains "$output" "abc123" "should show root thread"
    assert_contains "$output" "ghi789" "should show regular subdir thread"
    assert_not_contains "$output" "def456" "should not show nested repo thread"

    teardown_test_workspace
    end_test
}

# ====================================================================================
# Default boundary behavior (upward)
# ====================================================================================

# Test: --up stops at git root boundary
test_up_stops_at_git_root() {
    begin_test "--up stops at git root"
    setup_nested_workspace

    create_thread "abc123" "Root Thread" "active"
    create_thread_at_category "def456" "Category Thread" "cat1" "active"

    local output
    output=$(cd "$TEST_WS/cat1" && $THREADS_BIN list --up=0 2>/dev/null)
    local exit_code=$?

    # Should succeed and find parent thread but not go above git root
    assert_eq "0" "$exit_code" "command should succeed"
    assert_matches "abc123|def456" "$output" "should show threads"

    teardown_test_workspace
    end_test
}

# Test: --up at root doesn't crash
test_up_at_root_succeeds() {
    begin_test "--up at git root succeeds"
    setup_test_workspace

    create_thread "abc123" "Root Thread" "active"

    local output
    output=$(cd "$TEST_WS" && $THREADS_BIN list --up=0 2>/dev/null)
    local exit_code=$?

    assert_eq "0" "$exit_code" "should succeed at git root"
    assert_contains "$output" "abc123" "should show root thread"

    teardown_test_workspace
    end_test
}

# ====================================================================================
# Deeply nested structures
# ====================================================================================

# Test: deeply nested git repos
test_deeply_nested_repos() {
    begin_test "deeply nested git repos excluded"
    setup_test_workspace

    # Create nested repo inside nested repo
    create_nested_repo_with_threads "$TEST_WS/outer"
    create_nested_repo_with_threads "$TEST_WS/outer/inner"

    create_thread "abc123" "Root Thread" "active"
    create_thread "def456" "Outer Thread" "active" "" "$TEST_WS/outer"
    create_thread "ghi789" "Inner Thread" "active" "" "$TEST_WS/outer/inner"

    local output
    output=$(cd "$TEST_WS" && $THREADS_BIN list --down=0 2>/dev/null)

    assert_contains "$output" "abc123" "should show root"
    assert_not_contains "$output" "def456" "should not show outer (nested repo)"
    assert_not_contains "$output" "ghi789" "should not show inner (nested repo)"

    teardown_test_workspace
    end_test
}

# Test: nested repo without .threads directory
test_nested_repo_no_threads_dir() {
    begin_test "nested repo without .threads directory"
    setup_test_workspace

    # Create nested git repo WITHOUT .threads
    create_nested_git_repo "$TEST_WS/nested-repo"
    # Don't create .threads inside it

    create_thread "abc123" "Root Thread" "active"

    local output
    output=$(cd "$TEST_WS" && $THREADS_BIN list --down=0 2>/dev/null)

    # Should not crash, should still show root thread
    assert_contains "$output" "abc123" "should show root"

    teardown_test_workspace
    end_test
}

# ====================================================================================
# Special git structures
# ====================================================================================

# Test: git submodule-like handling
test_submodule_handling() {
    begin_test "git submodule-like handling"
    setup_test_workspace

    # Create a "submodule-like" directory (nested git repo)
    create_nested_repo_with_threads "$TEST_WS/submodule"
    create_thread "abc123" "Root Thread" "active"
    create_thread "def456" "Submodule Thread" "active" "" "$TEST_WS/submodule"

    # Submodules should be treated like nested repos (excluded)
    local output
    output=$(cd "$TEST_WS" && $THREADS_BIN list --down=0 2>/dev/null)

    assert_contains "$output" "abc123" "should show root"
    assert_not_contains "$output" "def456" "should not traverse into submodule"

    teardown_test_workspace
    end_test
}

# Test: git worktree-like handling
test_worktree_handling() {
    begin_test "git worktree-like handling"
    setup_test_workspace

    create_thread "abc123" "Root Thread" "active"

    # Create a directory with a .git file (worktree-like)
    mkdir -p "$TEST_WS/worktree-like"
    echo "gitdir: $TEST_WS/.git/worktrees/worktree-like" > "$TEST_WS/worktree-like/.git"
    mkdir -p "$TEST_WS/worktree-like/.threads"
    create_thread "def456" "Worktree Thread" "active" "" "$TEST_WS/worktree-like"

    local output
    output=$(cd "$TEST_WS" && $THREADS_BIN list --down=0 2>/dev/null)

    # Should show root, behavior with worktree-like dir is implementation-specific
    assert_contains "$output" "abc123" "should show root"

    teardown_test_workspace
    end_test
}

# ====================================================================================
# Run all tests
# ====================================================================================

# Downward boundary behavior
test_down_stops_at_nested_git
test_nested_repo_invisible

# Upward boundary behavior
test_up_stops_at_git_root
test_up_at_root_succeeds

# Nested structures
test_deeply_nested_repos
test_nested_repo_no_threads_dir

# Special git structures
test_submodule_handling
test_worktree_handling

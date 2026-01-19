#!/usr/bin/env bash
# Regression tests for code review issues (REVIEW.md)
# Tests for: commit isolation, deleted thread detection, .git file boundary, dollar sign preservation

# ====================================================================================
# Issue #1: Commit isolation - commit <id> should only commit that thread
# ====================================================================================

# Test: commit <id> isolates thread files, doesn't commit unrelated staged changes
test_commit_isolates_thread_files() {
    begin_test "commit <id> isolates thread files from other staged changes"
    setup_git_workspace

    # Create thread
    create_thread "abc123" "Test Thread" "active"

    # Stage thread
    git -C "$TEST_WS" add .threads/

    # Create and stage an unrelated file
    echo "unrelated content" > "$TEST_WS/unrelated.txt"
    git -C "$TEST_WS" add unrelated.txt

    # Commit only the thread
    $THREADS_BIN commit abc123 -m "threads: add abc123" >/dev/null 2>&1

    # Check: unrelated file should still be staged
    local status
    status=$(git -C "$TEST_WS" status --porcelain)
    assert_contains "$status" "A  unrelated.txt" "unrelated file should still be staged"

    # Check: thread should be committed
    local committed_files
    committed_files=$(git -C "$TEST_WS" show --name-only --format="" HEAD)
    assert_contains "$committed_files" "abc123" "thread should be committed"
    assert_not_contains "$committed_files" "unrelated.txt" "unrelated file should not be committed"

    teardown_test_workspace
    end_test
}

# ====================================================================================
# Issue #2: --pending should detect deleted thread files
# ====================================================================================

# Test: commit --pending detects and commits deleted threads
test_pending_detects_deleted_threads() {
    begin_test "commit --pending detects deleted thread files"
    setup_git_workspace

    # Create and commit a thread
    create_thread "abc123" "Thread to Delete" "active"
    git -C "$TEST_WS" add .
    git -C "$TEST_WS" commit -q -m "Add thread"

    # Delete the thread file (not git rm, just rm)
    rm "$TEST_WS/.threads/abc123-thread-to-delete.md"
    git -C "$TEST_WS" add -A  # Stage the deletion

    # Commit --pending should detect and commit the deletion
    $THREADS_BIN commit --pending -m "threads: remove abc123" >/dev/null 2>&1
    local exit_code=$?

    assert_eq "0" "$exit_code" "commit --pending should succeed"

    # Working tree should be clean
    local status
    status=$(git -C "$TEST_WS" status --porcelain)
    assert_eq "" "$status" "working tree should be clean after committing deletion"

    # Verify the deletion was committed
    local last_commit
    last_commit=$(git -C "$TEST_WS" log -1 --name-only --format="%s")
    assert_contains "$last_commit" "abc123" "commit should reference deleted thread"

    teardown_test_workspace
    end_test
}

# ====================================================================================
# Issue #3: .git FILE boundary detection (worktree-style)
# ====================================================================================

# Test: traversal stops at .git FILE (worktree-style), not just .git directory
test_git_file_boundary_detection() {
    begin_test ".git FILE stops traversal (worktree-style repos)"
    setup_test_workspace

    create_thread "abc123" "Root Thread" "active"

    # Create a directory with a .git FILE (worktree-style)
    mkdir -p "$TEST_WS/worktree-dir"
    echo "gitdir: /some/path/.git/worktrees/worktree" > "$TEST_WS/worktree-dir/.git"
    mkdir -p "$TEST_WS/worktree-dir/.threads"
    create_thread "def456" "Worktree Thread" "active" "" "$TEST_WS/worktree-dir"

    local output
    output=$(cd "$TEST_WS" && $THREADS_BIN list --down=0 2>/dev/null)

    assert_contains "$output" "abc123" "should show root thread"
    assert_not_contains "$output" "def456" "should NOT show worktree thread (boundary not respected)"

    teardown_test_workspace
    end_test
}

# ====================================================================================
# Issue #4: Dollar sign preservation in body/note/todo content
# ====================================================================================

# Test: body --set preserves dollar signs
test_body_preserves_dollar_signs() {
    begin_test "body --set preserves dollar signs"
    setup_git_workspace

    create_thread "abc123" "Dollar Test" "active"

    # Set body with dollar signs
    echo 'Price $100 var $1 $VAR end' | $THREADS_BIN body abc123 --set >/dev/null 2>&1

    local content
    content=$(get_thread_section "abc123" "Body")

    assert_contains "$content" '$100' "should preserve \$100"
    assert_contains "$content" '$1' "should preserve \$1"
    assert_contains "$content" '$VAR' "should preserve \$VAR"

    teardown_test_workspace
    end_test
}

# Test: note add preserves dollar signs
test_note_preserves_dollar_signs() {
    begin_test "note add preserves dollar signs"
    setup_git_workspace

    create_thread "abc123" "Dollar Test" "active"

    # Add note with dollar signs
    $THREADS_BIN note abc123 add 'Price $100 var $1 $VAR end' >/dev/null 2>&1

    local content
    content=$(get_thread_section "abc123" "Notes")

    assert_contains "$content" '$100' "should preserve \$100"
    assert_contains "$content" '$1' "should preserve \$1"
    assert_contains "$content" '$VAR' "should preserve \$VAR"

    teardown_test_workspace
    end_test
}

# Test: todo add preserves dollar signs
test_todo_preserves_dollar_signs() {
    begin_test "todo add preserves dollar signs"
    setup_git_workspace

    create_thread "abc123" "Dollar Test" "active"

    # Add todo with dollar signs
    $THREADS_BIN todo abc123 add 'Price $100 var $1 $VAR end' >/dev/null 2>&1

    local content
    content=$(get_thread_section "abc123" "Todo")

    assert_contains "$content" '$100' "should preserve \$100"
    assert_contains "$content" '$1' "should preserve \$1"
    assert_contains "$content" '$VAR' "should preserve \$VAR"

    teardown_test_workspace
    end_test
}

# Test: log entry preserves dollar signs
test_log_preserves_dollar_signs() {
    begin_test "log entry preserves dollar signs"
    setup_git_workspace

    create_thread "abc123" "Dollar Test" "active"

    # Add log entry with dollar signs
    $THREADS_BIN log abc123 'Price $100 var $1 $VAR end' >/dev/null 2>&1

    local content
    content=$(get_thread_section "abc123" "Log")

    assert_contains "$content" '$100' "should preserve \$100"
    assert_contains "$content" '$1' "should preserve \$1"
    assert_contains "$content" '$VAR' "should preserve \$VAR"

    teardown_test_workspace
    end_test
}

# ====================================================================================
# Run all tests
# ====================================================================================

# Issue #1: Commit isolation
test_commit_isolates_thread_files

# Issue #2: Deleted thread detection
test_pending_detects_deleted_threads

# Issue #3: .git file boundary
test_git_file_boundary_detection

# Issue #4: Dollar sign preservation
test_body_preserves_dollar_signs
test_note_preserves_dollar_signs
test_todo_preserves_dollar_signs
test_log_preserves_dollar_signs

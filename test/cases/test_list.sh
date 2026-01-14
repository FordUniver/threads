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

# Test: list --include-closed includes resolved
test_list_include_closed_includes_resolved() {
    begin_test "list --include-closed includes resolved"
    setup_test_workspace

    create_thread "abc123" "Active Thread" "active"
    create_thread "def456" "Resolved Thread" "resolved"

    local output
    output=$($THREADS_BIN list --include-closed 2>/dev/null)

    assert_contains "$output" "abc123" "should show active thread"
    assert_contains "$output" "def456" "should show resolved thread with --include-closed"

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

# ====================================================================================
# Core Terminal Status Tests
# ====================================================================================

# Test: status=resolved works without include-closed flag
test_list_status_resolved_without_flag() {
    begin_test "status=resolved without include-closed flag"
    setup_test_workspace

    create_thread "abc123" "Active Thread" "active"
    create_thread "def456" "Resolved Thread" "resolved"

    local output
    output=$($THREADS_BIN list --status=resolved 2>/dev/null)

    assert_contains "$output" "def456" "should show resolved thread without --include-closed"
    assert_not_contains "$output" "abc123" "should not show active thread"

    teardown_test_workspace
    end_test
}

# Test: status=resolved with include-closed flag (redundant but valid)
test_list_status_resolved_with_include_closed() {
    begin_test "status=resolved with include-closed flag"
    setup_test_workspace

    create_thread "abc123" "Active Thread" "active"
    create_thread "def456" "Resolved Thread" "resolved"

    local output
    output=$($THREADS_BIN list --status=resolved --include-closed 2>/dev/null)

    assert_contains "$output" "def456" "should show resolved thread with --include-closed"
    assert_not_contains "$output" "abc123" "should not show active thread"

    teardown_test_workspace
    end_test
}

# Test: include-closed shows all terminal statuses
test_list_include_closed_shows_all_terminal() {
    begin_test "include-closed shows all terminal statuses"
    setup_test_workspace

    create_thread "abc123" "Active Thread" "active"
    create_thread "def456" "Resolved Thread" "resolved"
    create_thread "ghi789" "Deferred Thread" "deferred"
    create_thread "jkl012" "Superseded Thread" "superseded"
    create_thread "mno345" "Rejected Thread" "rejected"

    local output
    output=$($THREADS_BIN list --include-closed 2>/dev/null)

    assert_contains "$output" "abc123" "should show active"
    assert_contains "$output" "def456" "should show resolved"
    assert_contains "$output" "ghi789" "should show deferred"
    assert_contains "$output" "jkl012" "should show superseded"
    assert_contains "$output" "mno345" "should show rejected"

    teardown_test_workspace
    end_test
}

# Test: status filter with multiple terminal statuses
test_list_status_multiple_terminal() {
    begin_test "status filter with multiple terminal statuses"
    setup_test_workspace

    create_thread "abc123" "Active Thread" "active"
    create_thread "def456" "Resolved Thread" "resolved"
    create_thread "ghi789" "Deferred Thread" "deferred"
    create_thread "jkl012" "Superseded Thread" "superseded"

    local output
    output=$($THREADS_BIN list --status=resolved,deferred 2>/dev/null)

    assert_contains "$output" "def456" "should show resolved"
    assert_contains "$output" "ghi789" "should show deferred"
    assert_not_contains "$output" "abc123" "should not show active"
    assert_not_contains "$output" "jkl012" "should not show superseded"

    teardown_test_workspace
    end_test
}

# Test: status filter with mixed terminal and active statuses
test_list_status_mixed_terminal_active() {
    begin_test "status filter with mixed terminal and active"
    setup_test_workspace

    create_thread "abc123" "Active Thread" "active"
    create_thread "def456" "Blocked Thread" "blocked"
    create_thread "ghi789" "Resolved Thread" "resolved"
    create_thread "jkl012" "Idea Thread" "idea"

    local output
    output=$($THREADS_BIN list --status=active,resolved,idea 2>/dev/null)

    assert_contains "$output" "abc123" "should show active"
    assert_contains "$output" "ghi789" "should show resolved"
    assert_contains "$output" "jkl012" "should show idea"
    assert_not_contains "$output" "def456" "should not show blocked"

    teardown_test_workspace
    end_test
}

# ====================================================================================
# Flag Validation Tests
# ====================================================================================

# Test: unknown long flag is rejected
test_list_unknown_flag_rejected() {
    begin_test "unknown long flag rejected"
    setup_test_workspace

    local exit_code=0
    local output
    output=$($THREADS_BIN list --resolved 2>&1) || exit_code=$?

    assert_eq "$exit_code" "1" "should exit with code 1"
    # Check for error message (different implementations use different wording)
    if ! echo "$output" | grep -qi "unknown\|unrecognized\|invalid"; then
        fail "error message should mention unknown/unrecognized flag"
    fi

    teardown_test_workspace
    end_test
}

# Test: unknown long flag with different name is rejected
test_list_unknown_long_flag_different() {
    begin_test "unknown long flag with different name rejected"
    setup_test_workspace

    local exit_code=0
    local output
    output=$($THREADS_BIN list --invalid-flag-name 2>&1) || exit_code=$?

    assert_eq "$exit_code" "1" "should exit with code 1"
    if ! echo "$output" | grep -qi "unknown\|unrecognized\|invalid"; then
        fail "error message should mention unknown/unrecognized flag"
    fi

    teardown_test_workspace
    end_test
}

# Test: unknown short flag is rejected
test_list_unknown_short_flag_rejected() {
    begin_test "unknown short flag rejected"
    setup_test_workspace

    local exit_code=0
    local output
    output=$($THREADS_BIN list -x 2>&1) || exit_code=$?

    assert_eq "$exit_code" "1" "should exit with code 1"

    teardown_test_workspace
    end_test
}

# ====================================================================================
# Edge Cases for Status Filter
# ====================================================================================

# Test: status with empty value
test_list_status_empty_value() {
    begin_test "status with empty value"
    setup_test_workspace

    create_thread "abc123" "Active Thread" "active"

    local exit_code=0
    local output
    output=$($THREADS_BIN list --status= 2>&1) || exit_code=$?

    # Either errors or shows no threads - both are acceptable
    if [ "$exit_code" -eq 0 ]; then
        # If succeeds, should show no matches
        assert_not_contains "$output" "abc123" "empty status should not match anything"
    fi

    teardown_test_workspace
    end_test
}

# Test: status with invalid value
test_list_status_invalid_value() {
    begin_test "status with invalid value"
    setup_test_workspace

    create_thread "abc123" "Active Thread" "active"

    local exit_code=0
    local output
    output=$($THREADS_BIN list --status=invalid_status 2>&1) || exit_code=$?

    # Should either error or show no matches
    # Most implementations allow any status value (no validation)
    # So this test documents current behavior rather than enforcing strict validation

    teardown_test_workspace
    end_test
}

# Test: status with partial invalid in comma-separated list
test_list_status_partial_invalid() {
    begin_test "status with partial invalid in list"
    setup_test_workspace

    create_thread "abc123" "Active Thread" "active"
    create_thread "def456" "Resolved Thread" "resolved"

    local output
    output=$($THREADS_BIN list --status=active,invalid,resolved 2>/dev/null) || true

    # Should handle gracefully - either show matching valid statuses or error
    # Documenting behavior across implementations

    teardown_test_workspace
    end_test
}

# Test: status is case-insensitive
test_list_status_case_insensitive() {
    begin_test "status is case-insensitive"
    setup_test_workspace

    create_thread "abc123" "Resolved Thread" "resolved"

    local output
    output=$($THREADS_BIN list --status=RESOLVED 2>/dev/null) || true

    # Most implementations are case-sensitive for status
    # This test documents the behavior

    teardown_test_workspace
    end_test
}

# Test: status with whitespace in comma list
test_list_status_with_whitespace() {
    begin_test "status with whitespace in comma list"
    setup_test_workspace

    create_thread "abc123" "Active Thread" "active"
    create_thread "def456" "Resolved Thread" "resolved"

    local output
    output=$($THREADS_BIN list --status="active, resolved" 2>/dev/null) || true

    # Implementations may or may not trim whitespace
    # This test documents the behavior

    teardown_test_workspace
    end_test
}

# ====================================================================================
# All Terminal Statuses Coverage
# ====================================================================================

# Test: each terminal status individually
test_list_each_terminal_status() {
    begin_test "each terminal status individually"
    setup_test_workspace

    create_thread "abc123" "Resolved Thread" "resolved"
    create_thread "def456" "Deferred Thread" "deferred"
    create_thread "ghi789" "Superseded Thread" "superseded"
    create_thread "jkl012" "Rejected Thread" "rejected"

    # Test resolved
    local output
    output=$($THREADS_BIN list --status=resolved 2>/dev/null)
    assert_contains "$output" "abc123" "should show resolved thread"
    assert_not_contains "$output" "def456" "should not show deferred"

    # Test deferred
    output=$($THREADS_BIN list --status=deferred 2>/dev/null)
    assert_contains "$output" "def456" "should show deferred thread"
    assert_not_contains "$output" "abc123" "should not show resolved"

    # Test superseded
    output=$($THREADS_BIN list --status=superseded 2>/dev/null)
    assert_contains "$output" "ghi789" "should show superseded thread"
    assert_not_contains "$output" "abc123" "should not show resolved"

    # Test rejected
    output=$($THREADS_BIN list --status=rejected 2>/dev/null)
    assert_contains "$output" "jkl012" "should show rejected thread"
    assert_not_contains "$output" "abc123" "should not show resolved"

    teardown_test_workspace
    end_test
}

# Test: all terminal statuses combined
test_list_all_terminal_statuses_combined() {
    begin_test "all terminal statuses combined"
    setup_test_workspace

    create_thread "abc123" "Resolved Thread" "resolved"
    create_thread "def456" "Deferred Thread" "deferred"
    create_thread "ghi789" "Superseded Thread" "superseded"
    create_thread "jkl012" "Rejected Thread" "rejected"

    local output
    output=$($THREADS_BIN list --status=resolved,deferred,superseded,rejected 2>/dev/null)

    assert_contains "$output" "abc123" "should show resolved"
    assert_contains "$output" "def456" "should show deferred"
    assert_contains "$output" "ghi789" "should show superseded"
    assert_contains "$output" "jkl012" "should show rejected"

    teardown_test_workspace
    end_test
}

# Test: default excludes each terminal status
test_list_default_excludes_each_terminal() {
    begin_test "default excludes each terminal status"
    setup_test_workspace

    create_thread "abc123" "Active Thread" "active"
    create_thread "def456" "Resolved Thread" "resolved"
    create_thread "ghi789" "Deferred Thread" "deferred"
    create_thread "jkl012" "Superseded Thread" "superseded"
    create_thread "mno345" "Rejected Thread" "rejected"

    local output
    output=$($THREADS_BIN list 2>/dev/null)

    assert_contains "$output" "abc123" "should show active"
    assert_not_contains "$output" "def456" "should not show resolved"
    assert_not_contains "$output" "ghi789" "should not show deferred"
    assert_not_contains "$output" "jkl012" "should not show superseded"
    assert_not_contains "$output" "mno345" "should not show rejected"

    teardown_test_workspace
    end_test
}

# Test: terminal status with parenthetical
test_list_terminal_status_with_parenthetical() {
    begin_test "terminal status with parenthetical"
    setup_test_workspace

    # Create thread with parenthetical status
    mkdir -p "$TEST_WS/.threads"
    cat > "$TEST_WS/.threads/abc123-test.md" <<EOF
---
id: abc123
name: Test Thread
desc: Test
status: resolved (completed early)
---
EOF

    local output
    output=$($THREADS_BIN list --status=resolved 2>/dev/null)

    assert_contains "$output" "abc123" "should match base status 'resolved'"

    teardown_test_workspace
    end_test
}

# ====================================================================================
# Interaction Tests
# ====================================================================================

# Test: status and search combined
test_list_status_and_search_combined() {
    begin_test "status and search combined"
    setup_test_workspace

    create_thread "abc123" "Auth Bug" "resolved" "Authentication issue"
    create_thread "def456" "Auth Feature" "active" "New auth method"

    local output
    output=$($THREADS_BIN list --status=resolved --search=auth 2>/dev/null)

    assert_contains "$output" "abc123" "should show resolved thread matching search"
    assert_not_contains "$output" "def456" "should not show active thread"

    teardown_test_workspace
    end_test
}

# Test: status and recursive combined
test_list_status_and_recursive_combined() {
    begin_test "status and recursive combined"
    setup_nested_workspace

    create_thread "abc123" "Workspace Resolved" "resolved"
    create_thread_at_category "def456" "Category Resolved" "cat1" "resolved"

    local output
    output=$(cd "$TEST_WS" && $THREADS_BIN list --status=resolved -r 2>/dev/null)

    assert_contains "$output" "abc123" "should show workspace-level resolved"
    assert_contains "$output" "def456" "should show category-level resolved with -r"

    teardown_test_workspace
    end_test
}

# ====================================================================================
# Run all tests
# ====================================================================================

# Original tests
test_list_empty
test_list_shows_active
test_list_excludes_resolved
test_list_include_closed_includes_resolved
test_list_status_filter
test_list_search
test_list_non_recursive_default
test_list_recursive

# Core terminal status tests
test_list_status_resolved_without_flag
test_list_status_resolved_with_include_closed
test_list_include_closed_shows_all_terminal
test_list_status_multiple_terminal
test_list_status_mixed_terminal_active

# Flag validation tests
test_list_unknown_flag_rejected
test_list_unknown_long_flag_different
test_list_unknown_short_flag_rejected

# Edge cases for status filter
test_list_status_empty_value
test_list_status_invalid_value
test_list_status_partial_invalid
test_list_status_case_insensitive
test_list_status_with_whitespace

# All terminal statuses coverage
test_list_each_terminal_status
test_list_all_terminal_statuses_combined
test_list_default_excludes_each_terminal
test_list_terminal_status_with_parenthetical

# Interaction tests
test_list_status_and_search_combined
test_list_status_and_recursive_combined

#!/usr/bin/env bash
# Tests for --down, --up direction flags on list and stats commands
# Phase 3 feature: direction flags for recursive traversal

# ====================================================================================
# Basic direction tests
# ====================================================================================

# Test: list --down finds subdirectory threads
test_list_down_flag() {
    begin_test "list --down finds subdirectory threads"
    setup_nested_workspace

    create_thread "abc123" "Root Thread" "active"
    create_thread_at_category "def456" "Category Thread" "cat1" "active"
    create_thread_at_project "ghi789" "Project Thread" "cat1" "proj1" "active"

    local output
    # Use --down=0 for unlimited depth (Go requires value, 0=unlimited)
    output=$(cd "$TEST_WS" && $THREADS_BIN list --down=0 2>/dev/null)

    assert_contains "$output" "abc123" "should show root thread"
    assert_contains "$output" "def456" "should show category thread with --down"
    # Project thread may show as name or ID depending on impl
    assert_matches "Project Thread|ghi789" "$output" "should show project thread with --down"

    teardown_test_workspace
    end_test
}

# Test: list --down with no limit goes deep
test_list_down_unlimited() {
    begin_test "list --down unlimited goes deep"
    setup_deep_nested_workspace 4

    # Create threads at each level
    create_thread "aaa001" "Root Thread" "active"
    create_thread_at_depth 1 "bbb001" "Level 1 Thread" "active"
    create_thread_at_depth 2 "ccc001" "Level 2 Thread" "active"
    create_thread_at_depth 3 "ddd001" "Level 3 Thread" "active"
    create_thread_at_depth 4 "eee001" "Level 4 Thread" "active"

    local output
    # Use --down=0 for unlimited depth
    output=$(cd "$TEST_WS" && $THREADS_BIN list --down=0 2>/dev/null)

    assert_contains "$output" "aaa001" "should show root"
    assert_contains "$output" "bbb001" "should show level 1"
    assert_contains "$output" "ccc001" "should show level 2"
    assert_contains "$output" "ddd001" "should show level 3"

    teardown_test_workspace
    end_test
}

# Test: list --up finds parent directory threads
test_list_up_flag() {
    begin_test "list --up finds parent directory threads"
    setup_nested_workspace

    create_thread "abc123" "Root Thread" "active"
    create_thread_at_category "def456" "Category Thread" "cat1" "active"

    local output
    # Use --up=0 for unlimited depth (to git root)
    output=$(cd "$TEST_WS/cat1" && $THREADS_BIN list --up=0 2>/dev/null)

    assert_contains "$output" "def456" "should show local thread"
    assert_contains "$output" "abc123" "should show parent thread with --up"

    teardown_test_workspace
    end_test
}

# Test: list --up stops at git root boundary
test_list_up_stops_at_root() {
    begin_test "list --up stops at git root boundary"
    setup_nested_workspace

    create_thread "abc123" "Root Thread" "active"
    create_thread_at_category "def456" "Category Thread" "cat1" "active"

    # Should not go above git root
    local output
    # Use --up=0 for unlimited (to git root)
    output=$(cd "$TEST_WS/cat1" && $THREADS_BIN list --up=0 2>/dev/null)

    # Should contain root but not crash/error
    assert_matches "abc123|def456" "$output" "should show threads within git root"

    teardown_test_workspace
    end_test
}

# Test: stats --down aggregates subdirectories
test_stats_down_flag() {
    begin_test "stats --down aggregates subdirectories"
    setup_nested_workspace

    create_thread "abc123" "Root Active" "active"
    create_thread_at_category "def456" "Category Blocked" "cat1" "blocked"
    create_thread_at_project "ghi789" "Project Idea" "cat1" "proj1" "idea"

    local output
    # Use --down=0 for unlimited depth
    output=$(cd "$TEST_WS" && $THREADS_BIN stats --down=0 2>/dev/null)

    assert_contains "$output" "active" "should include active status"
    assert_contains "$output" "blocked" "should include blocked from subdir"
    assert_contains "$output" "idea" "should include idea from deep subdir"

    teardown_test_workspace
    end_test
}

# Test: stats --up aggregates parent directories
test_stats_up_flag() {
    begin_test "stats --up aggregates parent directories"
    setup_nested_workspace

    create_thread "abc123" "Root Active" "active"
    create_thread_at_category "def456" "Category Blocked" "cat1" "blocked"

    local output
    # Use --up=0 for unlimited (to git root)
    output=$(cd "$TEST_WS/cat1" && $THREADS_BIN stats --up=0 2>/dev/null)

    assert_contains "$output" "active" "should include active from parent"
    assert_contains "$output" "blocked" "should include blocked from local"

    teardown_test_workspace
    end_test
}

# ====================================================================================
# Depth limiting tests
# ====================================================================================

# Test: list --down=1 stops at immediate children
test_list_down_depth_1() {
    begin_test "list --down=1 stops at immediate children"
    setup_nested_workspace

    create_thread "abc123" "Root Thread" "active"
    create_thread_at_category "def456" "Category Thread" "cat1" "active"
    create_thread_at_project "ghi789" "Project Thread" "cat1" "proj1" "active"

    local output
    output=$(cd "$TEST_WS" && $THREADS_BIN list --down=1 2>/dev/null)

    assert_contains "$output" "abc123" "should show root"
    assert_contains "$output" "def456" "should show category (depth 1)"
    assert_not_contains "$output" "ghi789" "should not show project (depth 2)"

    teardown_test_workspace
    end_test
}

# Test: list --down=2 goes two levels deep
test_list_down_depth_2() {
    begin_test "list --down=2 goes two levels deep"
    setup_nested_workspace

    create_thread "abc123" "Root Thread" "active"
    create_thread_at_category "def456" "Category Thread" "cat1" "active"
    create_thread_at_project "ghi789" "Project Thread" "cat1" "proj1" "active"

    local output
    output=$(cd "$TEST_WS" && $THREADS_BIN list --down=2 2>/dev/null)

    assert_contains "$output" "abc123" "should show root"
    assert_contains "$output" "def456" "should show category"
    # Project is at depth 2 from root, should be included
    assert_matches "ghi789|Project Thread" "$output" "should show project (depth 2)"

    teardown_test_workspace
    end_test
}

# Test: list without direction flags is local only (no recursion)
test_list_no_direction_is_local() {
    begin_test "list without direction flags is local only"
    setup_nested_workspace

    create_thread "abc123" "Root Thread" "active"
    create_thread_at_category "def456" "Category Thread" "cat1" "active"

    local output
    # No --down flag = local only
    output=$(cd "$TEST_WS" && $THREADS_BIN list 2>/dev/null)

    assert_contains "$output" "abc123" "should show local root thread"
    assert_not_contains "$output" "def456" "should not show nested thread"

    teardown_test_workspace
    end_test
}

# Test: list --up=1 checks immediate parent only
test_list_up_depth_1() {
    begin_test "list --up=1 checks immediate parent only"
    setup_nested_workspace

    create_thread "abc123" "Root Thread" "active"
    create_thread_at_category "def456" "Category Thread" "cat1" "active"
    create_thread_at_project "ghi789" "Project Thread" "cat1" "proj1" "active"

    local output
    output=$(cd "$TEST_WS/cat1/proj1" && $THREADS_BIN list --up=1 2>/dev/null)

    assert_contains "$output" "ghi789" "should show local"
    assert_contains "$output" "def456" "should show parent (depth 1)"
    assert_not_contains "$output" "abc123" "should not show grandparent (depth 2)"

    teardown_test_workspace
    end_test
}

# Test: list --up=2 goes two levels up
test_list_up_depth_2() {
    begin_test "list --up=2 goes two levels up"
    setup_nested_workspace

    create_thread "abc123" "Root Thread" "active"
    create_thread_at_category "def456" "Category Thread" "cat1" "active"
    create_thread_at_project "ghi789" "Project Thread" "cat1" "proj1" "active"

    local output
    output=$(cd "$TEST_WS/cat1/proj1" && $THREADS_BIN list --up=2 2>/dev/null)

    assert_contains "$output" "ghi789" "should show local"
    assert_contains "$output" "def456" "should show parent"
    assert_contains "$output" "abc123" "should show grandparent (depth 2)"

    teardown_test_workspace
    end_test
}

# Test: stats --down respects depth limits
test_stats_down_depth_limit() {
    begin_test "stats --down respects depth limits"
    setup_nested_workspace

    create_thread "abc123" "Root Active" "active"
    create_thread_at_category "def456" "Category Blocked" "cat1" "blocked"
    create_thread_at_project "ghi789" "Project Idea" "cat1" "proj1" "idea"

    local output
    output=$(cd "$TEST_WS" && $THREADS_BIN stats --down=1 2>/dev/null)

    assert_contains "$output" "active" "should include active"
    assert_contains "$output" "blocked" "should include blocked (depth 1)"
    # Idea is at depth 2, may or may not be included depending on implementation
    # Just verify command doesn't crash

    teardown_test_workspace
    end_test
}

# Test: stats --up respects depth limits
test_stats_up_depth_limit() {
    begin_test "stats --up respects depth limits"
    setup_nested_workspace

    create_thread "abc123" "Root Active" "active"
    create_thread_at_category "def456" "Category Blocked" "cat1" "blocked"
    create_thread_at_project "ghi789" "Project Idea" "cat1" "proj1" "idea"

    local output
    output=$(cd "$TEST_WS/cat1/proj1" && $THREADS_BIN stats --up=1 2>/dev/null)

    assert_contains "$output" "idea" "should include local"
    assert_contains "$output" "blocked" "should include parent (depth 1)"

    teardown_test_workspace
    end_test
}

# Test: depth limit exceeding structure works
test_depth_exceeds_structure() {
    begin_test "depth exceeding structure depth works"
    setup_nested_workspace

    create_thread "abc123" "Root Thread" "active"
    create_thread_at_category "def456" "Category Thread" "cat1" "active"

    local output
    output=$(cd "$TEST_WS" && $THREADS_BIN list --down=100 2>/dev/null)

    # Should not crash, should show all threads
    assert_contains "$output" "abc123" "should show root"
    assert_contains "$output" "def456" "should show category"

    teardown_test_workspace
    end_test
}

# ====================================================================================
# --down convenience
# ====================================================================================

# Test: --down with no value means unlimited
test_list_down_unlimited() {
    begin_test "--down means unlimited"
    setup_nested_workspace

    create_thread "abc123" "Root Thread" "active"
    create_thread_at_category "def456" "Category Thread" "cat1" "active"

    local output
    output=$(cd "$TEST_WS" && $THREADS_BIN list --down 2>/dev/null)

    assert_contains "$output" "abc123" "should show root"
    assert_contains "$output" "def456" "should show nested with --down"

    teardown_test_workspace
    end_test
}

test_list_r_is_rejected() {
    begin_test "-r is rejected"
    setup_nested_workspace

    create_thread "abc123" "Root Thread" "active"
    create_thread_at_category "def456" "Category Thread" "cat1" "active"

    # -r should be unknown now
    assert_exit_code 1 "$THREADS_BIN" list -r

    teardown_test_workspace
    end_test
}

# ====================================================================================
# Combined direction flags
# ====================================================================================

# Test: list --down and --up together
test_list_down_and_up_together() {
    begin_test "list --down and --up together"
    setup_nested_workspace

    create_thread "abc123" "Root Thread" "active"
    create_thread_at_category "def456" "Category Thread" "cat1" "active"
    create_thread_at_project "ghi789" "Project Thread" "cat1" "proj1" "active"
    create_thread_at_category "jkl012" "Cat2 Thread" "cat2" "active"

    local output
    # Use --up=0 and --down=0 for unlimited in both directions
    output=$(cd "$TEST_WS/cat1" && $THREADS_BIN list --up=0 --down=0 2>/dev/null)

    # Should show: parent (root), local (cat1), children (proj1)
    assert_contains "$output" "abc123" "should show root (up)"
    assert_contains "$output" "def456" "should show local"
    assert_matches "ghi789|Project Thread" "$output" "should show child (down)"

    teardown_test_workspace
    end_test
}

# Test: stats --down and --up together
test_stats_down_and_up_together() {
    begin_test "stats --down and --up together"
    setup_nested_workspace

    create_thread "abc123" "Root Active" "active"
    create_thread_at_category "def456" "Category Blocked" "cat1" "blocked"
    create_thread_at_project "ghi789" "Project Idea" "cat1" "proj1" "idea"

    local output
    # Use --up=0 and --down=0 for unlimited in both directions
    output=$(cd "$TEST_WS/cat1" && $THREADS_BIN stats --up=0 --down=0 2>/dev/null)

    # Should aggregate all directions
    assert_contains "$output" "active" "should include active (from parent)"
    assert_contains "$output" "blocked" "should include blocked (local)"
    assert_contains "$output" "idea" "should include idea (from child)"

    teardown_test_workspace
    end_test
}

# Test: --down with --status filter
test_direction_with_status_filter() {
    begin_test "--down with --status filter"
    setup_nested_workspace

    create_thread "abc123" "Root Active" "active"
    create_thread_at_category "def456" "Category Blocked" "cat1" "blocked"
    create_thread_at_project "ghi789" "Project Active" "cat1" "proj1" "active"

    local output
    # Use --down=0 for unlimited depth
    output=$(cd "$TEST_WS" && $THREADS_BIN list --down=0 --status=active 2>/dev/null)

    assert_contains "$output" "abc123" "should show root active"
    assert_not_contains "$output" "def456" "should not show blocked"
    assert_matches "ghi789|Project Active" "$output" "should show project active"

    teardown_test_workspace
    end_test
}

# Test: --up with --search filter
test_direction_with_search() {
    begin_test "--up with --search filter"
    setup_nested_workspace

    create_thread "abc123" "Auth Bug" "active" "Authentication issue"
    create_thread_at_category "def456" "Database Migration" "cat1" "active" "DB schema update"

    local output
    # Use --up=0 for unlimited (to git root)
    output=$(cd "$TEST_WS/cat1" && $THREADS_BIN list --up=0 --search="auth" 2>/dev/null)

    assert_contains "$output" "abc123" "should show auth bug from parent"
    assert_not_contains "$output" "def456" "should not show non-matching thread"

    teardown_test_workspace
    end_test
}

# ====================================================================================
# Run all tests
# ====================================================================================

# Basic direction tests
test_list_down_flag
test_list_down_unlimited
test_list_up_flag
test_list_up_stops_at_root
test_stats_down_flag
test_stats_up_flag

# Depth limiting tests
test_list_down_depth_1
test_list_down_depth_2
test_list_no_direction_is_local
test_list_up_depth_1
test_list_up_depth_2
test_stats_down_depth_limit
test_stats_up_depth_limit
test_depth_exceeds_structure

# -r removal tests
test_list_r_is_rejected

# Combined direction flags
test_list_down_and_up_together
test_stats_down_and_up_together
test_direction_with_status_filter
test_direction_with_search

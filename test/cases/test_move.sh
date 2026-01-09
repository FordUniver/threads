#!/usr/bin/env bash
# Tests for move command: thread relocation

# Test: move relocates thread file
test_move_relocates_file() {
    begin_test "move relocates thread file"
    setup_nested_workspace

    create_thread "abc123" "Thread to Move" "active"

    local old_path
    old_path=$(get_thread_path "abc123")
    assert_file_exists "$old_path" "thread should exist before move"

    $THREADS_BIN move abc123 cat1 >/dev/null 2>&1

    assert_file_not_exists "$old_path" "old path should not exist after move"

    local new_path
    new_path=$(get_thread_path "abc123" "$TEST_WS/cat1")
    assert_file_exists "$new_path" "thread should exist at new location"

    teardown_test_workspace
    end_test
}

# Test: move preserves content
test_move_preserves_content() {
    begin_test "move preserves content"
    setup_nested_workspace

    create_thread "abc123" "Thread to Move" "active" "Important description"

    $THREADS_BIN move abc123 cat1 >/dev/null 2>&1

    local status name desc
    status=$(get_thread_field "abc123" "status")
    name=$(get_thread_field "abc123" "name")
    desc=$(get_thread_field "abc123" "desc")

    assert_eq "active" "$status" "status should be preserved"
    assert_eq "Thread to Move" "$name" "name should be preserved"
    assert_eq "Important description" "$desc" "description should be preserved"

    teardown_test_workspace
    end_test
}

# Test: move to project level
test_move_to_project() {
    begin_test "move to project level"
    setup_nested_workspace

    create_thread "abc123" "Thread to Move" "active"

    $THREADS_BIN move abc123 cat1/proj1 >/dev/null 2>&1

    local new_path
    new_path=$(get_thread_path "abc123" "$TEST_WS/cat1/proj1")
    assert_file_exists "$new_path" "thread should exist at project level"

    teardown_test_workspace
    end_test
}

# Test: move fails for non-existent thread
test_move_nonexistent_thread() {
    begin_test "move fails for non-existent thread"
    setup_nested_workspace

    local exit_code
    exit_code=$(get_exit_code $THREADS_BIN move nonexist cat1)

    assert_eq "1" "$exit_code" "should fail for non-existent thread"

    teardown_test_workspace
    end_test
}

# Test: move fails for invalid destination
test_move_invalid_destination() {
    begin_test "move fails for invalid destination"
    setup_test_workspace

    create_thread "abc123" "Thread to Move" "active"

    local exit_code
    exit_code=$(get_exit_code $THREADS_BIN move abc123 /nonexistent/path)

    assert_eq "1" "$exit_code" "should fail for invalid destination"

    teardown_test_workspace
    end_test
}

# Test: move between categories
test_move_between_categories() {
    begin_test "move between categories"
    setup_nested_workspace

    create_thread_at_category "abc123" "Thread in Cat1" "cat1" "active"

    $THREADS_BIN move abc123 cat2 >/dev/null 2>&1

    local old_path new_path
    old_path=$(get_thread_path "abc123" "$TEST_WS/cat1")
    new_path=$(get_thread_path "abc123" "$TEST_WS/cat2")

    assert_file_not_exists "$old_path" "should not exist in cat1"
    assert_file_exists "$new_path" "should exist in cat2"

    teardown_test_workspace
    end_test
}

# Run all tests
test_move_relocates_file
test_move_preserves_content
test_move_to_project
test_move_nonexistent_thread
test_move_invalid_destination
test_move_between_categories

#!/usr/bin/env bash
# Tests for 'threads new' command

# Test: new creates a file in .threads/
test_new_creates_file() {
    begin_test "new creates file in .threads/"
    setup_test_workspace

    $THREADS_BIN new . "Test Thread" >/dev/null 2>&1

    local count
    count=$(count_threads)
    assert_eq "1" "$count" "should create exactly one thread file"

    teardown_test_workspace
    end_test
}

# Test: new generates 6-char hex ID
test_new_generates_id() {
    begin_test "new generates 6-char hex ID"
    setup_test_workspace

    local output
    output=$($THREADS_BIN new . "Test Thread" 2>/dev/null)

    # ID should be in output
    local id
    id=$(extract_id_from_output "$output")
    assert_matches "^[0-9a-f]{6}$" "$id" "ID should be 6 hex chars"

    # File should exist with this ID prefix
    local thread_path
    thread_path=$(get_thread_path "$id")
    assert_file_exists "$thread_path" "file should exist with ID prefix"

    teardown_test_workspace
    end_test
}

# Test: new sets frontmatter fields
test_new_sets_frontmatter() {
    begin_test "new sets frontmatter fields"
    setup_test_workspace

    local output
    output=$($THREADS_BIN new . "My Thread Name" 2>/dev/null)
    local id
    id=$(extract_id_from_output "$output")

    assert_eq "My Thread Name" "$(get_thread_field "$id" "name")" "name field"
    assert_matches "^(idea|active)$" "$(get_thread_field "$id" "status")" "status field"

    teardown_test_workspace
    end_test
}

# Test: new with --desc flag
test_new_with_desc() {
    begin_test "new with --desc flag"
    setup_test_workspace

    local output
    output=$($THREADS_BIN new . "Test Thread" --desc="A test description" 2>/dev/null)
    local id
    id=$(extract_id_from_output "$output")

    assert_eq "A test description" "$(get_thread_field "$id" "desc")" "desc field"

    teardown_test_workspace
    end_test
}

# Test: default status is idea
test_new_default_status_idea() {
    begin_test "new default status is idea"
    setup_test_workspace

    local output
    output=$($THREADS_BIN new . "Test Thread" 2>/dev/null)
    local id
    id=$(extract_id_from_output "$output")

    assert_eq "idea" "$(get_thread_field "$id" "status")" "default status should be idea"

    teardown_test_workspace
    end_test
}

# Test: new with --status flag
test_new_with_status() {
    begin_test "new with --status flag"
    setup_test_workspace

    local output
    output=$($THREADS_BIN new . "Test Thread" --status=active 2>/dev/null)
    local id
    id=$(extract_id_from_output "$output")

    assert_eq "active" "$(get_thread_field "$id" "status")" "status should be active"

    teardown_test_workspace
    end_test
}

# Test: new outputs the created ID
test_new_outputs_id() {
    begin_test "new outputs created ID"
    setup_test_workspace

    local output
    output=$($THREADS_BIN new . "Test Thread" 2>/dev/null)

    # Output should contain a 6-char hex ID
    assert_matches "[0-9a-f]{6}" "$output" "output should contain thread ID"

    teardown_test_workspace
    end_test
}

# Run all tests
test_new_creates_file
test_new_generates_id
test_new_sets_frontmatter
test_new_with_desc
test_new_default_status_idea
test_new_with_status
test_new_outputs_id

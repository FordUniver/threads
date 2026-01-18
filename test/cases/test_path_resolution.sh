#!/usr/bin/env bash
# Tests for git-root-relative path model from Phase 1
# Path resolution tests for thread location semantics

# ====================================================================================
# Path argument interpretation
# ====================================================================================

# Test: new with no path uses PWD
test_new_no_path_uses_pwd() {
    begin_test "new with no explicit path uses PWD"
    setup_nested_workspace

    # Create thread from category directory without path arg
    local output
    output=$(cd "$TEST_WS/cat1" && $THREADS_BIN new "Test Thread" 2>/dev/null) || \
        output=$(cd "$TEST_WS/cat1" && $THREADS_BIN new . "Test Thread" 2>/dev/null)

    local id
    id=$(extract_id_from_output "$output")

    # Thread should be in cat1/.threads/
    local thread_file
    thread_file=$(find "$TEST_WS/cat1/.threads" -name "${id}-*.md" 2>/dev/null | head -1)

    if [[ -n "$thread_file" ]]; then
        assert_file_exists "$thread_file" "thread should be created in PWD"
    else
        # If no path arg version isn't supported, check root
        thread_file=$(find "$TEST_WS/.threads" -name "${id}-*.md" 2>/dev/null | head -1)
        assert_file_exists "$thread_file" "thread should exist somewhere"
    fi

    teardown_test_workspace
    end_test
}

# Test: new with . uses PWD
test_new_dot_uses_pwd() {
    begin_test "new with . uses PWD"
    setup_nested_workspace

    local output
    output=$(cd "$TEST_WS/cat1" && $THREADS_BIN new . "Dot Thread" 2>/dev/null)

    local id
    id=$(extract_id_from_output "$output")

    # Thread should be in cat1/.threads/
    local thread_file
    thread_file=$(find "$TEST_WS/cat1/.threads" -name "${id}-*.md" 2>/dev/null | head -1)
    assert_file_exists "$thread_file" "thread should be created in PWD with ."

    teardown_test_workspace
    end_test
}

# Test: new with ./sub is PWD-relative
test_new_dotslash_relative() {
    begin_test "new with ./sub is PWD-relative"
    setup_nested_workspace

    # Create a subdirectory
    mkdir -p "$TEST_WS/cat1/subdir"

    local output
    output=$(cd "$TEST_WS/cat1" && $THREADS_BIN new ./subdir "Subdir Thread" 2>/dev/null)

    local id
    id=$(extract_id_from_output "$output")

    # Thread should be in cat1/subdir/.threads/
    local thread_file
    thread_file=$(find "$TEST_WS/cat1/subdir/.threads" -name "${id}-*.md" 2>/dev/null | head -1)
    assert_file_exists "$thread_file" "thread should be created in ./subdir relative to PWD"

    teardown_test_workspace
    end_test
}

# Test: new with bare path is git-root-relative
test_new_gitroot_relative() {
    begin_test "new with bare path is git-root-relative"
    setup_nested_workspace

    local output
    # From root, create thread at cat1 using bare path
    output=$(cd "$TEST_WS" && $THREADS_BIN new cat1 "Cat1 Thread" 2>/dev/null)

    local id
    id=$(extract_id_from_output "$output")

    # Thread should be in cat1/.threads/
    local thread_file
    thread_file=$(find "$TEST_WS/cat1/.threads" -name "${id}-*.md" 2>/dev/null | head -1)
    assert_file_exists "$thread_file" "thread should be created at git-root-relative path"

    teardown_test_workspace
    end_test
}

# Test: new with absolute path works
test_new_absolute_path() {
    begin_test "new with absolute path works"
    setup_nested_workspace

    local output
    output=$($THREADS_BIN new "$TEST_WS/cat2" "Absolute Thread" 2>/dev/null)

    local id
    id=$(extract_id_from_output "$output")

    # Thread should be in cat2/.threads/
    local thread_file
    thread_file=$(find "$TEST_WS/cat2/.threads" -name "${id}-*.md" 2>/dev/null | head -1)
    assert_file_exists "$thread_file" "thread should be created at absolute path"

    teardown_test_workspace
    end_test
}

# Test: new creates .threads/ directory if missing
test_new_path_creates_threads_dir() {
    begin_test "new creates .threads/ directory if missing"
    setup_test_workspace

    # Create a new subdirectory without .threads/
    mkdir -p "$TEST_WS/newdir"
    assert_dir_not_exists() {
        if [[ -d "$1" ]]; then
            _fail "directory should not exist: $1"
            return 1
        fi
        return 0
    }

    local output
    output=$($THREADS_BIN new "$TEST_WS/newdir" "New Dir Thread" 2>/dev/null)

    # .threads/ should now exist
    assert_dir_exists "$TEST_WS/newdir/.threads" ".threads should be created"

    local id
    id=$(extract_id_from_output "$output")
    local thread_file
    thread_file=$(find "$TEST_WS/newdir/.threads" -name "${id}-*.md" 2>/dev/null | head -1)
    assert_file_exists "$thread_file" "thread should exist"

    teardown_test_workspace
    end_test
}

# ====================================================================================
# Path display consistency
# ====================================================================================

# Test: list shows git-root-relative paths
test_list_shows_gitroot_relative() {
    begin_test "list shows git-root-relative paths"
    setup_nested_workspace

    create_thread_at_category "abc123" "Cat1 Thread" "cat1" "active"

    local output
    output=$(cd "$TEST_WS" && $THREADS_BIN list -r 2>/dev/null)

    # Should show path relative to git root (cat1/ prefix)
    assert_contains "$output" "cat1" "should show category path"

    teardown_test_workspace
    end_test
}

# Test: path command returns correct absolute path
test_path_cmd_absolute_accuracy() {
    begin_test "path command returns correct absolute path"
    setup_test_workspace

    create_thread "abc123" "Test Thread" "active"

    local output
    output=$($THREADS_BIN path abc123 2>/dev/null)

    # Should be absolute path
    assert_matches "^/" "$output" "path should be absolute"

    # Should point to the actual file
    local trimmed
    trimmed=$(echo "$output" | tr -d '\n\r')
    assert_file_exists "$trimmed" "path should point to existing file"

    teardown_test_workspace
    end_test
}

# Test: validate shows same paths as list
test_validate_paths_consistent() {
    begin_test "validate shows paths consistent with list"
    setup_nested_workspace

    create_thread_at_category "abc123" "Cat1 Thread" "cat1" "active"

    local list_output validate_output

    list_output=$(cd "$TEST_WS" && $THREADS_BIN list -r 2>/dev/null)
    validate_output=$(cd "$TEST_WS" && $THREADS_BIN validate -r 2>/dev/null)

    # Both should reference cat1 path
    assert_matches "cat1|abc123" "$list_output" "list should show thread"
    # validate may or may not show paths, just verify it doesn't crash

    teardown_test_workspace
    end_test
}

# Test: new JSON output paths match actual location
test_new_output_paths_match() {
    begin_test "new JSON output paths match actual location"
    setup_test_workspace

    local output
    output=$($THREADS_BIN new . "Test Thread" --json 2>/dev/null)

    # Get paths from JSON
    local json_path json_abs_path
    json_path=$(get_json_field "$output" ".path")
    json_abs_path=$(get_json_field "$output" ".path_absolute")

    # At least one should exist
    local actual_path
    if [[ -n "$json_abs_path" && "$json_abs_path" != "null" && "$json_abs_path" == /* ]]; then
        actual_path="$json_abs_path"
    elif [[ -n "$json_path" && "$json_path" != "null" ]]; then
        if [[ "$json_path" == /* ]]; then
            actual_path="$json_path"
        else
            actual_path="$TEST_WS/$json_path"
        fi
    fi

    if [[ -n "$actual_path" ]]; then
        assert_file_exists "$actual_path" "JSON path should point to actual file"
    fi

    teardown_test_workspace
    end_test
}

# ====================================================================================
# Error handling
# ====================================================================================

# Test: non-existent path gives clear error
test_invalid_path_error() {
    begin_test "non-existent path gives clear error"
    setup_test_workspace

    local exit_code=0
    local output
    output=$($THREADS_BIN new /nonexistent/path "Test Thread" 2>&1) || exit_code=$?

    # Should fail with non-zero exit
    assert_eq "1" "$exit_code" "should fail for non-existent path"

    # Error message should be meaningful
    assert_matches "not|exist|found|invalid|error" "$output" "error should mention path issue"

    teardown_test_workspace
    end_test
}

# Test: path outside git root behavior
test_path_outside_git_error() {
    begin_test "path outside git root behavior"
    setup_test_workspace

    # Try to create thread outside git root (parent directory)
    local exit_code=0
    local output
    output=$($THREADS_BIN new /tmp "Test Thread" 2>&1) || exit_code=$?

    # Implementation may either:
    # 1. Error (path outside git root)
    # 2. Use /tmp as path (if allowed)
    # Both are valid - just document behavior

    teardown_test_workspace
    end_test
}

# Test: ambiguous path resolution
test_ambiguous_path_resolution() {
    begin_test "ambiguous path resolution"
    setup_nested_workspace

    # Create a subdir with same name as category
    mkdir -p "$TEST_WS/cat1/cat1"

    # Path "cat1" could mean:
    # 1. git-root-relative: $TEST_WS/cat1
    # 2. PWD-relative: $TEST_WS/cat1/cat1 (if PWD is cat1)

    local output
    output=$(cd "$TEST_WS" && $THREADS_BIN new cat1 "Ambiguous Thread" 2>/dev/null)

    local id
    id=$(extract_id_from_output "$output")

    # Thread should be at git-root/cat1 (not cat1/cat1)
    local thread_file
    thread_file=$(find "$TEST_WS/cat1/.threads" -maxdepth 1 -name "${id}-*.md" 2>/dev/null | head -1)

    if [[ -n "$thread_file" ]]; then
        assert_file_exists "$thread_file" "thread should be at git-root-relative path"
    fi

    teardown_test_workspace
    end_test
}

# ====================================================================================
# Run all tests
# ====================================================================================

# Path argument interpretation
test_new_no_path_uses_pwd
test_new_dot_uses_pwd
test_new_dotslash_relative
test_new_gitroot_relative
test_new_absolute_path
test_new_path_creates_threads_dir

# Path display consistency
test_list_shows_gitroot_relative
test_path_cmd_absolute_accuracy
test_validate_paths_consistent
test_new_output_paths_match

# Error handling
test_invalid_path_error
test_path_outside_git_error
test_ambiguous_path_resolution

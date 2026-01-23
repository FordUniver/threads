#!/usr/bin/env bash
# Tests for validate command: thread file format validation

# Test: validate passes for valid thread
test_validate_valid_thread() {
    begin_test "validate passes for valid thread"
    setup_test_workspace

    create_thread "abc123" "Valid Thread" "active" "A description"

    local exit_code
    exit_code=$(get_exit_code $THREADS_BIN validate)

    assert_eq "0" "$exit_code" "should pass for valid thread"

    teardown_test_workspace
    end_test
}

# Test: validate fails for missing frontmatter
test_validate_no_frontmatter() {
    begin_test "validate fails for missing frontmatter"
    setup_test_workspace

    create_malformed_thread "bad001" "no_frontmatter"

    local exit_code
    exit_code=$(get_exit_code $THREADS_BIN validate)

    assert_eq "1" "$exit_code" "should fail for missing frontmatter"

    teardown_test_workspace
    end_test
}

# Test: validate fails for invalid YAML
test_validate_invalid_yaml() {
    begin_test "validate fails for invalid YAML"
    setup_test_workspace

    create_malformed_thread "bad002" "invalid_yaml"

    local exit_code
    exit_code=$(get_exit_code $THREADS_BIN validate)

    assert_eq "1" "$exit_code" "should fail for invalid YAML"

    teardown_test_workspace
    end_test
}

# Test: validate fails for missing id (id is always required)
test_validate_missing_id() {
    begin_test "validate fails for missing id"
    setup_test_workspace

    create_malformed_thread "bad003" "missing_id"

    local exit_code
    exit_code=$(get_exit_code $THREADS_BIN validate)

    # ID is always required in frontmatter
    assert_eq "1" "$exit_code" "should fail for missing id"

    teardown_test_workspace
    end_test
}

# Test: validate fails for missing name
test_validate_missing_name() {
    begin_test "validate fails for missing name field"
    setup_test_workspace

    create_malformed_thread "bad004" "missing_name"

    local exit_code
    exit_code=$(get_exit_code $THREADS_BIN validate)

    assert_eq "1" "$exit_code" "should fail for missing name"

    teardown_test_workspace
    end_test
}

# Test: validate --down validates recursively
test_validate_recursive() {
    begin_test "validate --down validates recursively"
    setup_nested_workspace

    # Create valid thread at root, invalid at category
    create_thread "aaa001" "Valid Thread" "active"
    create_malformed_thread "bad001" "no_frontmatter" "$TEST_WS/cat1"

    local exit_code
    exit_code=$(get_exit_code $THREADS_BIN validate --down)

    assert_eq "1" "$exit_code" "should fail when nested thread is invalid"

    teardown_test_workspace
    end_test
}

# Test: validate --down --json reports correct error count across nested dirs
# This catches implementations that skip files or don't actually validate
test_validate_error_count_accuracy() {
    begin_test "validate --down --json reports accurate error count"
    setup_nested_workspace

    # Create 3 valid threads at different levels (using valid 6-hex-char IDs)
    create_thread "aaa001" "Valid Root" "active"
    create_thread "bbb002" "Valid Cat" "active" "" "$TEST_WS/cat1"
    create_thread "ccc003" "Valid Proj" "active" "" "$TEST_WS/cat1/proj1"

    # Create 2 invalid threads (missing name) at different levels
    create_malformed_thread "ddd001" "missing_name" "$TEST_WS"
    create_malformed_thread "eee002" "missing_name" "$TEST_WS/cat1/proj1"

    local output
    output=$($THREADS_BIN validate --down --json 2>/dev/null) || true

    # Extract error count from JSON (should be exactly 2)
    # JSON format: {"total": N, "errors": M, "results": [...]}
    local errors
    errors=$(echo "$output" | grep -o '"errors"[[:space:]]*:[[:space:]]*[0-9]*' | grep -o '[0-9]*')

    local total
    total=$(echo "$output" | grep -o '"total"[[:space:]]*:[[:space:]]*[0-9]*' | grep -o '[0-9]*')

    assert_eq "5" "$total" "should validate all 5 threads"
    assert_eq "2" "$errors" "should report exactly 2 errors"

    teardown_test_workspace
    end_test
}

# Run all tests
test_validate_valid_thread
test_validate_no_frontmatter
test_validate_invalid_yaml
test_validate_missing_id
test_validate_missing_name
test_validate_recursive
test_validate_error_count_accuracy

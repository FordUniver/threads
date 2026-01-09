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

# Test: validate passes for thread without id (derived from filename)
test_validate_missing_id_ok() {
    begin_test "validate passes when id derived from filename"
    setup_test_workspace

    create_malformed_thread "bad003" "missing_id"

    local exit_code
    exit_code=$(get_exit_code $THREADS_BIN validate)

    # ID can be derived from filename, so this should pass
    assert_eq "0" "$exit_code" "should pass when id can be derived from filename"

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

# Test: validate -r validates recursively
test_validate_recursive() {
    begin_test "validate -r validates recursively"
    setup_nested_workspace

    # Create valid thread at root, invalid at category
    create_thread "aaa001" "Valid Thread" "active"
    create_malformed_thread "bad001" "no_frontmatter" "$TEST_WS/cat1"

    local exit_code
    exit_code=$(get_exit_code $THREADS_BIN validate -r)

    assert_eq "1" "$exit_code" "should fail when nested thread is invalid"

    teardown_test_workspace
    end_test
}

# Run all tests
test_validate_valid_thread
test_validate_no_frontmatter
test_validate_invalid_yaml
test_validate_missing_id_ok
test_validate_missing_name
test_validate_recursive

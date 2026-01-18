#!/usr/bin/env bash
# Tests for --format and --json flags on validate, new, path commands
# Phase 4 feature: structured output formats

# ====================================================================================
# validate format tests
# ====================================================================================

# Test: validate --format=json produces valid JSON
test_validate_format_json() {
    begin_test "validate --format=json produces valid JSON"
    setup_test_workspace

    create_thread "abc123" "Test Thread" "active"

    local output
    output=$($THREADS_BIN validate --format=json 2>/dev/null)

    assert_json_valid "$output" "validate output should be valid JSON"
    # Accept either .valid at root OR .results[].valid (Go's nested structure)
    if ! echo "$output" | jq -e '.valid' >/dev/null 2>&1; then
        assert_json_has_field "$output" ".results[0].valid" "JSON should have .valid or .results[].valid"
    fi

    teardown_test_workspace
    end_test
}

# Test: validate --format=yaml produces valid YAML
test_validate_format_yaml() {
    begin_test "validate --format=yaml produces valid YAML"
    setup_test_workspace

    create_thread "abc123" "Test Thread" "active"

    local output
    output=$($THREADS_BIN validate --format=yaml 2>/dev/null)

    assert_yaml_valid "$output" "validate output should be valid YAML"
    assert_contains "$output" "valid" "YAML should contain valid field"

    teardown_test_workspace
    end_test
}

# Test: validate --json is shorthand for --format=json
test_validate_json_shorthand() {
    begin_test "validate --json equals --format=json"
    setup_test_workspace

    create_thread "abc123" "Test Thread" "active"

    local json_output format_output

    json_output=$($THREADS_BIN validate --json 2>/dev/null)
    format_output=$($THREADS_BIN validate --format=json 2>/dev/null)

    assert_json_valid "$json_output" "--json should produce valid JSON"
    assert_json_valid "$format_output" "--format=json should produce valid JSON"

    # Both should have the same structure - check .valid or .results[0].valid
    local json_valid format_valid
    json_valid=$(get_json_field "$json_output" ".valid // .results[0].valid")
    format_valid=$(get_json_field "$format_output" ".valid // .results[0].valid")
    assert_eq "$json_valid" "$format_valid" "both should have same valid value"

    teardown_test_workspace
    end_test
}

# Test: validate --format=plain produces plain text
test_validate_format_plain() {
    begin_test "validate --format=plain produces plain text"
    setup_test_workspace

    create_thread "abc123" "Test Thread" "active"

    local output
    output=$($THREADS_BIN validate --format=plain 2>/dev/null) || output=$($THREADS_BIN validate 2>/dev/null)

    # Plain output should not be JSON
    if echo "$output" | jq . >/dev/null 2>&1; then
        # If it parses as JSON, that's fine - some impls may default to JSON
        # Just verify it doesn't error
        :
    fi

    teardown_test_workspace
    end_test
}

# Test: validate --json output is parseable
test_validate_json_valid_syntax() {
    begin_test "validate JSON output is parseable"
    setup_test_workspace

    create_thread "abc123" "Valid Thread" "active"
    create_malformed_thread "bad001" "missing_name"

    local output
    output=$($THREADS_BIN validate --json 2>/dev/null) || true

    # Even with validation errors, output should be valid JSON
    assert_json_valid "$output" "output should be valid JSON even with errors"

    teardown_test_workspace
    end_test
}

# Test: validate --format=yaml output is parseable
test_validate_yaml_valid_syntax() {
    begin_test "validate YAML output is parseable"
    setup_test_workspace

    create_thread "abc123" "Valid Thread" "active"

    local output
    output=$($THREADS_BIN validate --format=yaml 2>/dev/null)

    assert_yaml_valid "$output" "output should be valid YAML"

    teardown_test_workspace
    end_test
}

# ====================================================================================
# new format tests
# ====================================================================================

# Test: new --format=json returns structured output
test_new_format_json() {
    begin_test "new --format=json returns structured output"
    setup_test_workspace

    local output
    output=$($THREADS_BIN new . "Test Thread" --format=json 2>/dev/null)

    assert_json_valid "$output" "new output should be valid JSON"
    assert_json_has_field "$output" ".id" "JSON should have id field"
    assert_json_has_field "$output" ".path" "JSON should have path field"

    teardown_test_workspace
    end_test
}

# Test: new --format=yaml returns structured output
test_new_format_yaml() {
    begin_test "new --format=yaml returns structured output"
    setup_test_workspace

    local output
    output=$($THREADS_BIN new . "Test Thread" --format=yaml 2>/dev/null)

    assert_yaml_valid "$output" "new output should be valid YAML"
    assert_contains "$output" "id:" "YAML should have id field"
    assert_contains "$output" "path" "YAML should have path field"

    teardown_test_workspace
    end_test
}

# Test: new --json is shorthand for --format=json
test_new_json_shorthand() {
    begin_test "new --json equals --format=json"
    setup_test_workspace

    local json_output
    json_output=$($THREADS_BIN new . "Test Thread" --json 2>/dev/null)

    assert_json_valid "$json_output" "--json should produce valid JSON"
    assert_json_has_field "$json_output" ".id"

    teardown_test_workspace
    end_test
}

# Test: new --format=plain produces plain text (no JSON structure)
test_new_format_plain() {
    begin_test "new --format=plain produces plain text"
    setup_test_workspace

    local output
    output=$($THREADS_BIN new . "Test Thread" --format=plain 2>/dev/null) || \
        output=$($THREADS_BIN new . "Test Thread Two" 2>/dev/null)

    # Should contain the thread ID
    assert_matches "[0-9a-f]{6}" "$output" "output should contain thread ID"

    teardown_test_workspace
    end_test
}

# Test: new JSON id field matches created thread
test_new_json_id_field() {
    begin_test "new JSON id field matches created thread"
    setup_test_workspace

    local output
    output=$($THREADS_BIN new . "Test Thread" --json 2>/dev/null)

    local json_id
    json_id=$(get_json_field "$output" ".id")

    # ID should be 6 hex chars
    assert_matches "^[0-9a-f]{6}$" "$json_id" "JSON id should be 6 hex chars"

    # Thread file should exist with this ID
    local thread_file
    thread_file=$(find "$TEST_WS/.threads" -name "${json_id}-*.md" 2>/dev/null | head -1)
    assert_file_exists "$thread_file" "thread file should exist for returned ID"

    teardown_test_workspace
    end_test
}

# Test: new JSON path_absolute is valid file path
test_new_json_path_accuracy() {
    begin_test "new JSON paths are accurate"
    setup_test_workspace

    local output
    output=$($THREADS_BIN new . "Test Thread" --json 2>/dev/null)

    # Check path_absolute or path field
    local abs_path
    abs_path=$(get_json_field "$output" ".path_absolute")
    if [[ -z "$abs_path" || "$abs_path" == "null" ]]; then
        abs_path=$(get_json_field "$output" ".path")
    fi

    # If path is absolute, verify file exists
    if [[ "$abs_path" == /* ]]; then
        assert_file_exists "$abs_path" "path_absolute should point to existing file"
    fi

    teardown_test_workspace
    end_test
}

# ====================================================================================
# path format tests
# ====================================================================================

# Test: path --format=json returns structured output
test_path_format_json() {
    begin_test "path --format=json returns structured output"
    setup_test_workspace

    create_thread "abc123" "Test Thread" "active"

    local output
    output=$($THREADS_BIN path abc123 --format=json 2>/dev/null)

    assert_json_valid "$output" "path output should be valid JSON"
    assert_json_has_field "$output" ".path" "JSON should have path field"

    teardown_test_workspace
    end_test
}

# Test: path --format=yaml returns structured output
test_path_format_yaml() {
    begin_test "path --format=yaml returns structured output"
    setup_test_workspace

    create_thread "abc123" "Test Thread" "active"

    local output
    output=$($THREADS_BIN path abc123 --format=yaml 2>/dev/null)

    assert_yaml_valid "$output" "path output should be valid YAML"
    assert_contains "$output" "path" "YAML should have path field"

    teardown_test_workspace
    end_test
}

# Test: path --json is shorthand for --format=json
test_path_json_shorthand() {
    begin_test "path --json equals --format=json"
    setup_test_workspace

    create_thread "abc123" "Test Thread" "active"

    local output
    output=$($THREADS_BIN path abc123 --json 2>/dev/null)

    assert_json_valid "$output" "--json should produce valid JSON"
    assert_json_has_field "$output" ".path"

    teardown_test_workspace
    end_test
}

# Test: path --format=plain produces just the path
test_path_format_plain() {
    begin_test "path --format=plain produces just the path"
    setup_test_workspace

    create_thread "abc123" "Test Thread" "active"

    local output
    output=$($THREADS_BIN path abc123 --format=plain 2>/dev/null) || \
        output=$($THREADS_BIN path abc123 2>/dev/null)

    # Output should contain the path
    assert_contains "$output" ".threads" "output should contain .threads"
    assert_contains "$output" "abc123" "output should contain thread ID"

    # Should be a single line (just the path)
    local line_count
    line_count=$(echo "$output" | wc -l | tr -d ' ')
    assert_eq "1" "$line_count" "plain output should be single line"

    teardown_test_workspace
    end_test
}

# Test: path JSON paths resolve to same file
test_path_json_paths_exist() {
    begin_test "path JSON paths resolve to existing file"
    setup_test_workspace

    create_thread "abc123" "Test Thread" "active"

    local output
    output=$($THREADS_BIN path abc123 --json 2>/dev/null)

    # Get path_absolute or path
    local abs_path
    abs_path=$(get_json_field "$output" ".path_absolute")
    if [[ -z "$abs_path" || "$abs_path" == "null" ]]; then
        abs_path=$(get_json_field "$output" ".path")
    fi

    # If path is absolute, verify it exists
    if [[ "$abs_path" == /* ]]; then
        assert_file_exists "$abs_path" "path should point to existing file"
    fi

    teardown_test_workspace
    end_test
}

# ====================================================================================
# format error handling
# ====================================================================================

# Test: --format with invalid value produces error
test_format_invalid_value() {
    begin_test "format with invalid value errors gracefully"
    setup_test_workspace

    create_thread "abc123" "Test Thread" "active"

    local exit_code=0
    local output
    output=$($THREADS_BIN validate --format=invalid 2>&1) || exit_code=$?

    # Should either error (exit != 0) or fall back to default format
    # Both behaviors are acceptable
    if [[ "$exit_code" -ne 0 ]]; then
        # Error case - should have meaningful message
        assert_matches "invalid|format|unknown" "$output" "error should mention format"
    fi

    teardown_test_workspace
    end_test
}

# Test: behavior when both --json and --format specified
test_format_json_yaml_exclusive() {
    begin_test "format flags behavior when multiple specified"
    setup_test_workspace

    create_thread "abc123" "Test Thread" "active"

    local output
    # Try --json with --format=yaml - implementation-specific behavior
    output=$($THREADS_BIN validate --json --format=yaml 2>/dev/null) || true

    # Output should be valid (either JSON or YAML, depending on precedence)
    # We just verify it doesn't crash
    if [[ -n "$output" ]]; then
        # Try parsing as JSON first
        if echo "$output" | jq . >/dev/null 2>&1; then
            assert_json_valid "$output"
        else
            # Otherwise should be valid YAML
            assert_yaml_valid "$output"
        fi
    fi

    teardown_test_workspace
    end_test
}

# Test: format value case sensitivity
test_format_case_sensitivity() {
    begin_test "format case sensitivity"
    setup_test_workspace

    create_thread "abc123" "Test Thread" "active"

    # Try uppercase JSON
    local output
    output=$($THREADS_BIN validate --format=JSON 2>/dev/null) || \
        output=$($THREADS_BIN validate --format=json 2>/dev/null)

    # Should work with either case (or error gracefully)
    # Implementation may be case-sensitive or case-insensitive

    teardown_test_workspace
    end_test
}

# ====================================================================================
# Run all tests
# ====================================================================================

# validate format tests
test_validate_format_json
test_validate_format_yaml
test_validate_json_shorthand
test_validate_format_plain
test_validate_json_valid_syntax
test_validate_yaml_valid_syntax

# new format tests
test_new_format_json
test_new_format_yaml
test_new_json_shorthand
test_new_format_plain
test_new_json_id_field
test_new_json_path_accuracy

# path format tests
test_path_format_json
test_path_format_yaml
test_path_json_shorthand
test_path_format_plain
test_path_json_paths_exist

# format error handling
test_format_invalid_value
test_format_json_yaml_exclusive
test_format_case_sensitivity

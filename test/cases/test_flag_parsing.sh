#!/usr/bin/env bash
# Tests for flag parsing variations across all implementations
# Ensures all implementations handle these argument styles consistently:
# - Long flags: --flag=value, --flag value
# - Short flags: -f value, -f=value
# - Boolean flags: --flag, -f

# ==============================================================================
# --down flag variations
# ==============================================================================

test_flag_down_equals_value() {
    begin_test "--down=N (equals style)"
    setup_nested_workspace

    create_thread "abc123" "Root Thread" "active"
    create_thread_at_category "def456" "Cat Thread" "cat1" "active"

    local output
    output=$($THREADS_BIN list --down=1 2>/dev/null)

    assert_contains "$output" "abc123" "should show root thread"
    assert_contains "$output" "def456" "should show cat thread with --down=1"

    teardown_test_workspace
    end_test
}

test_flag_down_space_value() {
    begin_test "--down N (space-separated)"
    setup_nested_workspace

    create_thread "abc123" "Root Thread" "active"
    create_thread_at_category "def456" "Cat Thread" "cat1" "active"

    local output
    output=$($THREADS_BIN list --down 1 2>/dev/null)

    assert_contains "$output" "abc123" "should show root thread"
    assert_contains "$output" "def456" "should show cat thread with --down 1"

    teardown_test_workspace
    end_test
}

test_flag_d_space_value() {
    begin_test "-d N (short flag space-separated)"
    setup_nested_workspace

    create_thread "abc123" "Root Thread" "active"
    create_thread_at_category "def456" "Cat Thread" "cat1" "active"

    local output
    output=$($THREADS_BIN list -d 1 2>/dev/null)

    assert_contains "$output" "abc123" "should show root thread"
    assert_contains "$output" "def456" "should show cat thread with -d 1"

    teardown_test_workspace
    end_test
}

test_flag_d_equals_value() {
    begin_test "-d=N (short flag equals style)"
    setup_nested_workspace

    create_thread "abc123" "Root Thread" "active"
    create_thread_at_category "def456" "Cat Thread" "cat1" "active"

    local output
    output=$($THREADS_BIN list -d=1 2>/dev/null)

    assert_contains "$output" "abc123" "should show root thread"
    assert_contains "$output" "def456" "should show cat thread with -d=1"

    teardown_test_workspace
    end_test
}

# ==============================================================================
# --up flag variations
# ==============================================================================

test_flag_up_equals_value() {
    begin_test "--up=N (equals style)"
    setup_nested_workspace

    create_thread "abc123" "Root Thread" "active"
    create_thread_at_project "def456" "Proj Thread" "cat1" "proj1" "active"

    cd "$TEST_WS/cat1/proj1" || exit 1
    local output
    output=$($THREADS_BIN list --up=1 2>/dev/null)
    cd "$TEST_WS" || exit 1

    assert_contains "$output" "def456" "should show project thread"
    # With up=1, should find threads at parent (cat1) level too if any

    teardown_test_workspace
    end_test
}

test_flag_up_space_value() {
    begin_test "--up N (space-separated)"
    setup_nested_workspace

    create_thread_at_category "abc123" "Cat Thread" "cat1" "active"
    create_thread_at_project "def456" "Proj Thread" "cat1" "proj1" "active"

    cd "$TEST_WS/cat1/proj1" || exit 1
    local output
    output=$($THREADS_BIN list --up 1 2>/dev/null)
    cd "$TEST_WS" || exit 1

    assert_contains "$output" "def456" "should show project thread"
    assert_contains "$output" "abc123" "should show cat thread with --up 1"

    teardown_test_workspace
    end_test
}

# ==============================================================================
# --status flag variations
# ==============================================================================

test_flag_status_equals_value() {
    begin_test "--status=VALUE (equals style)"
    setup_test_workspace

    create_thread "abc123" "Active Thread" "active"
    create_thread "def456" "Blocked Thread" "blocked"

    local output
    output=$($THREADS_BIN list --status=blocked 2>/dev/null)

    assert_not_contains "$output" "abc123" "should hide active thread"
    assert_contains "$output" "def456" "should show blocked thread with --status=blocked"

    teardown_test_workspace
    end_test
}

test_flag_status_space_value() {
    begin_test "--status VALUE (space-separated)"
    setup_test_workspace

    create_thread "abc123" "Active Thread" "active"
    create_thread "def456" "Blocked Thread" "blocked"

    local output
    output=$($THREADS_BIN list --status blocked 2>/dev/null)

    assert_not_contains "$output" "abc123" "should hide active thread"
    assert_contains "$output" "def456" "should show blocked thread with --status blocked"

    teardown_test_workspace
    end_test
}

# Note: -m short flag for status is not universal across implementations
# Some use --status only, so we skip testing -m

# ==============================================================================
# --search flag variations
# ==============================================================================

test_flag_search_equals_value() {
    begin_test "--search=VALUE (equals style)"
    setup_test_workspace

    create_thread "abc123" "Login Feature" "active"
    create_thread "def456" "Bug Fix" "active"

    local output
    output=$($THREADS_BIN list --search=Login 2>/dev/null)

    assert_contains "$output" "abc123" "should show matching thread with --search=Login"
    assert_not_contains "$output" "def456" "should hide non-matching thread"

    teardown_test_workspace
    end_test
}

test_flag_search_space_value() {
    begin_test "--search VALUE (space-separated)"
    setup_test_workspace

    create_thread "abc123" "Login Feature" "active"
    create_thread "def456" "Bug Fix" "active"

    local output
    output=$($THREADS_BIN list --search Login 2>/dev/null)

    assert_contains "$output" "abc123" "should show matching thread with --search Login"
    assert_not_contains "$output" "def456" "should hide non-matching thread"

    teardown_test_workspace
    end_test
}

test_flag_s_space_value() {
    begin_test "-s VALUE (short search flag)"
    setup_test_workspace

    create_thread "abc123" "Login Feature" "active"
    create_thread "def456" "Bug Fix" "active"

    local output
    output=$($THREADS_BIN list -s Login 2>/dev/null)

    assert_contains "$output" "abc123" "should show matching thread with -s Login"
    assert_not_contains "$output" "def456" "should hide non-matching thread"

    teardown_test_workspace
    end_test
}

# ==============================================================================
# --format flag variations
# Note: --format for list is not universal (Bun uses --json only)
# Test using 'path' command which has better --format support
# ==============================================================================

test_flag_format_equals_json() {
    begin_test "--format=json for path command"
    setup_test_workspace

    create_thread "abc123" "Test Thread" "active"

    local output
    output=$($THREADS_BIN path --format=json abc123 2>/dev/null)

    # JSON output should have certain markers
    assert_contains "$output" "abc123" "JSON should contain thread ID"
    # Should start with { (object)
    if [[ ! "$output" =~ ^\{  ]]; then
        fail_test "output should be JSON (starts with {)"
    fi

    teardown_test_workspace
    end_test
}

test_flag_format_space_json() {
    begin_test "--format json for path command"
    setup_test_workspace

    create_thread "abc123" "Test Thread" "active"

    local output
    output=$($THREADS_BIN path --format json abc123 2>/dev/null)

    assert_contains "$output" "abc123" "JSON should contain thread ID"
    if [[ ! "$output" =~ ^\{  ]]; then
        fail_test "output should be JSON (starts with {)"
    fi

    teardown_test_workspace
    end_test
}

test_flag_f_space_json() {
    begin_test "-f json for path command"
    setup_test_workspace

    create_thread "abc123" "Test Thread" "active"

    local output
    output=$($THREADS_BIN path -f json abc123 2>/dev/null)

    assert_contains "$output" "abc123" "JSON should contain thread ID"
    if [[ ! "$output" =~ ^\{  ]]; then
        fail_test "output should be JSON (starts with {)"
    fi

    teardown_test_workspace
    end_test
}

# ==============================================================================
# Boolean flags
# ==============================================================================

test_flag_recursive_boolean() {
    begin_test "-r (boolean recursive flag)"
    setup_nested_workspace

    create_thread "abc123" "Root Thread" "active"
    create_thread_at_category "def456" "Cat Thread" "cat1" "active"
    create_thread_at_project "ghi789" "Proj Thread" "cat1" "proj1" "active"

    local output
    output=$($THREADS_BIN list -r 2>/dev/null)

    assert_contains "$output" "abc123" "should show root thread"
    assert_contains "$output" "def456" "should show cat thread with -r"
    assert_contains "$output" "ghi789" "should show proj thread with -r"

    teardown_test_workspace
    end_test
}

test_flag_include_closed_boolean() {
    begin_test "--include-closed (boolean flag)"
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

test_flag_json_boolean() {
    begin_test "--json (boolean flag shorthand)"
    setup_test_workspace

    create_thread "abc123" "Test Thread" "active"

    local output
    output=$($THREADS_BIN list --json 2>/dev/null)

    assert_contains "$output" "abc123" "JSON should contain thread ID"
    if [[ ! "$output" =~ ^\[|\{  ]]; then
        fail_test "output should be JSON (starts with [ or {)"
    fi

    teardown_test_workspace
    end_test
}

# ==============================================================================
# Combined flags
# ==============================================================================

test_flags_combined_multiple() {
    begin_test "multiple flags combined"
    setup_nested_workspace

    create_thread "abc123" "Active Root" "active"
    create_thread "def456" "Blocked Root" "blocked"
    create_thread_at_category "ghi789" "Active Cat" "cat1" "active"
    create_thread_at_category "jkl012" "Blocked Cat" "cat1" "blocked"

    local output
    output=$($THREADS_BIN list --down=1 --status=blocked 2>/dev/null)

    assert_not_contains "$output" "abc123" "should hide active root thread"
    assert_contains "$output" "def456" "should show blocked root thread"
    assert_not_contains "$output" "ghi789" "should hide active cat thread"
    assert_contains "$output" "jkl012" "should show blocked cat thread"

    teardown_test_workspace
    end_test
}

test_flags_combined_short_and_long() {
    begin_test "mixed short and long flags"
    setup_nested_workspace

    create_thread "abc123" "Login Feature" "active"
    create_thread_at_category "def456" "Login Cat" "cat1" "active"
    create_thread_at_category "ghi789" "Other Cat" "cat1" "active"

    local output
    output=$($THREADS_BIN list -d 1 --search Login 2>/dev/null)

    assert_contains "$output" "abc123" "should show matching root thread"
    assert_contains "$output" "def456" "should show matching cat thread"
    assert_not_contains "$output" "ghi789" "should hide non-matching thread"

    teardown_test_workspace
    end_test
}

# ==============================================================================
# Edge cases
# ==============================================================================

test_flag_value_with_special_chars() {
    begin_test "flag value with spaces (quoted)"
    setup_test_workspace

    create_thread "abc123" "Login Feature Request" "active" "Description with spaces"
    create_thread "def456" "Bug Fix" "active"

    local output
    output=$($THREADS_BIN list --search "Feature Request" 2>/dev/null)

    assert_contains "$output" "abc123" "should find thread with multi-word search"
    assert_not_contains "$output" "def456" "should hide non-matching thread"

    teardown_test_workspace
    end_test
}

test_flag_down_zero_unlimited() {
    begin_test "--down=0 (unlimited depth)"
    setup_nested_workspace

    create_thread "abc123" "Root Thread" "active"
    create_thread_at_category "def456" "Cat Thread" "cat1" "active"
    create_thread_at_project "ghi789" "Proj Thread" "cat1" "proj1" "active"

    local output
    output=$($THREADS_BIN list --down=0 2>/dev/null)

    assert_contains "$output" "abc123" "should show root thread"
    assert_contains "$output" "def456" "should show cat thread with unlimited depth"
    assert_contains "$output" "ghi789" "should show proj thread with unlimited depth"

    teardown_test_workspace
    end_test
}

# Run all tests
test_flag_down_equals_value
test_flag_down_space_value
test_flag_d_space_value
test_flag_d_equals_value
test_flag_up_equals_value
test_flag_up_space_value
test_flag_status_equals_value
test_flag_status_space_value
test_flag_search_equals_value
test_flag_search_space_value
test_flag_s_space_value
test_flag_format_equals_json
test_flag_format_space_json
test_flag_f_space_json
test_flag_recursive_boolean
test_flag_include_closed_boolean
test_flag_json_boolean
test_flags_combined_multiple
test_flags_combined_short_and_long
test_flag_value_with_special_chars
test_flag_down_zero_unlimited

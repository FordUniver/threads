#!/usr/bin/env bash
# Tests for 'threads search' command

test_search_finds_body_content() {
    begin_test "search finds body content (fuzzy)"
    setup_test_workspace

    create_thread "abc123" "Auth Bug" "active" "Login fails"
    echo "Login fails on mobile" | $THREADS_BIN body abc123 --set >/dev/null 2>&1

    local output
    output=$($THREADS_BIN search "lgn mob" --format plain 2>/dev/null)

    assert_contains "$output" "abc123" "should include matching thread"

    teardown_test_workspace
    end_test
}

test_search_excludes_resolved_by_default() {
    begin_test "search excludes resolved by default"
    setup_test_workspace

    create_thread "abc123" "Active Thread" "active"
    echo "jwt validation" | $THREADS_BIN body abc123 --set >/dev/null 2>&1

    create_thread "def456" "Resolved Thread" "resolved"
    echo "jwt validation" | $THREADS_BIN body def456 --set >/dev/null 2>&1

    local output
    output=$($THREADS_BIN search "jwt" --format plain 2>/dev/null)

    assert_contains "$output" "abc123" "should show active match"
    assert_not_contains "$output" "def456" "should hide resolved match by default"

    local output2
    output2=$($THREADS_BIN search "jwt" --include-closed --format plain 2>/dev/null)

    assert_contains "$output2" "def456" "should show resolved match with --include-closed"

    teardown_test_workspace
    end_test
}

test_search_ranks_by_closeness() {
    begin_test "search ranks by closeness"
    setup_test_workspace

    create_thread "abc123" "Tight Match" "active"
    echo "jwt validation" | $THREADS_BIN body abc123 --set >/dev/null 2>&1

    create_thread "def456" "Loose Match" "active"
    echo "j w t validation" | $THREADS_BIN body def456 --set >/dev/null 2>&1

    local output
    output=$($THREADS_BIN search "jwt" --format plain 2>/dev/null)

    assert_contains "$output" "abc123" "should include tight match"
    assert_contains "$output" "def456" "should include loose match"

    local line_tight line_loose
    line_tight=$(echo "$output" | grep -n "abc123" | head -1 | cut -d: -f1)
    line_loose=$(echo "$output" | grep -n "def456" | head -1 | cut -d: -f1)

    assert_gt "$line_loose" "$line_tight" "tight match should be ranked above loose match"

    teardown_test_workspace
    end_test
}

test_search_respects_direction_flags() {
    begin_test "search respects direction flags"
    setup_nested_workspace

    create_thread "abc123" "Root Thread" "active"
    echo "root content" | $THREADS_BIN body abc123 --set >/dev/null 2>&1

    create_thread_at_category "def456" "Category Thread" "cat1" "active"
    echo "nested content" | $THREADS_BIN body def456 --set >/dev/null 2>&1

    local output
    output=$(cd "$TEST_WS" && $THREADS_BIN search "nested" --format plain 2>/dev/null)
    assert_not_contains "$output" "def456" "should not find nested thread without --down"

    local output2
    output2=$(cd "$TEST_WS" && $THREADS_BIN search "nested" --down --format plain 2>/dev/null)
    assert_contains "$output2" "def456" "should find nested thread with --down"

    teardown_test_workspace
    end_test
}

test_search_hints_about_closed_metadata_matches() {
    begin_test "search hints about closed metadata matches"
    setup_test_workspace

    create_thread "abc123" "Carola Aldo Olivia" "resolved" "draft"

    local output
    output=$($THREADS_BIN search "carola aldo olivia" --format plain 2>/dev/null)

    assert_not_contains "$output" "abc123" "should hide resolved match by default"
    assert_contains "$output" "closed thread metadata" "should hint about closed matches"

    teardown_test_workspace
    end_test
}

test_search_finds_body_content
test_search_excludes_resolved_by_default
test_search_ranks_by_closeness
test_search_respects_direction_flags
test_search_hints_about_closed_metadata_matches

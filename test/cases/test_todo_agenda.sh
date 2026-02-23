#!/usr/bin/env bash
# Tests for 'threads todo' agenda mode (no id → cross-scope view)

# Test: no todos → empty message
test_todo_agenda_empty() {
    begin_test "todo agenda: empty workspace"
    setup_test_workspace

    create_thread "abc123" "Empty Thread" "active"

    local output
    output=$(cd "$TEST_WS" && $THREADS_BIN todo 2>/dev/null)

    assert_contains "$output" "No open todos found." "should report no open todos"

    teardown_test_workspace
    end_test
}

# Test: open todo appears in agenda
test_todo_agenda_open_todo() {
    begin_test "todo agenda: open todo appears"
    setup_test_workspace

    create_thread "abc123" "My Thread" "active"
    $THREADS_BIN todo abc123 add "Write the report" >/dev/null 2>&1

    local output
    output=$(cd "$TEST_WS" && $THREADS_BIN todo 2>/dev/null)

    assert_contains "$output" "Write the report" "should show open todo text"
    assert_contains "$output" "abc123" "should show thread id"

    teardown_test_workspace
    end_test
}

# Test: checked todo absent by default, present with --include-done
test_todo_agenda_include_done() {
    begin_test "todo agenda: --include-done shows checked todos"
    setup_test_workspace

    create_thread "abc123" "My Thread" "active"

    local add_output hash
    add_output=$($THREADS_BIN todo abc123 add "Finished task" 2>/dev/null)
    hash=$(extract_hash_from_output "$add_output")

    if [[ -n "$hash" ]]; then
        $THREADS_BIN todo abc123 check "$hash" >/dev/null 2>&1
    fi

    local output_default output_done
    output_default=$(cd "$TEST_WS" && $THREADS_BIN todo 2>/dev/null)
    output_done=$(cd "$TEST_WS" && $THREADS_BIN todo --include-done 2>/dev/null)

    assert_not_contains "$output_default" "Finished task" "checked todo absent by default"
    assert_contains "$output_done" "Finished task" "checked todo present with --include-done"

    teardown_test_workspace
    end_test
}

# Test: resolved (closed) thread skipped by default, included with --include-closed
test_todo_agenda_closed_thread() {
    begin_test "todo agenda: resolved thread skipped by default"
    setup_test_workspace

    create_thread "abc123" "Open Thread" "active"
    create_thread "def456" "Resolved Thread" "resolved"

    $THREADS_BIN todo abc123 add "Open task" >/dev/null 2>&1
    $THREADS_BIN todo def456 add "Resolved task" >/dev/null 2>&1

    local output_default output_closed
    output_default=$(cd "$TEST_WS" && $THREADS_BIN todo 2>/dev/null)
    output_closed=$(cd "$TEST_WS" && $THREADS_BIN todo --include-closed 2>/dev/null)

    assert_contains "$output_default" "Open task" "open thread todo present"
    assert_not_contains "$output_default" "Resolved task" "resolved thread todo absent by default"
    assert_contains "$output_closed" "Resolved task" "resolved thread todo present with --include-closed"

    teardown_test_workspace
    end_test
}

# Test: --down collects from subdirectory threads
test_todo_agenda_down() {
    begin_test "todo agenda: --down collects subdirectory todos"
    setup_nested_workspace

    create_thread "abc123" "Root Thread" "active"
    create_thread_at_category "def456" "Cat Thread" "cat1" "active"

    $THREADS_BIN todo abc123 add "Root task" >/dev/null 2>&1
    $THREADS_BIN todo def456 add "Cat task" >/dev/null 2>&1

    local output
    output=$(cd "$TEST_WS" && $THREADS_BIN todo --down 2>/dev/null)

    assert_contains "$output" "Root task" "should show root todo"
    assert_contains "$output" "Cat task" "should show subdirectory todo with --down"

    teardown_test_workspace
    end_test
}

# Test: --json output has correct fields
test_todo_agenda_json() {
    begin_test "todo agenda: --json output has correct fields"
    setup_test_workspace

    create_thread "abc123" "JSON Thread" "active"
    $THREADS_BIN todo abc123 add "JSON task" >/dev/null 2>&1

    local output
    output=$(cd "$TEST_WS" && $THREADS_BIN todo --json 2>/dev/null)

    assert_json_valid "$output" "output should be valid JSON"
    assert_json_field "$output" ".[0].done" "false" "done field should be false"
    assert_contains "$(echo "$output" | jq -r '.[0].text' 2>/dev/null)" "JSON task" "text field should contain task"
    assert_json_field_not_empty "$output" ".[0].hash" "hash field should be present"
    assert_json_field "$output" ".[0].thread_id" "abc123" "thread_id should match"
    assert_json_field_not_empty "$output" ".[0].thread_name" "thread_name field should be present"
    assert_json_field_not_empty "$output" ".[0].thread_path" "thread_path field should be present"

    teardown_test_workspace
    end_test
}

# Test: multiple threads → all open todos aggregated
test_todo_agenda_multiple_threads() {
    begin_test "todo agenda: aggregates todos from multiple threads"
    setup_test_workspace

    create_thread "abc123" "Thread One" "active"
    create_thread "def456" "Thread Two" "active"
    create_thread "ghi789" "Thread Three" "active"

    $THREADS_BIN todo abc123 add "Task from one" >/dev/null 2>&1
    $THREADS_BIN todo def456 add "Task from two" >/dev/null 2>&1
    $THREADS_BIN todo ghi789 add "Task from three" >/dev/null 2>&1

    local output
    output=$(cd "$TEST_WS" && $THREADS_BIN todo 2>/dev/null)

    assert_contains "$output" "Task from one" "should show todo from thread one"
    assert_contains "$output" "Task from two" "should show todo from thread two"
    assert_contains "$output" "Task from three" "should show todo from thread three"

    teardown_test_workspace
    end_test
}

# Test: single-thread mode still works when id provided
test_todo_single_thread_unaffected() {
    begin_test "todo single-thread mode unaffected"
    setup_test_workspace

    create_thread "abc123" "My Thread" "active"
    $THREADS_BIN todo abc123 add "My task" >/dev/null 2>&1

    local output
    output=$($THREADS_BIN todo abc123 2>/dev/null)

    assert_contains "$output" "My task" "single-thread list still works"
    assert_contains "$output" "[ ]" "should show unchecked mark"

    teardown_test_workspace
    end_test
}

# Run all tests
test_todo_agenda_empty
test_todo_agenda_open_todo
test_todo_agenda_include_done
test_todo_agenda_closed_thread
test_todo_agenda_down
test_todo_agenda_json
test_todo_agenda_multiple_threads
test_todo_single_thread_unaffected

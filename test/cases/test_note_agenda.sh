#!/usr/bin/env bash
# Tests for 'threads note' agenda mode (no id → cross-scope view)

# Test: no notes → empty message
test_note_agenda_empty() {
    begin_test "note agenda: empty workspace"
    setup_test_workspace

    create_thread "abc123" "Empty Thread" "active"

    local output
    output=$(cd "$TEST_WS" && $THREADS_BIN note 2>/dev/null)

    assert_contains "$output" "No notes found." "should report no notes"

    teardown_test_workspace
    end_test
}

# Test: note in open thread appears in agenda
test_note_agenda_open_note() {
    begin_test "note agenda: note in open thread appears"
    setup_test_workspace

    create_thread "abc123" "My Thread" "active"
    $THREADS_BIN note abc123 add "Remember this" >/dev/null 2>&1

    local output
    output=$(cd "$TEST_WS" && $THREADS_BIN note 2>/dev/null)

    assert_contains "$output" "Remember this" "should show note text"
    assert_contains "$output" "abc123" "should show thread id"

    teardown_test_workspace
    end_test
}

# Test: note in resolved thread absent by default, present with --include-closed
test_note_agenda_closed_thread() {
    begin_test "note agenda: resolved thread skipped by default"
    setup_test_workspace

    create_thread "abc123" "Open Thread" "active"
    create_thread "def456" "Resolved Thread" "resolved"

    $THREADS_BIN note abc123 add "Open note" >/dev/null 2>&1
    $THREADS_BIN note def456 add "Resolved note" >/dev/null 2>&1

    local output_default output_closed
    output_default=$(cd "$TEST_WS" && $THREADS_BIN note 2>/dev/null)
    output_closed=$(cd "$TEST_WS" && $THREADS_BIN note --include-closed 2>/dev/null)

    assert_contains "$output_default" "Open note" "open thread note present"
    assert_not_contains "$output_default" "Resolved note" "resolved thread note absent by default"
    assert_contains "$output_closed" "Resolved note" "resolved thread note present with --include-closed"

    teardown_test_workspace
    end_test
}

# Test: --down collects from subdirectory threads
test_note_agenda_down() {
    begin_test "note agenda: --down collects subdirectory notes"
    setup_nested_workspace

    create_thread "abc123" "Root Thread" "active"
    create_thread_at_category "def456" "Cat Thread" "cat1" "active"

    $THREADS_BIN note abc123 add "Root note" >/dev/null 2>&1
    $THREADS_BIN note def456 add "Cat note" >/dev/null 2>&1

    local output
    output=$(cd "$TEST_WS" && $THREADS_BIN note --down 2>/dev/null)

    assert_contains "$output" "Root note" "should show root note"
    assert_contains "$output" "Cat note" "should show subdirectory note with --down"

    teardown_test_workspace
    end_test
}

# Test: --json output has correct fields
test_note_agenda_json() {
    begin_test "note agenda: --json output has correct fields"
    setup_test_workspace

    create_thread "abc123" "JSON Thread" "active"
    $THREADS_BIN note abc123 add "JSON note" >/dev/null 2>&1

    local output
    output=$(cd "$TEST_WS" && $THREADS_BIN note --json 2>/dev/null)

    assert_json_valid "$output" "output should be valid JSON"
    assert_contains "$(echo "$output" | jq -r '.[0].text' 2>/dev/null)" "JSON note" "text field should contain note"
    assert_json_field_not_empty "$output" ".[0].hash" "hash field should be present"
    assert_json_field "$output" ".[0].thread_id" "abc123" "thread_id should match"
    assert_json_field_not_empty "$output" ".[0].thread_name" "thread_name field should be present"
    assert_json_field_not_empty "$output" ".[0].thread_path" "thread_path field should be present"

    teardown_test_workspace
    end_test
}

# Test: multiple threads → all notes aggregated
test_note_agenda_multiple_threads() {
    begin_test "note agenda: aggregates notes from multiple threads"
    setup_test_workspace

    create_thread "abc123" "Thread One" "active"
    create_thread "def456" "Thread Two" "active"
    create_thread "ghi789" "Thread Three" "active"

    $THREADS_BIN note abc123 add "Note from one" >/dev/null 2>&1
    $THREADS_BIN note def456 add "Note from two" >/dev/null 2>&1
    $THREADS_BIN note ghi789 add "Note from three" >/dev/null 2>&1

    local output
    output=$(cd "$TEST_WS" && $THREADS_BIN note 2>/dev/null)

    assert_contains "$output" "Note from one" "should show note from thread one"
    assert_contains "$output" "Note from two" "should show note from thread two"
    assert_contains "$output" "Note from three" "should show note from thread three"

    teardown_test_workspace
    end_test
}

# Test: --yaml output is valid
test_note_agenda_yaml() {
    begin_test "note agenda: --yaml output is valid"
    setup_test_workspace

    create_thread "abc123" "YAML Thread" "active"
    $THREADS_BIN note abc123 add "YAML note" >/dev/null 2>&1

    local output
    output=$(cd "$TEST_WS" && $THREADS_BIN note --format=yaml 2>/dev/null)

    assert_yaml_valid "$output" "output should be valid YAML"

    teardown_test_workspace
    end_test
}

# Test: single-thread list still works when id provided
test_note_single_thread_list() {
    begin_test "note single-thread list unaffected"
    setup_test_workspace

    create_thread "abc123" "My Thread" "active"
    $THREADS_BIN note abc123 add "My note" >/dev/null 2>&1

    local output
    output=$($THREADS_BIN note abc123 2>/dev/null)

    assert_contains "$output" "My note" "single-thread list still works"

    teardown_test_workspace
    end_test
}

# Test: single-thread add still works when id provided
test_note_single_thread_add() {
    begin_test "note single-thread add unaffected"
    setup_test_workspace

    create_thread "abc123" "My Thread" "active"

    local output
    output=$($THREADS_BIN note abc123 add "Added note" 2>/dev/null)

    assert_contains "$output" "Added note" "add output confirms note was added"

    teardown_test_workspace
    end_test
}

# Run all tests
test_note_agenda_empty
test_note_agenda_open_note
test_note_agenda_closed_thread
test_note_agenda_down
test_note_agenda_json
test_note_agenda_yaml
test_note_agenda_multiple_threads
test_note_single_thread_list
test_note_single_thread_add

#!/usr/bin/env bash
# Tests for 'threads todo' command

# Test: todo add creates item (stored in YAML frontmatter)
test_todo_add() {
    begin_test "todo add creates item"
    setup_test_workspace

    create_thread "abc123" "Test Thread" "active"

    $THREADS_BIN todo abc123 add "New task" >/dev/null 2>&1

    local content
    content=$(cat "$(get_thread_path abc123)")

    assert_contains "$content" "New task" "should add todo text"
    assert_contains "$content" "done: false" "should have unchecked checkbox"

    teardown_test_workspace
    end_test
}

# Test: todo check marks item complete
test_todo_check() {
    begin_test "todo check marks complete"
    setup_test_workspace

    create_thread "abc123" "Test Thread" "active"

    # Add a todo
    local output
    output=$($THREADS_BIN todo abc123 add "Task to complete" 2>/dev/null)

    # Extract hash
    local hash
    hash=$(extract_hash_from_output "$output")

    if [[ -n "$hash" ]]; then
        # Check it
        $THREADS_BIN todo abc123 check "$hash" >/dev/null 2>&1

        local content
        content=$(cat "$(get_thread_path abc123)")

        assert_contains "$content" "done: true" "should have checked checkbox"
    fi

    teardown_test_workspace
    end_test
}

# Test: todo uncheck marks item incomplete
test_todo_uncheck() {
    begin_test "todo uncheck marks incomplete"
    setup_test_workspace

    create_thread "abc123" "Test Thread" "active"

    # Add and check a todo
    local output
    output=$($THREADS_BIN todo abc123 add "Task to uncheck" 2>/dev/null)
    local hash
    hash=$(extract_hash_from_output "$output")

    if [[ -n "$hash" ]]; then
        $THREADS_BIN todo abc123 check "$hash" >/dev/null 2>&1
        $THREADS_BIN todo abc123 uncheck "$hash" >/dev/null 2>&1

        local content
        content=$(cat "$(get_thread_path abc123)")

        assert_contains "$content" "done: false" "should have unchecked checkbox after uncheck"
    fi

    teardown_test_workspace
    end_test
}

# Test: todo remove deletes item
test_todo_remove() {
    begin_test "todo remove deletes item"
    setup_test_workspace

    create_thread "abc123" "Test Thread" "active"

    # Add a todo
    local output
    output=$($THREADS_BIN todo abc123 add "Task to remove" 2>/dev/null)
    local hash
    hash=$(extract_hash_from_output "$output")

    if [[ -n "$hash" ]]; then
        # Remove it
        $THREADS_BIN todo abc123 remove "$hash" >/dev/null 2>&1

        local content
        content=$(cat "$(get_thread_path abc123)")

        assert_not_contains "$content" "Task to remove" "todo should be removed"
    fi

    teardown_test_workspace
    end_test
}

# Test: todo list has correct YAML frontmatter format
test_todo_list_format() {
    begin_test "todo has correct checkbox format"
    setup_test_workspace

    create_thread "abc123" "Test Thread" "active"

    $THREADS_BIN todo abc123 add "Unchecked item" >/dev/null 2>&1

    local content
    content=$(cat "$(get_thread_path abc123)")

    # Todos stored in YAML frontmatter with done field and hash
    assert_contains "$content" "done: false" "should have done field"
    assert_contains "$content" "hash:" "should have hash field"

    teardown_test_workspace
    end_test
}

# Run all tests
test_todo_add
test_todo_check
test_todo_uncheck
test_todo_remove
test_todo_list_format

#!/usr/bin/env bash
# Tests for configuration system: ENV vars, manifests, sections

# ============================================================================
# ENV Variable Tests
# ============================================================================

# Test: NO_COLOR suppresses colored output
test_no_color_env() {
    begin_test "NO_COLOR suppresses colored output"
    setup_test_workspace

    create_thread "aaa001" "Test Thread" "active"

    # With NO_COLOR, output should not contain escape sequences
    local output
    output=$(NO_COLOR=1 capture_stdout $THREADS_BIN list)

    # Check for absence of ANSI escape codes - plain format uses pipe separators
    assert_contains "$output" "|" "should use plain format with NO_COLOR"

    teardown_test_workspace
    end_test
}

# Test: FORCE_COLOR enables colored output
test_force_color_env() {
    begin_test "FORCE_COLOR enables colored output"
    setup_test_workspace

    create_thread "aaa001" "Test Thread" "active"

    # With FORCE_COLOR, should use pretty format even when not a TTY
    local output
    output=$(FORCE_COLOR=1 capture_stdout $THREADS_BIN list)

    # Pretty output uses box drawing characters
    assert_contains "$output" "│" "should use box drawing with FORCE_COLOR"

    teardown_test_workspace
    end_test
}

# Test: THREADS_QUIET suppresses hints
test_threads_quiet_env() {
    begin_test "THREADS_QUIET suppresses hints"
    setup_test_workspace

    # Create thread with CLI (not direct file creation)
    local output
    output=$(capture_all $THREADS_BIN new "Test Thread" --desc "testing")

    # Without THREADS_QUIET, should show hint
    assert_contains "$output" "Note:" "should show hint without THREADS_QUIET"

    # With THREADS_QUIET, should not show hint
    output=$(THREADS_QUIET=1 capture_all $THREADS_BIN new "Another Thread" --desc "testing")

    assert_not_contains "$output" "Note:" "should not show Note with THREADS_QUIET"
    assert_not_contains "$output" "Hint:" "should not show Hint with THREADS_QUIET"

    teardown_test_workspace
    end_test
}

# Test: THREADS_INCLUDE_CLOSED includes closed threads
test_threads_include_closed_env() {
    begin_test "THREADS_INCLUDE_CLOSED includes closed threads"
    setup_test_workspace

    create_thread "aaa001" "Open Thread" "active"
    create_thread "bbb001" "Closed Thread" "resolved"

    # Without THREADS_INCLUDE_CLOSED, should not show resolved
    local output
    output=$(capture_stdout $THREADS_BIN list)
    assert_not_contains "$output" "bbb001" "should not show closed without env var"

    # With THREADS_INCLUDE_CLOSED, should show resolved
    output=$(THREADS_INCLUDE_CLOSED=1 capture_stdout $THREADS_BIN list)
    assert_contains "$output" "bbb001" "should show closed with THREADS_INCLUDE_CLOSED=1"

    teardown_test_workspace
    end_test
}

# ============================================================================
# Manifest Configuration Tests
# ============================================================================

# Test: Manifest loads custom default status
test_manifest_default_status() {
    begin_test "manifest sets custom default status"
    setup_test_workspace

    # Create manifest with custom default
    mkdir -p "$TEST_WS/.threads-config"
    cat > "$TEST_WS/.threads-config/manifest.yaml" << 'EOF'
defaults:
  new: planning
EOF

    # Create thread (should use planning, not idea)
    local output
    output=$(capture_all $THREADS_BIN new "Test Thread" --desc "testing")
    local id
    id=$(extract_id_from_output "$output")

    # Check status is planning
    local status
    status=$(get_thread_field "$id" "status")
    assert_eq "planning" "$status" "should use planning from manifest"

    teardown_test_workspace
    end_test
}

# Test: Manifest loads custom statuses
test_manifest_custom_statuses() {
    begin_test "manifest sets custom status list"
    setup_test_workspace

    # Create manifest with custom statuses
    mkdir -p "$TEST_WS/.threads-config"
    cat > "$TEST_WS/.threads-config/manifest.yaml" << 'EOF'
status:
  open: [draft, wip, review]
  closed: [merged, abandoned]
defaults:
  new: draft
  closed: merged
EOF

    # Create thread with custom status
    local output
    output=$(capture_all $THREADS_BIN new "Test Thread" --desc "testing" --status=wip)
    local id
    id=$(extract_id_from_output "$output")

    local status
    status=$(get_thread_field "$id" "status")
    assert_eq "wip" "$status" "should accept custom status 'wip'"

    # Standard status should be rejected
    output=$(capture_all $THREADS_BIN status "$id" active 2>&1)
    local exit_code=$?

    # exit_code should be 1 (failure)
    assert_eq "1" "$exit_code" "should reject non-custom status 'active'"
    assert_contains "$output" "Invalid status" "should show invalid status error"

    teardown_test_workspace
    end_test
}

# Test: Nested manifest overrides parent
test_manifest_nested_override() {
    begin_test "nested manifest overrides parent"
    setup_test_workspace

    # Create root manifest
    mkdir -p "$TEST_WS/.threads-config"
    cat > "$TEST_WS/.threads-config/manifest.yaml" << 'EOF'
defaults:
  new: idea
display:
  root_name: "root workspace"
EOF

    # Create nested manifest
    mkdir -p "$TEST_WS/subproject/.threads-config"
    cat > "$TEST_WS/subproject/.threads-config/manifest.yaml" << 'EOF'
defaults:
  new: planning
EOF

    # Check config from subproject shows override
    local output
    output=$(cd "$TEST_WS/subproject" && $THREADS_BIN config show --effective)

    assert_contains "$output" "new: planning" "should show overridden default"
    assert_contains "$output" "root_name: root workspace" "should inherit root_name from parent"

    teardown_test_workspace
    end_test
}

# Test: threads config show works
test_config_show() {
    begin_test "threads config show displays resolved config"
    setup_test_workspace

    local output
    output=$(capture_stdout $THREADS_BIN config show)

    # Should show all config sections
    assert_contains "$output" "status:" "should show status section"
    assert_contains "$output" "defaults:" "should show defaults section"
    assert_contains "$output" "behavior:" "should show behavior section"

    teardown_test_workspace
    end_test
}

# Test: threads config env works
test_config_env() {
    begin_test "threads config env lists environment variables"
    setup_test_workspace

    local output
    output=$(capture_stdout $THREADS_BIN config env)

    assert_contains "$output" "THREADS_QUIET" "should list THREADS_QUIET"
    assert_contains "$output" "NO_COLOR" "should list NO_COLOR"
    assert_contains "$output" "FORCE_COLOR" "should list FORCE_COLOR"

    teardown_test_workspace
    end_test
}

# Test: threads config schema outputs JSON
test_config_schema() {
    begin_test "threads config schema outputs valid JSON schema"
    setup_test_workspace

    local output
    output=$(capture_stdout $THREADS_BIN config schema)

    assert_contains "$output" '"$schema"' "should have schema field"
    assert_contains "$output" '"Config"' "should have Config definition"

    # Validate it's valid JSON using assert_json_valid
    assert_json_valid "$output" "output should be valid JSON"

    teardown_test_workspace
    end_test
}

# Test: threads config init creates template
test_config_init() {
    begin_test "threads config init creates manifest template"
    setup_test_workspace

    # Should not exist initially
    assert_file_not_exists "$TEST_WS/.threads-config/manifest.yaml"

    local output
    output=$(capture_all $THREADS_BIN config init)

    assert_file_exists "$TEST_WS/.threads-config/manifest.yaml" "should create manifest file"
    assert_contains "$output" "Created:" "should confirm creation"

    # File should contain commented template
    local content
    content=$(cat "$TEST_WS/.threads-config/manifest.yaml")
    assert_contains "$content" "status:" "template should have status section"

    teardown_test_workspace
    end_test
}

# ============================================================================
# Section Configuration Tests
# ============================================================================

# Test: Section disabled prevents operations
test_section_disabled() {
    begin_test "disabled section prevents operations"
    setup_test_workspace

    # Create manifest that disables Notes
    mkdir -p "$TEST_WS/.threads-config"
    cat > "$TEST_WS/.threads-config/manifest.yaml" << 'EOF'
sections:
  Notes: null
EOF

    create_thread "aaa001" "Test Thread" "active"

    # Try to add a note - should fail
    local output
    output=$(echo "test note" | capture_all $THREADS_BIN note aaa001 add 2>&1)
    local exit_code=$?

    # exit_code should be 1 (failure)
    assert_eq "1" "$exit_code" "should fail when Notes disabled"
    assert_contains "$output" "disabled" "should mention section is disabled"

    teardown_test_workspace
    end_test
}

# Test: Validation respects configured section names
test_section_validation() {
    begin_test "validation warns about unknown sections with custom config"
    setup_test_workspace

    # Create manifest with renamed sections
    mkdir -p "$TEST_WS/.threads-config"
    cat > "$TEST_WS/.threads-config/manifest.yaml" << 'EOF'
sections:
  Todo: Tasks
  Log: History
EOF

    # Create thread with standard section names (which are now "wrong")
    create_thread "aaa001" "Test Thread" "active"

    local output
    output=$(capture_all $THREADS_BIN validate 2>&1)

    # Should warn about unknown sections Todo and Log
    assert_contains "$output" "unknown section 'Todo'" "should warn about Todo"
    assert_contains "$output" "unknown section 'Log'" "should warn about Log"

    teardown_test_workspace
    end_test
}

# ============================================================================
# Terminology Tests (close/resolve aliases)
# ============================================================================

# Test: close command works
test_close_command() {
    begin_test "close command marks thread closed"
    setup_test_workspace

    create_thread "aaa001" "Test Thread" "active"

    local output
    output=$(capture_all $THREADS_BIN close aaa001)

    assert_contains "$output" "Closed:" "should confirm closure"

    local status
    status=$(get_thread_field "aaa001" "status")
    assert_eq "resolved" "$status" "should be resolved"

    teardown_test_workspace
    end_test
}

# Test: resolve alias works
test_resolve_alias() {
    begin_test "resolve is alias for close"
    setup_test_workspace

    create_thread "aaa001" "Test Thread" "active"

    local output
    output=$(capture_all $THREADS_BIN resolve aaa001)

    assert_contains "$output" "Closed:" "should confirm closure via alias"

    teardown_test_workspace
    end_test
}

# Test: --include-closed flag works
test_include_closed_flag() {
    begin_test "--include-closed includes closed threads"
    setup_test_workspace

    create_thread "aaa001" "Open Thread" "active"
    create_thread "bbb001" "Closed Thread" "resolved"

    local output
    output=$(capture_stdout $THREADS_BIN list --include-closed)

    assert_contains "$output" "aaa001" "should include open thread"
    assert_contains "$output" "bbb001" "should include closed thread"

    teardown_test_workspace
    end_test
}

# Test: --include-concluded alias works
test_include_concluded_alias() {
    begin_test "--include-concluded is alias for --include-closed"
    setup_test_workspace

    create_thread "aaa001" "Open Thread" "active"
    create_thread "bbb001" "Closed Thread" "resolved"

    local output
    output=$(capture_stdout $THREADS_BIN list --include-concluded)

    assert_contains "$output" "bbb001" "should include closed thread via alias"

    teardown_test_workspace
    end_test
}

# ============================================================================
# Git History Inference Tests (Phase 8)
# ============================================================================

# Test: Reopen restores previous status from git history
test_reopen_git_history() {
    begin_test "reopen restores previous status from git history"
    setup_test_workspace

    # Create thread and commit
    create_thread "aaa001" "Test Thread" "active"
    git -C "$TEST_WS" add .threads
    git -C "$TEST_WS" commit -q -m "initial"

    # Change to blocked and commit
    $THREADS_BIN status aaa001 blocked >/dev/null 2>&1
    git -C "$TEST_WS" add .threads
    git -C "$TEST_WS" commit -q -m "blocked"

    # Close and commit
    $THREADS_BIN close aaa001 >/dev/null 2>&1
    git -C "$TEST_WS" add .threads
    git -C "$TEST_WS" commit -q -m "closed"

    # Reopen - should restore to blocked (previous open status)
    local output
    output=$(capture_all $THREADS_BIN reopen aaa001)

    local status
    status=$(get_thread_field "aaa001" "status")
    assert_eq "blocked" "$status" "should restore to blocked from git history"

    teardown_test_workspace
    end_test
}

# Test: Reopen falls back to config default when no git history
test_reopen_fallback_default() {
    begin_test "reopen falls back to config default without git history"
    setup_test_workspace

    # Create manifest with custom reopen default
    mkdir -p "$TEST_WS/.threads-config"
    cat > "$TEST_WS/.threads-config/manifest.yaml" << 'EOF'
defaults:
  open: planning
EOF

    # Create thread directly as resolved (no history)
    create_thread "aaa001" "Test Thread" "resolved"

    # Reopen - should use config default (planning)
    local output
    output=$(capture_all $THREADS_BIN reopen aaa001)

    local status
    status=$(get_thread_field "aaa001" "status")
    assert_eq "planning" "$status" "should use config default without history"

    teardown_test_workspace
    end_test
}

# Test: display.root_name customizes output
test_display_root_name() {
    begin_test "display.root_name customizes repo root display"
    setup_test_workspace

    # Create manifest with custom root_name
    mkdir -p "$TEST_WS/.threads-config"
    cat > "$TEST_WS/.threads-config/manifest.yaml" <<EOF
display:
  root_name: "workspace root"
EOF

    # Create a thread so list has something
    create_thread "aaa001" "Test Thread" "active"

    # Use plain format to check root_name (it shows "Showing X threads in <root_name>")
    local output
    output=$(capture_stdout $THREADS_BIN list --format=plain)

    assert_contains "$output" "workspace root" "should use custom root_name in output"

    teardown_test_workspace
    end_test
}

# ============================================================================
# Run all tests
# ============================================================================

# ENV var tests
test_no_color_env
test_force_color_env
test_threads_quiet_env
test_threads_include_closed_env

# Manifest tests
test_manifest_default_status
test_manifest_custom_statuses
test_manifest_nested_override
test_config_show
test_config_env
test_config_schema
test_config_init
test_display_root_name

# Section tests
test_section_disabled
test_section_validation

# Terminology tests
test_close_command
test_resolve_alias
test_include_closed_flag
test_include_concluded_alias

# Git history tests
test_reopen_git_history
test_reopen_fallback_default

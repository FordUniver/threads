#!/usr/bin/env bash
# Assertion library for threads CLI tests
# Semantic validation - checks content presence, not exact formatting

# Colors (disabled if not a terminal)
if [[ -t 1 ]]; then
    RED='\033[0;31m'
    GREEN='\033[0;32m'
    YELLOW='\033[0;33m'
    NC='\033[0m'
else
    RED='' GREEN='' YELLOW='' NC=''
fi

# Track test results
_TEST_PASSED=0
_TEST_FAILED=0
_TEST_CURRENT=""
_CURRENT_TEST_FAILED=""
_DIAGNOSTIC_OUTPUT=""

# Start a test (call before assertions)
begin_test() {
    _TEST_CURRENT="$1"
    _CURRENT_TEST_FAILED=""
    _DIAGNOSTIC_OUTPUT=""
}

# Record failure with details (no TAP output - accumulated for end_test)
_fail() {
    local msg="$1"
    _CURRENT_TEST_FAILED=1

    # Accumulate diagnostic output
    _DIAGNOSTIC_OUTPUT="${_DIAGNOSTIC_OUTPUT}  # $msg\n"
    if [[ -n "${2:-}" ]]; then
        _DIAGNOSTIC_OUTPUT="${_DIAGNOSTIC_OUTPUT}  #   expected: $2\n"
    fi
    if [[ -n "${3:-}" ]]; then
        _DIAGNOSTIC_OUTPUT="${_DIAGNOSTIC_OUTPUT}  #   actual: $3\n"
    fi
}

# Assert two values are equal
assert_eq() {
    local expected="$1"
    local actual="$2"
    local msg="${3:-values should be equal}"

    if [[ "$expected" == "$actual" ]]; then
        return 0
    else
        _fail "$msg" "$expected" "$actual"
        return 1
    fi
}

# Assert haystack contains needle
assert_contains() {
    local haystack="$1"
    local needle="$2"
    local msg="${3:-should contain}"

    if [[ "$haystack" == *"$needle"* ]]; then
        return 0
    else
        _fail "$msg: expected to contain '$needle'" "" "${haystack:0:200}"
        return 1
    fi
}

# Assert haystack does NOT contain needle
assert_not_contains() {
    local haystack="$1"
    local needle="$2"
    local msg="${3:-should not contain}"

    if [[ "$haystack" != *"$needle"* ]]; then
        return 0
    else
        _fail "$msg: should not contain '$needle'" "" "${haystack:0:200}"
        return 1
    fi
}

# Assert file exists
assert_file_exists() {
    local path="$1"
    local msg="${2:-file should exist}"

    if [[ -f "$path" ]]; then
        return 0
    else
        _fail "$msg: $path"
        return 1
    fi
}

# Assert file does NOT exist
assert_file_not_exists() {
    local path="$1"
    local msg="${2:-file should not exist}"

    if [[ ! -f "$path" ]]; then
        return 0
    else
        _fail "$msg: $path exists"
        return 1
    fi
}

# Assert directory exists
assert_dir_exists() {
    local path="$1"
    local msg="${2:-directory should exist}"

    if [[ -d "$path" ]]; then
        return 0
    else
        _fail "$msg: $path"
        return 1
    fi
}

# Assert command exits with expected code
# Usage: assert_exit_code 0 "$THREADS_BIN" list
assert_exit_code() {
    local expected="$1"
    shift
    local actual

    "$@" >/dev/null 2>&1
    actual=$?

    if [[ "$actual" -eq "$expected" ]]; then
        return 0
    else
        _fail "expected exit code $expected, got $actual" "$expected" "$actual"
        return 1
    fi
}

# Assert stdout contains text
# Usage: assert_stdout_contains "needle" "$THREADS_BIN" list
assert_stdout_contains() {
    local needle="$1"
    shift
    local output

    output=$("$@" 2>/dev/null) || true

    if [[ "$output" == *"$needle"* ]]; then
        return 0
    else
        _fail "stdout should contain '$needle'" "" "${output:0:200}"
        return 1
    fi
}

# Assert stdout does NOT contain text
assert_stdout_not_contains() {
    local needle="$1"
    shift
    local output

    output=$("$@" 2>/dev/null) || true

    if [[ "$output" != *"$needle"* ]]; then
        return 0
    else
        _fail "stdout should not contain '$needle'" "" "${output:0:200}"
        return 1
    fi
}

# Assert stderr contains text
# Usage: assert_stderr_contains "error" "$THREADS_BIN" bad-command
assert_stderr_contains() {
    local needle="$1"
    shift
    local output

    output=$("$@" 2>&1 >/dev/null) || true

    if [[ "$output" == *"$needle"* ]]; then
        return 0
    else
        _fail "stderr should contain '$needle'" "" "${output:0:200}"
        return 1
    fi
}

# Assert output matches regex
assert_matches() {
    local pattern="$1"
    local actual="$2"
    local msg="${3:-should match pattern}"

    if [[ "$actual" =~ $pattern ]]; then
        return 0
    else
        _fail "$msg: expected to match '$pattern'" "" "${actual:0:200}"
        return 1
    fi
}

# Assert numeric comparison
assert_gt() {
    local actual="$1"
    local threshold="$2"
    local msg="${3:-should be greater than}"

    if [[ "$actual" -gt "$threshold" ]]; then
        return 0
    else
        _fail "$msg $threshold" ">$threshold" "$actual"
        return 1
    fi
}

# End test and output TAP result
end_test() {
    # Skip if no test is active
    if [[ -z "$_TEST_CURRENT" ]]; then
        return
    fi

    # Calculate test number
    local test_num=$((_TEST_PASSED + _TEST_FAILED + 1))

    # Output single TAP line based on test result
    if [[ -n "$_CURRENT_TEST_FAILED" ]]; then
        ((_TEST_FAILED++))
        echo -e "${RED}not ok${NC} $test_num - $_TEST_CURRENT"
        # Output accumulated diagnostics
        if [[ -n "$_DIAGNOSTIC_OUTPUT" ]]; then
            echo -e "$_DIAGNOSTIC_OUTPUT"
        fi
    else
        ((_TEST_PASSED++))
        echo -e "${GREEN}ok${NC} $test_num - $_TEST_CURRENT"
    fi

    # Reset state
    _TEST_CURRENT=""
    _CURRENT_TEST_FAILED=""
    _DIAGNOSTIC_OUTPUT=""
}

# Print summary and return appropriate exit code
print_summary() {
    local total=$((_TEST_PASSED + _TEST_FAILED))
    echo ""
    echo "1..$total"
    if [[ $_TEST_FAILED -eq 0 ]]; then
        echo -e "${GREEN}All $total tests passed${NC}"
        return 0
    else
        echo -e "${RED}$_TEST_FAILED of $total tests failed${NC}"
        return 1
    fi
}

# Reset counters (for running multiple test files)
reset_counters() {
    _TEST_PASSED=0
    _TEST_FAILED=0
    _TEST_CURRENT=""
    _CURRENT_TEST_FAILED=""
    _DIAGNOSTIC_OUTPUT=""

    # Reset setup/teardown state for clean slate between test files
    if [[ -n "$TEST_WS" && -d "$TEST_WS" ]]; then
        rm -rf "$TEST_WS" 2>/dev/null || true
    fi
    TEST_WS=""
    _ORIGINAL_PWD=""
    unset WORKSPACE 2>/dev/null || true
}

# ====================================================================================
# JSON/YAML Validation Assertions
# ====================================================================================

# Assert output is valid JSON
# Usage: assert_json_valid "$output" ["description"]
assert_json_valid() {
    local output="$1"
    local msg="${2:-JSON should be valid}"

    # Reject empty output - empty string is not valid JSON
    if [[ -z "$output" || "$output" =~ ^[[:space:]]*$ ]]; then
        _fail "$msg: empty output is not valid JSON"
        return 1
    fi

    if echo "$output" | jq . >/dev/null 2>&1; then
        return 0
    else
        _fail "$msg: invalid JSON" "" "${output:0:200}"
        return 1
    fi
}

# Assert JSON field equals expected value
# Usage: assert_json_field "$output" ".field" "expected"
assert_json_field() {
    local output="$1"
    local field="$2"
    local expected="$3"
    local msg="${4:-JSON field $field}"
    local actual

    actual=$(echo "$output" | jq -r "$field" 2>/dev/null)
    if [[ "$actual" == "$expected" ]]; then
        return 0
    else
        _fail "$msg" "$expected" "$actual"
        return 1
    fi
}

# Assert JSON has a field (not null)
# Usage: assert_json_has_field "$output" ".field"
assert_json_has_field() {
    local output="$1"
    local field="$2"
    local msg="${3:-JSON should have field $field}"

    if echo "$output" | jq -e "$field" >/dev/null 2>&1; then
        return 0
    else
        _fail "$msg"
        return 1
    fi
}

# Assert JSON field is not empty string
# Usage: assert_json_field_not_empty "$output" ".field"
assert_json_field_not_empty() {
    local output="$1"
    local field="$2"
    local msg="${3:-JSON field $field should not be empty}"
    local value

    value=$(echo "$output" | jq -r "$field" 2>/dev/null)
    if [[ -n "$value" && "$value" != "null" ]]; then
        return 0
    else
        _fail "$msg" "non-empty" "$value"
        return 1
    fi
}

# Assert output is valid YAML (requires yq)
# Usage: assert_yaml_valid "$output" ["description"]
assert_yaml_valid() {
    local output="$1"
    local msg="${2:-YAML should be valid}"

    # Require yq - CI images must have it installed
    if ! command -v yq >/dev/null 2>&1; then
        _fail "$msg: yq not installed (required for YAML validation)"
        return 1
    fi

    # Reject empty output - empty string is not valid YAML
    if [[ -z "$output" || "$output" =~ ^[[:space:]]*$ ]]; then
        _fail "$msg: empty output is not valid YAML"
        return 1
    fi

    if echo "$output" | yq . >/dev/null 2>&1; then
        return 0
    else
        _fail "$msg: invalid YAML" "" "${output:0:200}"
        return 1
    fi
}

# Assert YAML field equals expected value (requires yq)
# Usage: assert_yaml_field "$output" ".field" "expected"
assert_yaml_field() {
    local output="$1"
    local field="$2"
    local expected="$3"
    local msg="${4:-YAML field $field}"
    local actual

    # Require yq - CI images must have it installed
    if ! command -v yq >/dev/null 2>&1; then
        _fail "$msg: yq not installed (required for YAML validation)"
        return 1
    fi

    actual=$(echo "$output" | yq -r "$field" 2>/dev/null)
    if [[ "$actual" == "$expected" ]]; then
        return 0
    else
        _fail "$msg" "$expected" "$actual"
        return 1
    fi
}

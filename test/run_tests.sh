#!/usr/bin/env bash
# Test runner for threads CLI
# Usage: ./run_tests.sh [options] [binary] [test_file...]
# Examples:
#   ./run_tests.sh                              # Test shell impl (default)
#   ./run_tests.sh ./go/bin/threads             # Test Go impl
#   ./run_tests.sh "python -m threads"          # Test Python impl
#   ./run_tests.sh ./shell/threads test_new.sh  # Run specific test file
#   ./run_tests.sh --validate                   # Verify tests pass individually too

set -uo pipefail
# Note: -e removed because assertion failures return non-zero and we want to continue

# === CRITICAL: Sanitize environment before anything else ===
# Prevents parent shell pollution (e.g., WORKSPACE already set)
unset WORKSPACE TEST_WS _ORIGINAL_PWD 2>/dev/null || true
unset _TEST_PASSED _TEST_FAILED _TEST_CURRENT 2>/dev/null || true

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_DIR="$(dirname "$SCRIPT_DIR")"

# Check for --validate flag
VALIDATE_MODE=false
if [[ "${1:-}" == "--validate" ]]; then
    VALIDATE_MODE=true
    shift
fi

# Parse arguments
# First arg could be a binary path or a test file
if [[ $# -gt 0 && ! "$1" =~ \.sh$ && ! "$1" =~ ^cases/ ]]; then
    THREADS_BIN="$1"
    shift
else
    THREADS_BIN="$REPO_DIR/shell/threads"
fi

# Convert all relative paths in THREADS_BIN to absolute paths
# This ensures paths work correctly inside subshells that cd to different directories
convert_relative_paths() {
    local cmd="$1"
    local result=""
    local word
    for word in $cmd; do
        # Check if it's a relative path (contains / but doesn't start with /)
        if [[ "$word" == */* && ! "$word" =~ ^/ ]]; then
            # Try to resolve it
            local dir base
            dir="$(dirname "$word")"
            base="$(basename "$word")"
            if [[ -d "$dir" ]]; then
                word="$(cd "$dir" && pwd)/$base"
            fi
        fi
        result="${result:+$result }$word"
    done
    echo "$result"
}
THREADS_BIN="$(convert_relative_paths "$THREADS_BIN")"

# Remaining args are test files (or empty for all)
TEST_FILES=("$@")

# Export for test files
export THREADS_BIN
export SCRIPT_DIR
export REPO_DIR

# Source libraries
source "$SCRIPT_DIR/lib/assertions.sh"
source "$SCRIPT_DIR/lib/setup.sh"
source "$SCRIPT_DIR/lib/helpers.sh"

# Colors
if [[ -t 1 ]]; then
    BOLD='\033[1m'
    DIM='\033[2m'
    RED='\033[0;31m'
    GREEN='\033[0;32m'
    YELLOW='\033[0;33m'
    NC='\033[0m'
else
    BOLD='' DIM='' RED='' GREEN='' YELLOW='' NC=''
fi

echo -e "${BOLD}threads CLI test suite${NC}"
echo -e "${DIM}Binary: $THREADS_BIN${NC}"
echo ""

# Verify binary exists/works
if ! command -v ${THREADS_BIN%% *} >/dev/null 2>&1; then
    # Try as a path
    if [[ ! -x "${THREADS_BIN%% *}" ]]; then
        echo "Error: Cannot find or execute: $THREADS_BIN" >&2
        exit 1
    fi
fi

# Find test files
if [[ ${#TEST_FILES[@]} -eq 0 ]]; then
    # Run all test files
    mapfile -t TEST_FILES < <(find "$SCRIPT_DIR/cases" -name "test_*.sh" -type f | sort)
fi

# Track overall results
TOTAL_PASSED=0
TOTAL_FAILED=0
FAILED_TESTS=()

# Run each test file in isolated subshell
for test_file in "${TEST_FILES[@]}"; do
    # Handle relative paths
    if [[ ! "$test_file" =~ ^/ ]]; then
        if [[ -f "$SCRIPT_DIR/cases/$test_file" ]]; then
            test_file="$SCRIPT_DIR/cases/$test_file"
        elif [[ -f "$SCRIPT_DIR/$test_file" ]]; then
            test_file="$SCRIPT_DIR/$test_file"
        fi
    fi

    if [[ ! -f "$test_file" ]]; then
        echo "Warning: Test file not found: $test_file" >&2
        continue
    fi

    test_name=$(basename "$test_file" .sh)
    echo -e "${BOLD}# $test_name${NC}"

    # Run test file in ISOLATED subshell
    # This prevents environment pollution between test files
    result=$(bash -c '
        # Clean environment - no inherited WORKSPACE
        unset WORKSPACE TEST_WS _ORIGINAL_PWD 2>/dev/null || true

        # Import test framework
        source "'"$SCRIPT_DIR"'/lib/assertions.sh"
        source "'"$SCRIPT_DIR"'/lib/setup.sh"
        source "'"$SCRIPT_DIR"'/lib/helpers.sh"

        # Export required vars
        export THREADS_BIN="'"$THREADS_BIN"'"
        export SCRIPT_DIR="'"$SCRIPT_DIR"'"
        export REPO_DIR="'"$REPO_DIR"'"

        # Run the test file
        source "'"$test_file"'" || true

        # Output results in parseable format (last line)
        echo "___RESULTS___:${_TEST_PASSED}:${_TEST_FAILED}"
    ' 2>&1)

    # Parse output: everything except last line is test output
    # Last line contains results
    output_lines=()
    file_passed=0
    file_failed=0

    while IFS= read -r line; do
        if [[ "$line" == "___RESULTS___:"* ]]; then
            # Parse results line
            IFS=: read -r _ passed failed <<< "$line"
            file_passed=$passed
            file_failed=$failed
        else
            output_lines+=("$line")
        fi
    done <<< "$result"

    # Print test output
    printf '%s\n' "${output_lines[@]}"

    # Accumulate results
    TOTAL_PASSED=$((file_passed + TOTAL_PASSED))
    TOTAL_FAILED=$((file_failed + TOTAL_FAILED))

    if [[ $file_failed -gt 0 ]]; then
        FAILED_TESTS+=("$test_name")
    fi

    echo ""
done

# Final summary
echo "========================"
TOTAL=$((TOTAL_PASSED + TOTAL_FAILED))
echo "1..$TOTAL"

if [[ $TOTAL_FAILED -eq 0 ]]; then
    echo -e "${GREEN}All $TOTAL tests passed${NC}"
else
    echo -e "${RED}$TOTAL_FAILED of $TOTAL tests failed${NC}"
    echo ""
    echo "Failed test files:"
    for name in "${FAILED_TESTS[@]}"; do
        echo "  - $name"
    done
fi

# Validate mode: also run each test file individually and compare
if $VALIDATE_MODE; then
    echo ""
    echo -e "${BOLD}=== Validation Mode ===${NC}"
    echo "Running each test file individually to verify isolation..."
    echo ""

    individual_failed=0
    for test_file in "${TEST_FILES[@]}"; do
        # Handle relative paths (same logic as above)
        if [[ ! "$test_file" =~ ^/ ]]; then
            if [[ -f "$SCRIPT_DIR/cases/$test_file" ]]; then
                test_file="$SCRIPT_DIR/cases/$test_file"
            elif [[ -f "$SCRIPT_DIR/$test_file" ]]; then
                test_file="$SCRIPT_DIR/$test_file"
            fi
        fi
        [[ ! -f "$test_file" ]] && continue

        test_name=$(basename "$test_file" .sh)

        # Run this single test file in complete isolation
        if ! "$0" "$THREADS_BIN" "$test_file" >/dev/null 2>&1; then
            echo -e "${RED}FAIL${NC} (individual): $test_name"
            individual_failed=1
        else
            echo -e "${GREEN}ok${NC} (individual): $test_name"
        fi
    done

    echo ""
    if [[ $individual_failed -eq 1 ]]; then
        echo -e "${RED}ISOLATION ERROR: Some tests fail when run individually${NC}"
        echo "This indicates hidden dependencies between test files."
        exit 1
    elif [[ $TOTAL_FAILED -gt 0 ]]; then
        echo -e "${YELLOW}Tests fail both together and individually (consistent)${NC}"
        exit 1
    else
        echo -e "${GREEN}Validation passed: Tests work both together and individually${NC}"
        exit 0
    fi
fi

# Exit with appropriate code
if [[ $TOTAL_FAILED -eq 0 ]]; then
    exit 0
else
    exit 1
fi

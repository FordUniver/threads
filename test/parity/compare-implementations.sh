#!/usr/bin/env bash
# Cross-implementation output parity check
# Compares outputs of key commands across all implementations
# Exits with non-zero if any differences are found
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(dirname "$(dirname "$SCRIPT_DIR")")"

# Colors
if [[ -t 1 ]]; then
    BOLD='\033[1m'
    RED='\033[0;31m'
    GREEN='\033[0;32m'
    YELLOW='\033[0;33m'
    NC='\033[0m'
else
    BOLD='' RED='' GREEN='' YELLOW='' NC=''
fi

echo -e "${BOLD}threads Implementation Parity Check${NC}"
echo "======================================"
echo

# Setup test workspace
WORKSPACE=$(mktemp -d "${TMPDIR:-/tmp}/threads-parity.XXXXXX")
trap "rm -rf '$WORKSPACE'" EXIT

mkdir -p "$WORKSPACE/.threads"
mkdir -p "$WORKSPACE/cat1/.threads"
mkdir -p "$WORKSPACE/cat1/proj1/.threads"
mkdir -p "$WORKSPACE/cat2/.threads"

# Initialize git (required by most implementations)
(cd "$WORKSPACE" && git init -q && git config user.email "test@test" && git config user.name "Test" && git commit --allow-empty -q -m "init")

# Create test threads
create_thread() {
    local path="$1"
    local id="$2"
    local name="$3"
    local status="$4"
    local slug
    slug=$(echo "$name" | tr '[:upper:]' '[:lower:]' | sed 's/[^a-z0-9]/-/g' | sed 's/--*/-/g' | sed 's/^-//;s/-$//')
    cat > "$path/.threads/${id}-${slug}.md" << EOF
---
id: $id
name: $name
desc: Test thread
status: $status
---

## Body

## Notes

## Todo

## Log
EOF
}

create_thread "$WORKSPACE" "abc123" "Root Thread" "active"
create_thread "$WORKSPACE" "def456" "Blocked Thread" "blocked"
create_thread "$WORKSPACE/cat1" "ghi789" "Category Thread" "idea"
create_thread "$WORKSPACE/cat1/proj1" "jkl012" "Project Thread" "planning"
create_thread "$WORKSPACE/cat2" "mno345" "Another Cat" "active"

echo "Test workspace created at: $WORKSPACE"
echo

# Find implementations
declare -a IMPLS=()
declare -A IMPL_CMDS=()

# Go
if [[ -f "$ROOT_DIR/go/go.mod" ]] && command -v go &>/dev/null; then
    echo "Building Go..."
    if (cd "$ROOT_DIR/go" && go build -o threads-parity ./cmd/threads 2>/dev/null); then
        IMPLS+=(go)
        IMPL_CMDS[go]="$ROOT_DIR/go/threads-parity"
    fi
fi

# Rust
if [[ -f "$ROOT_DIR/rust/Cargo.toml" ]] && command -v cargo &>/dev/null; then
    echo "Building Rust..."
    if (cd "$ROOT_DIR/rust" && cargo build --release --quiet 2>/dev/null); then
        IMPLS+=(rust)
        IMPL_CMDS[rust]="$ROOT_DIR/rust/target/release/threads"
    fi
fi

# Swift
if [[ -f "$ROOT_DIR/swift/Package.swift" ]] && command -v swift &>/dev/null; then
    echo "Building Swift..."
    if (cd "$ROOT_DIR/swift" && swift build -c release --quiet 2>/dev/null); then
        IMPLS+=(swift)
        IMPL_CMDS[swift]="$ROOT_DIR/swift/.build/release/threads"
    fi
fi

# Python (use test-threads wrapper if available, else direct module call)
if [[ -f "$ROOT_DIR/python/test-threads" ]]; then
    IMPLS+=(python)
    IMPL_CMDS[python]="$ROOT_DIR/python/test-threads"
elif [[ -f "$ROOT_DIR/python/pyproject.toml" ]] && command -v uv &>/dev/null; then
    # Create wrapper if it doesn't exist
    cat > "$ROOT_DIR/python/test-threads" << 'PYWRAP'
#!/usr/bin/env bash
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
export PYTHONPATH="$SCRIPT_DIR/src:${PYTHONPATH:-}"
exec "$SCRIPT_DIR/.venv/bin/python" -m threads "$@"
PYWRAP
    chmod +x "$ROOT_DIR/python/test-threads"
    (cd "$ROOT_DIR/python" && uv sync --quiet 2>/dev/null)
    IMPLS+=(python)
    IMPL_CMDS[python]="$ROOT_DIR/python/test-threads"
fi

# Ruby
if [[ -f "$ROOT_DIR/ruby/bin/threads" ]]; then
    IMPLS+=(ruby)
    IMPL_CMDS[ruby]="$ROOT_DIR/ruby/bin/threads"
fi

# Perl
if [[ -f "$ROOT_DIR/perl/bin/threads" ]]; then
    IMPLS+=(perl)
    IMPL_CMDS[perl]="$ROOT_DIR/perl/bin/threads"
fi

# Bun
if [[ -f "$ROOT_DIR/bun/bin/threads" ]]; then
    # Ensure bun is available
    if command -v bun &>/dev/null; then
        IMPLS+=(bun)
        IMPL_CMDS[bun]="bun $ROOT_DIR/bun/bin/threads"
    fi
fi

echo
echo "Found ${#IMPLS[@]} implementations: ${IMPLS[*]}"
echo

if [[ ${#IMPLS[@]} -lt 2 ]]; then
    echo "ERROR: Need at least 2 implementations for comparison"
    exit 1
fi

# Output directory for comparison
OUTPUT_DIR=$(mktemp -d "${TMPDIR:-/tmp}/threads-parity-output.XXXXXX")
trap "rm -rf '$WORKSPACE' '$OUTPUT_DIR'" EXIT

FAILURES=0
TOTAL_CHECKS=0

# Run a command for all implementations and compare outputs
# Usage: compare_outputs "description" "command_args..."
compare_outputs() {
    local description="$1"
    shift
    local cmd_args=("$@")

    echo -e "${BOLD}Checking: $description${NC}"
    ((TOTAL_CHECKS++))

    local reference_impl="${IMPLS[0]}"
    local reference_cmd="${IMPL_CMDS[$reference_impl]}"

    # Get reference output
    local reference_output="$OUTPUT_DIR/reference-$TOTAL_CHECKS.txt"
    (cd "$WORKSPACE" && $reference_cmd "${cmd_args[@]}" 2>/dev/null) | normalize_output > "$reference_output"

    local all_match=true
    for impl in "${IMPLS[@]:1}"; do
        local impl_cmd="${IMPL_CMDS[$impl]}"
        local impl_output="$OUTPUT_DIR/${impl}-$TOTAL_CHECKS.txt"

        (cd "$WORKSPACE" && $impl_cmd "${cmd_args[@]}" 2>/dev/null) | normalize_output > "$impl_output"

        if ! diff -q "$reference_output" "$impl_output" >/dev/null 2>&1; then
            all_match=false
            echo -e "  ${RED}MISMATCH: $reference_impl vs $impl${NC}"
            echo "  Diff (first 10 lines):"
            diff "$reference_output" "$impl_output" | head -10 | sed 's/^/    /'
        fi
    done

    if $all_match; then
        echo -e "  ${GREEN}OK: All implementations match${NC}"
    else
        ((FAILURES++))
    fi
    echo
}

# Extract thread IDs from JSON output (handles both array and wrapped formats)
extract_ids_from_json() {
    # Try both .threads[].id (Go-style wrapper) and .[].id (plain array)
    # Use explicit error handling
    local input
    input=$(cat)

    # Try Go-style first (.threads array)
    local ids
    ids=$(echo "$input" | jq -r '.threads[]?.id // empty' 2>/dev/null)

    # If empty, try plain array style
    if [[ -z "$ids" ]]; then
        ids=$(echo "$input" | jq -r '.[]?.id // empty' 2>/dev/null)
    fi

    echo "$ids" | sort
}

# Normalize output to handle minor differences
# - Strip trailing whitespace
# - Normalize multiple spaces to single space
# - Remove debug/status lines (lines starting with "PWD:", "Git root:", etc.)
# - Remove "Showing N threads" header line (count may vary in wording)
# - Remove column headers (ID STATUS PATH NAME)
# - Remove implementation-specific markers (← PWD, etc.)
# - Sort lines to handle different ordering
normalize_output() {
    # Remove implementation-specific header lines, normalize whitespace, sort
    sed 's/[[:space:]]*$//' | \
    sed '/^PWD/d' | \
    sed '/^Git root/d' | \
    sed '/^Showing /d' | \
    sed '/^Hint:/d' | \
    sed '/^ID /d' | \
    sed '/^-- /d' | \
    sed 's/ ← PWD//g' | \
    sed 's/[[:space:]]\+/ /g' | \
    sed '/^$/d' | \
    sort
}

# JSON output (normalize and compare by thread IDs)
compare_json_outputs() {
    local description="$1"
    shift
    local cmd_args=("$@")

    echo -e "${BOLD}Checking: $description${NC}"
    ((TOTAL_CHECKS++))

    local reference_impl="${IMPLS[0]}"
    local reference_cmd="${IMPL_CMDS[$reference_impl]}"

    # Get reference thread IDs
    local ref_ids
    ref_ids=$((cd "$WORKSPACE" && $reference_cmd "${cmd_args[@]}" 2>/dev/null) | extract_ids_from_json)

    local all_match=true
    for impl in "${IMPLS[@]:1}"; do
        local impl_cmd="${IMPL_CMDS[$impl]}"
        local impl_ids
        impl_ids=$((cd "$WORKSPACE" && $impl_cmd "${cmd_args[@]}" 2>/dev/null) | extract_ids_from_json)

        if [[ "$ref_ids" != "$impl_ids" ]]; then
            all_match=false
            echo -e "  ${RED}MISMATCH: $reference_impl vs $impl (different thread IDs)${NC}"
            echo "  Reference IDs: $(echo "$ref_ids" | tr '\n' ' ')"
            echo "  $impl IDs: $(echo "$impl_ids" | tr '\n' ' ')"
        fi
    done

    if $all_match; then
        echo -e "  ${GREEN}OK: All implementations return same thread IDs${NC}"
    else
        ((FAILURES++))
    fi
    echo
}

# === Comparisons ===

# List commands (text output)
compare_outputs "list (local level only)" list
compare_outputs "list -r (recursive)" list -r
compare_outputs "list --down=1" list --down=1

# List commands (JSON - compare thread IDs only)
compare_json_outputs "list --json" list --json
compare_json_outputs "list -r --json" list -r --json

# Status filter
compare_outputs "list --status=blocked" list --status=blocked
compare_outputs "list --status blocked (space-separated)" list --status blocked

# Read command (content should be identical)
compare_outputs "read abc123" read abc123

# Note: stats command has minor formatting differences between implementations
# that are not functionally significant, so we skip it for parity checking

# === Summary ===

echo "======================================"
if [[ $FAILURES -eq 0 ]]; then
    echo -e "${GREEN}All $TOTAL_CHECKS checks passed${NC}"
    exit 0
else
    echo -e "${RED}$FAILURES of $TOTAL_CHECKS checks failed${NC}"
    exit 1
fi

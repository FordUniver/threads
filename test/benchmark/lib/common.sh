#!/usr/bin/env bash
# Common utilities for benchmark suite
# Sourced by other benchmark scripts

# Paths
BENCH_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REPO_DIR="$(dirname "$(dirname "$BENCH_DIR")")"
RESULTS_DIR="$BENCH_DIR/../results"

# Implementation definitions
declare -A IMPL_PATHS
declare -A IMPL_CMDS

# Shell (baseline)
IMPL_PATHS["shell"]="$REPO_DIR/shell/threads"
IMPL_CMDS["shell"]="$REPO_DIR/shell/threads"

# Go (compiled)
IMPL_PATHS["go"]="$REPO_DIR/go/threads"
IMPL_CMDS["go"]="$REPO_DIR/go/threads"

# Python (via uv)
IMPL_PATHS["python"]="$REPO_DIR/python/src/threads"
IMPL_CMDS["python"]="uv run --quiet --directory $REPO_DIR/python python -m threads"

# Perl (with lib path)
IMPL_PATHS["perl"]="$REPO_DIR/perl/bin/threads"
IMPL_CMDS["perl"]="perl -I$REPO_DIR/perl/lib $REPO_DIR/perl/bin/threads"

# Rust (compiled)
IMPL_PATHS["rust"]="$REPO_DIR/rust/target/release/threads"
IMPL_CMDS["rust"]="$REPO_DIR/rust/target/release/threads"

# Swift (compiled) - use standard release path (works across architectures)
IMPL_PATHS["swift"]="$REPO_DIR/swift/.build/release/threads"
IMPL_CMDS["swift"]="$REPO_DIR/swift/.build/release/threads"

# Ruby (interpreted)
IMPL_PATHS["ruby"]="$REPO_DIR/ruby/bin/threads"
IMPL_CMDS["ruby"]="$REPO_DIR/ruby/bin/threads"

# Bun (TypeScript)
IMPL_PATHS["bun"]="$REPO_DIR/bun/bin/threads"
IMPL_CMDS["bun"]="$REPO_DIR/bun/bin/threads"

# Default implementations to test
DEFAULT_IMPLS="shell go python perl rust swift ruby bun"

# Verify implementations exist and work
# Args: impl names (space-separated) or empty for all
# Returns: valid impl names (space-separated)
verify_impls() {
    local requested="${1:-$DEFAULT_IMPLS}"
    local valid=""

    for name in $requested; do
        local path="${IMPL_PATHS[$name]}"
        local cmd="${IMPL_CMDS[$name]}"

        if [[ -z "$path" ]]; then
            echo "Warning: Unknown implementation '$name'" >&2
            continue
        fi

        if [[ ! -e "$path" ]]; then
            echo "Warning: $name not found at $path" >&2
            continue
        fi

        # Verify command works (check for output, not exit code - some frameworks exit 1 on help)
        local help_output
        help_output=$($cmd --help 2>&1 || true)
        if [[ -z "$help_output" ]]; then
            echo "Warning: $name failed --help test (no output)" >&2
            continue
        fi

        valid="$valid $name"
    done

    echo "${valid# }"
}

# Get command for implementation
get_impl_cmd() {
    local name="$1"
    echo "${IMPL_CMDS[$name]}"
}

# Get sorted list of implementations (for consistent output order)
sorted_impls() {
    local impls="$1"
    echo "$impls" | tr ' ' '\n' | sort | tr '\n' ' ' | sed 's/ $//'
}

# Create results directory with timestamp
create_results_dir() {
    local timestamp
    timestamp=$(date +%Y%m%d_%H%M%S)
    local dir="$RESULTS_DIR/$timestamp"
    mkdir -p "$dir"/{raw,csv,reports}
    echo "$dir"
}

# Print section header
print_header() {
    local title="$1"
    echo ""
    echo "========================================"
    echo "$title"
    echo "========================================"
    echo ""
}

# Print subsection header
print_subheader() {
    local title="$1"
    echo "## $title"
    echo ""
}

# Debug logging (only if BENCH_DEBUG is set)
debug() {
    [[ -n "${BENCH_DEBUG:-}" ]] && echo "[DEBUG] $*" >&2
}

#!/usr/bin/env bash
# Benchmark script for comparing threads implementations
# Usage: ./benchmark.sh [iterations]

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_DIR="$(dirname "$SCRIPT_DIR")"
ITERATIONS="${1:-100}"

# Implementations to test (name -> command)
# Note: Some need special invocation (uv run, perl with lib path)
declare -A IMPLS
declare -A IMPL_CMDS

# Shell (baseline)
IMPLS["shell"]="$REPO_DIR/shell/threads"
IMPL_CMDS["shell"]="$REPO_DIR/shell/threads"

# Go (compiled)
IMPLS["go"]="$REPO_DIR/go/threads"
IMPL_CMDS["go"]="$REPO_DIR/go/threads"

# Python (via uv)
IMPLS["python"]="$REPO_DIR/python/src/threads"
IMPL_CMDS["python"]="uv run --quiet --directory $REPO_DIR/python python -m threads"

# Perl (with lib path)
IMPLS["perl"]="$REPO_DIR/perl/bin/threads"
IMPL_CMDS["perl"]="perl -I$REPO_DIR/perl/lib $REPO_DIR/perl/bin/threads"

# Check for hyperfine
if command -v hyperfine &>/dev/null; then
    USE_HYPERFINE=true
else
    USE_HYPERFINE=false
    echo "Note: Install hyperfine for better benchmarks"
    echo ""
fi

echo "threads CLI Benchmark"
echo "====================="
echo "Iterations: $ITERATIONS"
echo ""

# Verify implementations exist
for name in "${!IMPLS[@]}"; do
    path="${IMPLS[$name]}"
    cmd="${IMPL_CMDS[$name]}"
    # Check if the main file/directory exists
    if [[ ! -e "$path" ]]; then
        echo "Warning: $name implementation not found at $path"
        unset IMPLS[$name]
        unset IMPL_CMDS[$name]
        continue
    fi
    # Verify command actually works
    if ! $cmd --help >/dev/null 2>&1; then
        echo "Warning: $name implementation failed --help test"
        unset IMPLS[$name]
        unset IMPL_CMDS[$name]
    fi
done

if [[ ${#IMPLS[@]} -eq 0 ]]; then
    echo "No implementations to benchmark"
    exit 1
fi

# Create temp workspace for benchmarks
TEST_WS=$(mktemp -d)
mkdir -p "$TEST_WS/.threads"
for i in {1..50}; do
    id=$(printf "%06x" $i)
    cat > "$TEST_WS/.threads/${id}-thread-${i}.md" << EOF
---
id: $id
name: Test Thread $i
status: active
---

## Log
EOF
done

export WORKSPACE="$TEST_WS"
trap "rm -rf '$TEST_WS'" EXIT

echo "Test workspace: $TEST_WS (50 threads)"
echo ""

# Benchmark functions
benchmark_command() {
    local name="$1"
    local cmd="$2"
    local args="$3"

    if $USE_HYPERFINE; then
        hyperfine --warmup 3 --min-runs "$ITERATIONS" \
            --export-json "/tmp/bench_${name}.json" \
            "$cmd $args" 2>&1 | grep -E "Time|Range"
    else
        # Simple timing loop
        # First check if command works at all
        local test_output
        test_output=$($cmd $args 2>&1)
        local test_exit=$?
        if [[ $test_exit -ne 0 ]]; then
            echo "  FAILED (exit code $test_exit)"
            echo "  Output: ${test_output:0:200}"
            return 1
        fi
        local total=0
        for ((i=0; i<ITERATIONS; i++)); do
            local start=$(gdate +%s%N 2>/dev/null || date +%s%N)
            $cmd $args >/dev/null 2>&1
            local end=$(gdate +%s%N 2>/dev/null || date +%s%N)
            total=$((total + end - start))
        done
        local avg=$((total / ITERATIONS / 1000000))  # Convert to ms
        echo "  Mean: ${avg}ms"
    fi
}

# Helper to create N threads
create_threads() {
    local count="$1"
    local dir="${2:-.threads}"
    mkdir -p "$TEST_WS/$dir"
    for ((i=1; i<=count; i++)); do
        local id=$(printf "%06x" $i)
        cat > "$TEST_WS/$dir/${id}-thread-${i}.md" << EOF
---
id: $id
name: Test Thread $i
status: active
---

## Body

## Todo

## Log
EOF
    done
}

# Run benchmarks (sorted for consistent output)
for name in $(echo "${!IMPLS[@]}" | tr ' ' '\n' | sort); do
    cmd="${IMPL_CMDS[$name]}"
    echo "## $name"
    echo ""

    echo "### --help (cold start)"
    benchmark_command "${name}_help" "$cmd" "--help"
    echo ""

    echo "### list (50 threads)"
    benchmark_command "${name}_list" "$cmd" "list"
    echo ""

    echo "### list -r (recursive)"
    benchmark_command "${name}_list_r" "$cmd" "list -r"
    echo ""

    echo "### read (single thread)"
    benchmark_command "${name}_read" "$cmd" "read 000001"
    echo ""

    echo "### status (change status)"
    # Toggle between active/blocked to avoid no-op
    benchmark_command "${name}_status" "$cmd" "status 000001 blocked"
    $cmd status 000001 active >/dev/null 2>&1  # Reset
    echo ""
done

# Scale tests
echo "========================================"
echo "Scale Tests"
echo "========================================"
echo ""

# Create larger thread sets
echo "Creating 200 threads..."
rm -rf "$TEST_WS/.threads"
create_threads 200
echo ""

echo "## list with 200 threads"
for name in $(echo "${!IMPLS[@]}" | tr ' ' '\n' | sort); do
    cmd="${IMPL_CMDS[$name]}"
    echo "### $name"
    benchmark_command "${name}_list_200" "$cmd" "list"
    echo ""
done

echo "Creating 500 threads..."
rm -rf "$TEST_WS/.threads"
create_threads 500
echo ""

echo "## list with 500 threads"
for name in $(echo "${!IMPLS[@]}" | tr ' ' '\n' | sort); do
    cmd="${IMPL_CMDS[$name]}"
    echo "### $name"
    benchmark_command "${name}_list_500" "$cmd" "list"
    echo ""
done

# Summary comparison if hyperfine was used
if $USE_HYPERFINE && [[ ${#IMPLS[@]} -gt 1 ]]; then
    echo "## Direct Comparison (list command)"
    echo ""

    cmds=()
    names=()
    for name in $(echo "${!IMPLS[@]}" | tr ' ' '\n' | sort); do
        cmds+=("${IMPL_CMDS[$name]} list")
        names+=("$name")
    done

    # Build hyperfine command with names
    hf_args=(--warmup 3)
    for i in "${!cmds[@]}"; do
        hf_args+=(--command-name "${names[$i]}" "${cmds[$i]}")
    done

    hyperfine "${hf_args[@]}"
fi

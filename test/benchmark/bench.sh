#!/usr/bin/env bash
# Benchmark threads (Go implementation)
# Usage: ./bench.sh [iterations] [workspace_size]
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(dirname "$(dirname "$SCRIPT_DIR")")"

ITERATIONS=${1:-5}
WORKSPACE_SIZE=${2:-3000}
WORKSPACE="/tmp/threads-benchmark-workspace"

echo "threads Benchmark"
echo "======================================================"
echo "Iterations: $ITERATIONS"
echo "Workspace size: $WORKSPACE_SIZE threads"
echo

# Generate workspace if needed or if size changed
if [[ ! -d "$WORKSPACE" ]] || [[ ! -f "$WORKSPACE/.bench-size" ]] || [[ "$(cat "$WORKSPACE/.bench-size")" != "$WORKSPACE_SIZE" ]]; then
    "$SCRIPT_DIR/generate-workspace.sh" "$WORKSPACE_SIZE" "$WORKSPACE"
    echo "$WORKSPACE_SIZE" > "$WORKSPACE/.bench-size"
    echo
fi

# Build Go
THREADS_BIN="$ROOT_DIR/go/threads-bench"
if [[ -f "$ROOT_DIR/go/go.mod" ]] && command -v go &>/dev/null; then
    echo "Building Go..."
    if ! (cd "$ROOT_DIR/go" && go build -o threads-bench ./cmd/threads); then
        echo "ERROR: Go build failed" >&2
        exit 1
    fi
else
    echo "ERROR: Go implementation or go command not found" >&2
    exit 1
fi

echo "Using: $THREADS_BIN"
echo

# CSV output
CSV_FILE="$SCRIPT_DIR/benchmark-results.csv"
echo "operation,iterations,total_ms,avg_ms" > "$CSV_FILE"

# Timing function using perl for sub-ms precision
get_time_ms() {
    perl -MTime::HiRes=time -e 'printf "%.3f", time * 1000'
}

# Benchmark a single operation
# Args: operation [args...]
# Returns: average ms per call
bench_operation() {
    local op="$1"
    shift
    local args=("$@")

    local total_ms=0

    # Warmup (1 run)
    (cd "$WORKSPACE" && "$THREADS_BIN" "${args[@]}") >/dev/null 2>&1 || true

    # Timed runs
    local start end elapsed
    start=$(get_time_ms)
    for ((i = 0; i < ITERATIONS; i++)); do
        (cd "$WORKSPACE" && "$THREADS_BIN" "${args[@]}") >/dev/null 2>&1 || true
    done
    end=$(get_time_ms)

    total_ms=$(echo "$end - $start" | bc)
    local avg_ms=$(echo "scale=1; $total_ms / $ITERATIONS" | bc)

    # CSV
    echo "$op,$ITERATIONS,$total_ms,$avg_ms" >> "$CSV_FILE"

    echo "$avg_ms"
}

# Pick a random thread ID from the workspace
get_random_thread_id() {
    local file
    file=$(find "$WORKSPACE" -name "*.md" -path "*/.threads/*" | shuf -n1)
    basename "$file" .md | cut -d- -f1
}

# Store results for table output
declare -A RESULTS

# Run benchmarks
echo "Running benchmarks..."
echo

# Operations to benchmark
OPERATIONS=("list -r" "validate -r" "read" "path")

for op in "${OPERATIONS[@]}"; do
    echo "=== $op ==="

    # Build args
    declare -a op_args
    case "$op" in
        "list -r")
            op_args=(list -r)
            ;;
        "validate -r")
            op_args=(validate -r)
            ;;
        "read")
            thread_id=$(get_random_thread_id)
            op_args=(read "$thread_id")
            ;;
        "path")
            thread_id=$(get_random_thread_id)
            op_args=(path "$thread_id")
            ;;
    esac

    avg=$(bench_operation "$op" "${op_args[@]}")
    RESULTS["$op"]="$avg"
    printf "  %8s ms/call\n" "$avg"
    echo
done

# Print summary table
echo "======================================================"
echo "Summary (ms/call, lower is better)"
echo "======================================================"
printf "%-14s %10s\n" "Operation" "Time"
printf "%-14s %10s\n" "---------" "----"

for op in "${OPERATIONS[@]}"; do
    val="${RESULTS[$op]:-N/A}"
    printf "%-14s %10s\n" "$op" "${val}ms"
done

echo
echo "======================================================"
echo "Results saved to: $CSV_FILE"
echo
echo "Notes:"
echo "  - 'list -r' and 'validate -r' scan all $WORKSPACE_SIZE threads"
echo "  - 'read' and 'path' operate on a single random thread"

#!/usr/bin/env bash
# Benchmark threads implementations
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

# Find available implementations
declare -a IMPLS=()
declare -A IMPL_PATHS=()

# Track build failures
BUILD_FAILED=0

# Go (required if go is available)
if [[ -f "$ROOT_DIR/go/go.mod" ]] && command -v go &>/dev/null; then
    echo "Building Go..."
    if (cd "$ROOT_DIR/go" && go build -o threads-bench ./cmd/threads); then
        IMPLS+=(go) && IMPL_PATHS[go]="$ROOT_DIR/go/threads-bench"
    else
        echo "ERROR: Go build failed" >&2
        BUILD_FAILED=1
    fi
fi

# Rust (required if cargo available)
if [[ -f "$ROOT_DIR/rust/Cargo.toml" ]] && command -v cargo &>/dev/null; then
    echo "Building Rust..."
    if (cd "$ROOT_DIR/rust" && cargo build --release --quiet); then
        IMPLS+=(rust) && IMPL_PATHS[rust]="$ROOT_DIR/rust/target/release/threads"
    else
        echo "ERROR: Rust build failed" >&2
        BUILD_FAILED=1
    fi
fi

# Swift (required if swift available)
if [[ -f "$ROOT_DIR/swift/Package.swift" ]] && command -v swift &>/dev/null; then
    echo "Building Swift..."
    if (cd "$ROOT_DIR/swift" && swift build -c release --quiet); then
        IMPLS+=(swift) && IMPL_PATHS[swift]="$ROOT_DIR/swift/.build/release/threads"
    else
        echo "ERROR: Swift build failed" >&2
        BUILD_FAILED=1
    fi
fi

# Python (required if uv available)
if [[ -f "$ROOT_DIR/python/bin/threads" ]] && command -v uv &>/dev/null; then
    IMPLS+=(python) && IMPL_PATHS[python]="$ROOT_DIR/python/bin/threads"
elif [[ -f "$ROOT_DIR/python/bin/threads" ]]; then
    echo "ERROR: Python requires uv but not found" >&2
    BUILD_FAILED=1
fi

# Ruby (required)
if [[ -f "$ROOT_DIR/ruby/bin/threads" ]]; then
    IMPLS+=(ruby) && IMPL_PATHS[ruby]="$ROOT_DIR/ruby/bin/threads"
else
    echo "ERROR: Ruby implementation not found" >&2
    BUILD_FAILED=1
fi

# Perl (required)
if [[ -f "$ROOT_DIR/perl/bin/threads" ]]; then
    IMPLS+=(perl) && IMPL_PATHS[perl]="$ROOT_DIR/perl/bin/threads"
else
    echo "ERROR: Perl implementation not found" >&2
    BUILD_FAILED=1
fi

# Bun (required)
if [[ -f "$ROOT_DIR/bun/bin/threads" ]]; then
    IMPLS+=(bun) && IMPL_PATHS[bun]="$ROOT_DIR/bun/bin/threads"
else
    echo "ERROR: Bun implementation not found" >&2
    BUILD_FAILED=1
fi

# Exit if any required build failed
if [[ $BUILD_FAILED -ne 0 ]]; then
    echo "Build failures detected, aborting benchmark" >&2
    exit 1
fi

if [[ ${#IMPLS[@]} -eq 0 ]]; then
    echo "ERROR: No implementations found!"
    exit 1
fi

echo "Found implementations: ${IMPLS[*]}"
echo

# === Correctness validation ===
# Ensure all implementations find the same threads
echo "Validating correctness across implementations..."
VALIDATION_DIR="/tmp/threads-bench-validation"
mkdir -p "$VALIDATION_DIR"

REFERENCE_IMPL="${IMPLS[0]}"
REFERENCE_PATH="${IMPL_PATHS[$REFERENCE_IMPL]}"

# Get reference output (thread IDs only, sorted)
WORKSPACE="$WORKSPACE" "$REFERENCE_PATH" list -r --json 2>/dev/null | jq -r '.threads[].id' | sort > "$VALIDATION_DIR/reference.txt"
REFERENCE_COUNT=$(wc -l < "$VALIDATION_DIR/reference.txt" | tr -d ' ')
echo "  Reference ($REFERENCE_IMPL): $REFERENCE_COUNT threads"

VALIDATION_FAILED=0
for impl in "${IMPLS[@]}"; do
    if [[ "$impl" == "$REFERENCE_IMPL" ]]; then
        continue
    fi
    impl_path="${IMPL_PATHS[$impl]}"
    WORKSPACE="$WORKSPACE" "$impl_path" list -r --json 2>/dev/null | jq -r '.threads[].id' | sort > "$VALIDATION_DIR/${impl}.txt"
    impl_count=$(wc -l < "$VALIDATION_DIR/${impl}.txt" | tr -d ' ')

    if ! diff -q "$VALIDATION_DIR/reference.txt" "$VALIDATION_DIR/${impl}.txt" >/dev/null 2>&1; then
        echo "  ERROR: $impl found $impl_count threads (differs from reference)"
        diff "$VALIDATION_DIR/reference.txt" "$VALIDATION_DIR/${impl}.txt" | head -10
        VALIDATION_FAILED=1
    else
        echo "  OK: $impl ($impl_count threads)"
    fi
done

if [[ $VALIDATION_FAILED -ne 0 ]]; then
    echo "Validation failed! Implementations disagree on thread list." >&2
    exit 1
fi
echo "All implementations agree on $REFERENCE_COUNT threads."
echo

# CSV output
CSV_FILE="$SCRIPT_DIR/benchmark-results.csv"
echo "implementation,operation,iterations,total_ms,avg_ms" > "$CSV_FILE"

# Timing function using perl for sub-ms precision
get_time_ms() {
    perl -MTime::HiRes=time -e 'printf "%.3f", time * 1000'
}

# Benchmark a single operation
# Args: impl operation [args...]
# Returns: average ms per call
bench_operation() {
    local impl="$1"
    local op="$2"
    shift 2
    local args=("$@")

    local impl_path="${IMPL_PATHS[$impl]}"
    local total_ms=0

    # Warmup (1 run)
    WORKSPACE="$WORKSPACE" "$impl_path" "${args[@]}" >/dev/null 2>&1 || true

    # Timed runs
    local start end elapsed
    start=$(get_time_ms)
    for ((i = 0; i < ITERATIONS; i++)); do
        WORKSPACE="$WORKSPACE" "$impl_path" "${args[@]}" >/dev/null 2>&1 || true
    done
    end=$(get_time_ms)

    total_ms=$(echo "$end - $start" | bc)
    local avg_ms=$(echo "scale=1; $total_ms / $ITERATIONS" | bc)

    # CSV
    echo "$impl,$op,$ITERATIONS,$total_ms,$avg_ms" >> "$CSV_FILE"

    echo "$avg_ms"
}

# Shuffle array (Fisher-Yates)
shuffle_array() {
    local -n arr=$1
    local i j tmp
    for ((i = ${#arr[@]} - 1; i > 0; i--)); do
        j=$((RANDOM % (i + 1)))
        tmp="${arr[i]}"
        arr[i]="${arr[j]}"
        arr[j]="$tmp"
    done
}

# Pick a random thread ID from the workspace
get_random_thread_id() {
    local file
    file=$(find "$WORKSPACE" -name "*.md" -path "*/.threads/*" | shuf -n1)
    basename "$file" .md | cut -d- -f1
}

# Store results for table output
declare -A RESULTS

# Run benchmarks in randomized order per operation
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

    # Randomize implementation order
    shuffled_impls=("${IMPLS[@]}")
    shuffle_array shuffled_impls

    for impl in "${shuffled_impls[@]}"; do
        avg=$(bench_operation "$impl" "$op" "${op_args[@]}")
        RESULTS["$impl,$op"]="$avg"
        printf "  %-8s %8s ms/call\n" "$impl:" "$avg"
    done
    echo
done

# Print summary table
echo "======================================================"
echo "Summary (ms/call, lower is better)"
echo "======================================================"
printf "%-10s" "Impl"
for op in "${OPERATIONS[@]}"; do
    printf "%14s" "$op"
done
echo
printf "%-10s" "----"
for op in "${OPERATIONS[@]}"; do
    printf "%14s" "----------"
done
echo

for impl in "${IMPLS[@]}"; do
    printf "%-10s" "$impl"
    for op in "${OPERATIONS[@]}"; do
        val="${RESULTS[$impl,$op]:-N/A}"
        printf "%14s" "${val}ms"
    done
    echo
done

echo
echo "======================================================"
echo "Results saved to: $CSV_FILE"
echo
echo "Notes:"
echo "  - Compiled (go, rust, swift) typically have better startup"
echo "  - Interpreted (python, ruby, perl, bun) may be competitive at scale"
echo "  - 'list -r' and 'validate -r' scan all $WORKSPACE_SIZE threads"

#!/usr/bin/env bash
# Test workspace generation for benchmarks
# Creates reproducible thread data for consistent benchmarking

# Status distribution for realistic workloads
STATUSES=("active" "idea" "blocked" "planning" "paused")

# Generate N threads with deterministic IDs
# Args: count, directory, seed (optional)
generate_threads() {
    local count="$1"
    local dir="$2"
    local seed="${3:-42}"

    mkdir -p "$dir/.threads"

    for ((i=1; i<=count; i++)); do
        # Deterministic ID from seed + index
        local id
        id=$(printf "%06x" $((seed + i)))

        # Rotate through statuses
        local status="${STATUSES[$((i % ${#STATUSES[@]}))]}"

        # Create thread file
        cat > "$dir/.threads/${id}-benchmark-thread-${i}.md" << EOF
---
id: $id
name: Benchmark Thread $i
desc: Generated for benchmarking with seed $seed
status: $status
---

## Body

This is thread $i of $count generated threads.
Content for benchmarking purposes.

## Notes

- Initial note for thread $i  <!-- ${id}01 -->

## Todo

- [ ] Task $i.1  <!-- ${id}11 -->
- [ ] Task $i.2  <!-- ${id}12 -->

## Log

### 2026-01-01

- **10:00** Created for benchmarking.
EOF
    done
}

# Generate nested workspace structure
# Args: base_dir, ws_threads, cat_threads, proj_threads
generate_nested() {
    local base="$1"
    local ws_threads="${2:-10}"
    local cat_threads="${3:-10}"
    local proj_threads="${4:-10}"

    # Workspace level
    generate_threads "$ws_threads" "$base" 1000

    # Category level (cat1, cat2)
    local cat_seed=2000
    for cat in cat1 cat2; do
        mkdir -p "$base/$cat"
        generate_threads "$cat_threads" "$base/$cat" "$cat_seed"
        cat_seed=$((cat_seed + 100))

        # Project level (proj1, proj2 under each cat)
        local proj_seed=$((cat_seed + 500))
        for proj in proj1 proj2; do
            mkdir -p "$base/$cat/$proj"
            generate_threads "$proj_threads" "$base/$cat/$proj" "$proj_seed"
            proj_seed=$((proj_seed + 100))
        done
    done
}

# Create a temporary benchmark workspace
# Args: thread_count (optional, default 50)
# Returns: workspace path (also exported as BENCH_WORKSPACE)
create_bench_workspace() {
    local count="${1:-50}"

    local ws
    ws=$(mktemp -d)
    generate_threads "$count" "$ws"

    export WORKSPACE="$ws"
    export BENCH_WORKSPACE="$ws"
    echo "$ws"
}

# Create nested temporary workspace
# Args: ws_threads, cat_threads, proj_threads
create_nested_bench_workspace() {
    local ws_threads="${1:-10}"
    local cat_threads="${2:-10}"
    local proj_threads="${3:-10}"

    local ws
    ws=$(mktemp -d)
    generate_nested "$ws" "$ws_threads" "$cat_threads" "$proj_threads"

    export WORKSPACE="$ws"
    export BENCH_WORKSPACE="$ws"
    echo "$ws"
}

# Clean up workspace
cleanup_bench_workspace() {
    if [[ -n "${BENCH_WORKSPACE:-}" && -d "$BENCH_WORKSPACE" ]]; then
        rm -rf "$BENCH_WORKSPACE"
        unset BENCH_WORKSPACE
        unset WORKSPACE
    fi
}

# Get first thread ID from workspace
# Useful for single-thread operations
# Args: workspace (required)
get_first_thread_id() {
    local ws="${1:-${BENCH_WORKSPACE:-}}"
    [[ -z "$ws" ]] && { echo "Error: workspace required" >&2; return 1; }
    local first_file
    first_file=$(find "$ws/.threads" -name "*.md" -type f 2>/dev/null | head -1)
    if [[ -n "$first_file" ]]; then
        basename "$first_file" | cut -c1-6
    fi
}

# Reset thread to known state (for mutation benchmarks)
# Args: thread_id, workspace
reset_thread() {
    local id="$1"
    local ws="${2:-${BENCH_WORKSPACE:-}}"
    [[ -z "$ws" ]] && { echo "Error: workspace required" >&2; return 1; }

    local file
    file=$(find "$ws/.threads" -name "${id}-*.md" -type f 2>/dev/null | head -1)
    [[ -z "$file" ]] && return 1

    cat > "$file" << EOF
---
id: $id
name: Benchmark Thread Reset
desc: Reset for mutation benchmark
status: active
---

## Body

Reset content.

## Notes

## Todo

## Log
EOF
}

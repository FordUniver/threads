#!/usr/bin/env bash
# Timing utilities for benchmark suite
# Supports hyperfine (preferred) with fallback to manual timing
#
# Dependencies:
# - gdate (GNU coreutils) or date with nanosecond support for timing
# - bc (basic calculator) for stddev calculation - without bc, stddev defaults to 0
# - jq for parsing hyperfine JSON output (optional, for hyperfine mode)
# - hyperfine (optional, falls back to manual timing)

# Check if hyperfine is available
has_hyperfine() {
    command -v hyperfine &>/dev/null
}

# Check if bc is available (used for stddev)
has_bc() {
    command -v bc &>/dev/null
}

# Warn about missing bc once
_BC_WARNING_SHOWN=""
warn_missing_bc() {
    if [[ -z "$_BC_WARNING_SHOWN" ]] && ! has_bc; then
        echo "Warning: bc not available, stddev will be reported as 0" >&2
        _BC_WARNING_SHOWN=1
    fi
}

# Get current time in nanoseconds (cross-platform)
# Priority: gdate (macOS with coreutils), date (Linux), python fallback
now_ns() {
    # GNU date (gdate on macOS, date on Linux)
    gdate +%s%N 2>/dev/null && return
    date +%s%N 2>/dev/null && return
    # Fallback: use Python for sub-second precision
    python3 -c 'import time; print(int(time.time() * 1e9))' 2>/dev/null && return
    # Last resort: seconds only (multiply by 1e9 for ns)
    echo "$(($(date +%s) * 1000000000))"
}

# Run benchmark with hyperfine
# Args: name, command, iterations, warmup, json_output_file
# Note: Commands run from BENCH_WORKSPACE directory (required for threads CLI)
bench_hyperfine() {
    local name="$1"
    local cmd="$2"
    local iterations="${3:-100}"
    local warmup="${4:-3}"
    local json_file="${5:-}"
    local ws="${BENCH_WORKSPACE:-$(pwd)}"

    local hf_args=(
        --warmup "$warmup"
        --min-runs "$iterations"
        --command-name "$name"
    )

    if [[ -n "$json_file" ]]; then
        hf_args+=(--export-json "$json_file")
    fi

    # Run from workspace directory
    hyperfine "${hf_args[@]}" "cd '$ws' && $cmd" 2>&1
}

# Run benchmark with manual timing
# Args: name, command, iterations
# Output: JSON-like result to stdout
# Note: Commands run from BENCH_WORKSPACE directory (required for threads CLI)
bench_manual() {
    local name="$1"
    local cmd="$2"
    local iterations="${3:-100}"
    local ws="${BENCH_WORKSPACE:-$(pwd)}"

    # Verify command works (run from workspace)
    if ! (cd "$ws" && $cmd) >/dev/null 2>&1; then
        echo "FAILED: $name (command error)" >&2
        return 1
    fi

    local total_ns=0
    local min_ns=999999999999
    local max_ns=0
    local times=()

    for ((i=0; i<iterations; i++)); do
        local start end elapsed
        start=$(now_ns)
        (cd "$ws" && $cmd) >/dev/null 2>&1
        end=$(now_ns)
        elapsed=$((end - start))

        times+=("$elapsed")
        total_ns=$((total_ns + elapsed))
        ((elapsed < min_ns)) && min_ns=$elapsed
        ((elapsed > max_ns)) && max_ns=$elapsed
    done

    local mean_ns=$((total_ns / iterations))
    local mean_ms=$((mean_ns / 1000000))
    local min_ms=$((min_ns / 1000000))
    local max_ms=$((max_ns / 1000000))

    # Calculate stddev
    local sum_sq=0
    for t in "${times[@]}"; do
        local diff=$((t - mean_ns))
        sum_sq=$((sum_sq + diff * diff))
    done
    local variance=$((sum_sq / iterations))
    # Integer sqrt approximation (requires bc)
    warn_missing_bc
    local stddev_ns
    stddev_ns=$(echo "sqrt($variance)" | bc 2>/dev/null || echo "0")
    local stddev_ms=$((stddev_ns / 1000000))

    # Output in parseable format
    echo "  $name: ${mean_ms}ms (min: ${min_ms}ms, max: ${max_ms}ms, stddev: ${stddev_ms}ms)"

    # Return values for collection
    export BENCH_MEAN_MS="$mean_ms"
    export BENCH_MIN_MS="$min_ms"
    export BENCH_MAX_MS="$max_ms"
    export BENCH_STDDEV_MS="$stddev_ms"
}

# Run a single benchmark (auto-selects hyperfine or manual)
# Args: name, command, iterations, warmup, json_file (optional)
# Exports: BENCH_MEAN_MS, BENCH_MIN_MS, BENCH_MAX_MS, BENCH_STDDEV_MS
run_bench() {
    local name="$1"
    local cmd="$2"
    local iterations="${3:-100}"
    local warmup="${4:-3}"
    local json_file="${5:-}"

    if has_hyperfine; then
        bench_hyperfine "$name" "$cmd" "$iterations" "$warmup" "$json_file"
        # Parse hyperfine JSON for metrics if available
        if [[ -n "$json_file" && -f "$json_file" ]]; then
            BENCH_MEAN_MS=$(jq -r '.results[0].mean * 1000 | floor' "$json_file" 2>/dev/null || echo "0")
            BENCH_MIN_MS=$(jq -r '.results[0].min * 1000 | floor' "$json_file" 2>/dev/null || echo "0")
            BENCH_MAX_MS=$(jq -r '.results[0].max * 1000 | floor' "$json_file" 2>/dev/null || echo "0")
            BENCH_STDDEV_MS=$(jq -r '.results[0].stddev * 1000 | floor' "$json_file" 2>/dev/null || echo "0")
            export BENCH_MEAN_MS BENCH_MIN_MS BENCH_MAX_MS BENCH_STDDEV_MS
        fi
    else
        bench_manual "$name" "$cmd" "$iterations"
    fi
}

# Compare multiple implementations on same command
# Args: command_suffix (e.g., "list"), iterations, warmup
# Uses IMPLS array (must be set by caller)
run_comparison() {
    local cmd_suffix="$1"
    local iterations="${2:-100}"
    local warmup="${3:-3}"

    if has_hyperfine; then
        local hf_args=(--warmup "$warmup" --min-runs "$iterations")
        local cmds=()
        local names=()

        for impl in $IMPLS; do
            local base_cmd="${IMPL_CMDS[$impl]}"
            cmds+=("$base_cmd $cmd_suffix")
            names+=("$impl")
        done

        # Build hyperfine command with named commands
        for i in "${!cmds[@]}"; do
            hf_args+=(--command-name "${names[$i]}" "${cmds[$i]}")
        done

        hyperfine "${hf_args[@]}"
    else
        for impl in $IMPLS; do
            local base_cmd="${IMPL_CMDS[$impl]}"
            bench_manual "$impl" "$base_cmd $cmd_suffix" "$iterations"
        done
    fi
}

# Memory profiling (macOS only)
# Args: command (as single string)
# Returns: peak RSS in KB
measure_memory() {
    local cmd="$1"

    if [[ "$(uname)" == "Darwin" ]]; then
        /usr/bin/time -l bash -c "$cmd" 2>&1 | awk '/maximum resident set size/ {print $1}'
    else
        # Linux
        /usr/bin/time -v bash -c "$cmd" 2>&1 | awk '/Maximum resident set size/ {print $6}'
    fi
}

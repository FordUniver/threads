#!/usr/bin/env bash
# Mutation benchmarks - measures write operations
# Tests: new, body, note, todo, log, status

set -euo pipefail

CATEGORY_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$CATEGORY_DIR/../lib/common.sh"
source "$CATEGORY_DIR/../lib/timing.sh"
source "$CATEGORY_DIR/../lib/workspace.sh"
source "$CATEGORY_DIR/../lib/output.sh"

# Run a single mutation benchmark with reset between runs
# Args: impl, name, setup_cmd, bench_cmd, iterations
run_mutation_bench() {
    local impl="$1"
    local name="$2"
    local setup_cmd="$3"
    local bench_cmd="$4"
    local iterations="${5:-50}"

    local total_ns=0
    local min_ns=999999999999
    local max_ns=0
    local times=()

    for ((i=0; i<iterations; i++)); do
        # Run setup if provided
        [[ -n "$setup_cmd" ]] && eval "$setup_cmd" >/dev/null 2>&1

        local start end elapsed
        start=$(gdate +%s%N 2>/dev/null || date +%s%N)
        eval "$bench_cmd" >/dev/null 2>&1
        end=$(gdate +%s%N 2>/dev/null || date +%s%N)
        elapsed=$((end - start))

        times+=("$elapsed")
        total_ns=$((total_ns + elapsed))
        ((elapsed < min_ns)) && min_ns=$elapsed
        ((elapsed > max_ns)) && max_ns=$elapsed
    done

    local mean_ns=$((total_ns / iterations))
    BENCH_MEAN_MS=$((mean_ns / 1000000))
    BENCH_MIN_MS=$((min_ns / 1000000))
    BENCH_MAX_MS=$((max_ns / 1000000))

    # Calculate stddev
    local sum_sq=0
    for t in "${times[@]}"; do
        local diff=$((t - mean_ns))
        sum_sq=$((sum_sq + diff * diff))
    done
    local variance=$((sum_sq / iterations))
    local stddev_ns
    stddev_ns=$(echo "sqrt($variance)" | bc 2>/dev/null || echo "0")
    BENCH_STDDEV_MS=$((stddev_ns / 1000000))

    echo "  $name: ${BENCH_MEAN_MS}ms (min: ${BENCH_MIN_MS}ms, max: ${BENCH_MAX_MS}ms)"
}

# Run mutation benchmarks
# Args: implementations, iterations
run_mutation_benchmarks() {
    local impls="${1:-$DEFAULT_IMPLS}"
    local iterations="${2:-50}"

    print_header "Mutation Benchmarks"
    echo "Iterations: $iterations"
    echo "Note: Lower iteration count due to state management overhead"
    echo ""

    # Create workspace with some existing threads
    local ws
    ws=$(create_bench_workspace 10)
    export BENCH_WORKSPACE="$ws"
    export WORKSPACE="$ws"
    # shellcheck disable=SC2064
    trap "rm -rf '$ws'" RETURN

    local first_id
    first_id=$(get_first_thread_id "$ws")

    # new benchmark (creates new thread each run, cleanup after)
    print_subheader "new (create thread)"
    for impl in $(sorted_impls "$impls"); do
        local cmd
        cmd=$(get_impl_cmd "$impl")
        echo "### $impl"

        # Each iteration creates a thread, then we clean up extras
        run_mutation_bench "$impl" "new" "" \
            "$cmd new 'Benchmark Thread' --desc='Created for benchmark'" \
            "$iterations"

        add_result "$impl" "mutation" "new" 0 \
            "${BENCH_MEAN_MS:-0}" "${BENCH_STDDEV_MS:-0}" \
            "${BENCH_MIN_MS:-0}" "${BENCH_MAX_MS:-0}"

        # Clean up created threads (keep only first 10)
        find "$ws/.threads" -name "*.md" | tail -n +11 | xargs rm -f 2>/dev/null || true
        echo ""
    done

    # status benchmark (toggle between states)
    print_subheader "status (change status)"
    for impl in $(sorted_impls "$impls"); do
        local cmd
        cmd=$(get_impl_cmd "$impl")
        echo "### $impl"

        run_mutation_bench "$impl" "status" \
            "$cmd status $first_id active" \
            "$cmd status $first_id blocked" \
            "$iterations"

        add_result "$impl" "mutation" "status" 1 \
            "${BENCH_MEAN_MS:-0}" "${BENCH_STDDEV_MS:-0}" \
            "${BENCH_MIN_MS:-0}" "${BENCH_MAX_MS:-0}"

        # Reset to active
        $cmd status "$first_id" active >/dev/null 2>&1
        echo ""
    done

    # body --set benchmark
    print_subheader "body --set"
    for impl in $(sorted_impls "$impls"); do
        local cmd
        cmd=$(get_impl_cmd "$impl")
        echo "### $impl"

        run_mutation_bench "$impl" "body-set" \
            "" \
            "echo 'New body content for benchmark' | $cmd body $first_id --set" \
            "$iterations"

        add_result "$impl" "mutation" "body-set" 1 \
            "${BENCH_MEAN_MS:-0}" "${BENCH_STDDEV_MS:-0}" \
            "${BENCH_MIN_MS:-0}" "${BENCH_MAX_MS:-0}"
        echo ""
    done

    # note add benchmark
    print_subheader "note add"
    for impl in $(sorted_impls "$impls"); do
        local cmd
        cmd=$(get_impl_cmd "$impl")
        echo "### $impl"

        run_mutation_bench "$impl" "note-add" \
            "" \
            "$cmd note $first_id add 'Benchmark note entry'" \
            "$iterations"

        add_result "$impl" "mutation" "note-add" 1 \
            "${BENCH_MEAN_MS:-0}" "${BENCH_STDDEV_MS:-0}" \
            "${BENCH_MIN_MS:-0}" "${BENCH_MAX_MS:-0}"

        # Reset thread to clean state
        reset_thread "$first_id" "$ws"
        echo ""
    done

    # todo add benchmark
    print_subheader "todo add"
    for impl in $(sorted_impls "$impls"); do
        local cmd
        cmd=$(get_impl_cmd "$impl")
        echo "### $impl"

        run_mutation_bench "$impl" "todo-add" \
            "" \
            "$cmd todo $first_id add 'Benchmark todo item'" \
            "$iterations"

        add_result "$impl" "mutation" "todo-add" 1 \
            "${BENCH_MEAN_MS:-0}" "${BENCH_STDDEV_MS:-0}" \
            "${BENCH_MIN_MS:-0}" "${BENCH_MAX_MS:-0}"

        # Reset thread
        reset_thread "$first_id"
        echo ""
    done

    # log benchmark
    print_subheader "log (add entry)"
    for impl in $(sorted_impls "$impls"); do
        local cmd
        cmd=$(get_impl_cmd "$impl")
        echo "### $impl"

        run_mutation_bench "$impl" "log" \
            "" \
            "$cmd log $first_id 'Benchmark log entry'" \
            "$iterations"

        add_result "$impl" "mutation" "log" 1 \
            "${BENCH_MEAN_MS:-0}" "${BENCH_STDDEV_MS:-0}" \
            "${BENCH_MIN_MS:-0}" "${BENCH_MAX_MS:-0}"

        # Reset thread
        reset_thread "$first_id"
        echo ""
    done
}

# Run if executed directly
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    impls=$(verify_impls "${1:-}")
    iterations="${2:-50}"

    if [[ -z "$impls" ]]; then
        echo "No valid implementations to benchmark" >&2
        exit 1
    fi

    run_mutation_benchmarks "$impls" "$iterations"
fi

#!/usr/bin/env bash
# Query benchmarks - measures read-only operations
# Tests: list, list -r, list --search, list --status, read, stats, validate

set -euo pipefail

CATEGORY_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$CATEGORY_DIR/../lib/common.sh"
source "$CATEGORY_DIR/../lib/timing.sh"
source "$CATEGORY_DIR/../lib/workspace.sh"
source "$CATEGORY_DIR/../lib/output.sh"

# Run query benchmarks
# Args: implementations, iterations, thread_count
run_query_benchmarks() {
    local impls="${1:-$DEFAULT_IMPLS}"
    local iterations="${2:-100}"
    local thread_count="${3:-200}"
    local warmup="${4:-3}"

    print_header "Query Benchmarks ($thread_count threads)"
    echo "Iterations: $iterations, Warmup: $warmup"
    echo ""

    # Create workspace with threads
    local ws
    ws=$(create_bench_workspace "$thread_count")
    export BENCH_WORKSPACE="$ws"
    export WORKSPACE="$ws"
    # shellcheck disable=SC2064
    trap "rm -rf '$ws'" RETURN

    local first_id
    first_id=$(get_first_thread_id "$ws")

    # list benchmark
    print_subheader "list"
    for impl in $(sorted_impls "$impls"); do
        local cmd
        cmd=$(get_impl_cmd "$impl")
        echo "### $impl"
        run_bench "${impl}_list" "$cmd list" "$iterations" "$warmup"
        add_result "$impl" "query" "list" "$thread_count" \
            "${BENCH_MEAN_MS:-0}" "${BENCH_STDDEV_MS:-0}" \
            "${BENCH_MIN_MS:-0}" "${BENCH_MAX_MS:-0}"
        echo ""
    done

    # read benchmark (single thread)
    print_subheader "read (single thread)"
    for impl in $(sorted_impls "$impls"); do
        local cmd
        cmd=$(get_impl_cmd "$impl")
        echo "### $impl"
        run_bench "${impl}_read" "$cmd read $first_id" "$iterations" "$warmup"
        add_result "$impl" "query" "read" 1 \
            "${BENCH_MEAN_MS:-0}" "${BENCH_STDDEV_MS:-0}" \
            "${BENCH_MIN_MS:-0}" "${BENCH_MAX_MS:-0}"
        echo ""
    done

    # stats benchmark
    print_subheader "stats"
    for impl in $(sorted_impls "$impls"); do
        local cmd
        cmd=$(get_impl_cmd "$impl")
        echo "### $impl"
        run_bench "${impl}_stats" "$cmd stats" "$iterations" "$warmup"
        add_result "$impl" "query" "stats" "$thread_count" \
            "${BENCH_MEAN_MS:-0}" "${BENCH_STDDEV_MS:-0}" \
            "${BENCH_MIN_MS:-0}" "${BENCH_MAX_MS:-0}"
        echo ""
    done

    # validate benchmark
    print_subheader "validate"
    for impl in $(sorted_impls "$impls"); do
        local cmd
        cmd=$(get_impl_cmd "$impl")
        echo "### $impl"
        run_bench "${impl}_validate" "$cmd validate" "$iterations" "$warmup"
        add_result "$impl" "query" "validate" "$thread_count" \
            "${BENCH_MEAN_MS:-0}" "${BENCH_STDDEV_MS:-0}" \
            "${BENCH_MIN_MS:-0}" "${BENCH_MAX_MS:-0}"
        echo ""
    done

    # list with --search filter
    print_subheader "list --search"
    for impl in $(sorted_impls "$impls"); do
        local cmd
        cmd=$(get_impl_cmd "$impl")
        echo "### $impl"
        run_bench "${impl}_list_search" "$cmd list --search=Thread" "$iterations" "$warmup"
        add_result "$impl" "query" "list-search" "$thread_count" \
            "${BENCH_MEAN_MS:-0}" "${BENCH_STDDEV_MS:-0}" \
            "${BENCH_MIN_MS:-0}" "${BENCH_MAX_MS:-0}"
        echo ""
    done

    # list with --status filter
    print_subheader "list --status=active"
    for impl in $(sorted_impls "$impls"); do
        local cmd
        cmd=$(get_impl_cmd "$impl")
        echo "### $impl"
        run_bench "${impl}_list_status" "$cmd list --status=active" "$iterations" "$warmup"
        add_result "$impl" "query" "list-status" "$thread_count" \
            "${BENCH_MEAN_MS:-0}" "${BENCH_STDDEV_MS:-0}" \
            "${BENCH_MIN_MS:-0}" "${BENCH_MAX_MS:-0}"
        echo ""
    done
}

# Run recursive query benchmarks (requires nested workspace)
run_recursive_benchmarks() {
    local impls="${1:-$DEFAULT_IMPLS}"
    local iterations="${2:-100}"
    local warmup="${3:-3}"

    print_header "Recursive Query Benchmarks"
    echo "Iterations: $iterations"
    echo ""

    # Create nested workspace (10 ws + 2 cats * 10 + 4 projs * 10 = 70 threads)
    local ws
    ws=$(create_nested_bench_workspace 10 10 10)
    export BENCH_WORKSPACE="$ws"
    export WORKSPACE="$ws"
    local total_threads=70
    # shellcheck disable=SC2064
    trap "rm -rf '$ws'" RETURN

    # list -r benchmark
    print_subheader "list -r (recursive)"
    for impl in $(sorted_impls "$impls"); do
        local cmd
        cmd=$(get_impl_cmd "$impl")
        echo "### $impl"
        run_bench "${impl}_list_r" "$cmd list -r" "$iterations" "$warmup"
        add_result "$impl" "query" "list-r" "$total_threads" \
            "${BENCH_MEAN_MS:-0}" "${BENCH_STDDEV_MS:-0}" \
            "${BENCH_MIN_MS:-0}" "${BENCH_MAX_MS:-0}"
        echo ""
    done

    # stats -r benchmark
    print_subheader "stats -r (recursive)"
    for impl in $(sorted_impls "$impls"); do
        local cmd
        cmd=$(get_impl_cmd "$impl")
        echo "### $impl"
        run_bench "${impl}_stats_r" "$cmd stats -r" "$iterations" "$warmup"
        add_result "$impl" "query" "stats-r" "$total_threads" \
            "${BENCH_MEAN_MS:-0}" "${BENCH_STDDEV_MS:-0}" \
            "${BENCH_MIN_MS:-0}" "${BENCH_MAX_MS:-0}"
        echo ""
    done
}

# Run if executed directly
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    impls=$(verify_impls "${1:-}")
    iterations="${2:-100}"
    thread_count="${3:-200}"

    if [[ -z "$impls" ]]; then
        echo "No valid implementations to benchmark" >&2
        exit 1
    fi

    run_query_benchmarks "$impls" "$iterations" "$thread_count"
    run_recursive_benchmarks "$impls" "$iterations"
fi

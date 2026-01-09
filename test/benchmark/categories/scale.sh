#!/usr/bin/env bash
# Scale benchmarks - measures performance at different thread counts
# Tests how implementations scale with increasing data

set -euo pipefail

CATEGORY_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$CATEGORY_DIR/../lib/common.sh"
source "$CATEGORY_DIR/../lib/timing.sh"
source "$CATEGORY_DIR/../lib/workspace.sh"
source "$CATEGORY_DIR/../lib/output.sh"

# Default scale points
DEFAULT_SCALE_POINTS="10 50 100 200 500 1000"

# Run scale benchmarks
# Args: implementations, iterations, scale_points
run_scale_benchmarks() {
    local impls="${1:-$DEFAULT_IMPLS}"
    local iterations="${2:-50}"
    local scale_points="${3:-$DEFAULT_SCALE_POINTS}"
    local warmup="${4:-3}"

    print_header "Scale Benchmarks"
    echo "Iterations per point: $iterations"
    echo "Scale points: $scale_points"
    echo ""

    for count in $scale_points; do
        print_subheader "list ($count threads)"

        # Create workspace with specified thread count
        local ws
        ws=$(create_bench_workspace "$count")
        export BENCH_WORKSPACE="$ws"
        export WORKSPACE="$ws"

        for impl in $(sorted_impls "$impls"); do
            local cmd
            cmd=$(get_impl_cmd "$impl")
            echo "### $impl"

            if has_hyperfine; then
                run_bench "${impl}_list_${count}" "$cmd list" "$iterations" "$warmup"
            else
                run_bench "${impl}_list_${count}" "$cmd list" "$iterations"
            fi

            add_result "$impl" "scale" "list" "$count" \
                "${BENCH_MEAN_MS:-0}" "${BENCH_STDDEV_MS:-0}" \
                "${BENCH_MIN_MS:-0}" "${BENCH_MAX_MS:-0}"
            echo ""
        done

        rm -rf "$ws"
        unset BENCH_WORKSPACE WORKSPACE
    done

    # Stats scaling (separate loop to avoid workspace churn)
    for count in $scale_points; do
        print_subheader "stats ($count threads)"

        local ws
        ws=$(create_bench_workspace "$count")
        export BENCH_WORKSPACE="$ws"
        export WORKSPACE="$ws"

        for impl in $(sorted_impls "$impls"); do
            local cmd
            cmd=$(get_impl_cmd "$impl")
            echo "### $impl"

            if has_hyperfine; then
                run_bench "${impl}_stats_${count}" "$cmd stats" "$iterations" "$warmup"
            else
                run_bench "${impl}_stats_${count}" "$cmd stats" "$iterations"
            fi

            add_result "$impl" "scale" "stats" "$count" \
                "${BENCH_MEAN_MS:-0}" "${BENCH_STDDEV_MS:-0}" \
                "${BENCH_MIN_MS:-0}" "${BENCH_MAX_MS:-0}"
            echo ""
        done

        rm -rf "$ws"
        unset BENCH_WORKSPACE WORKSPACE
    done
}

# Print scale summary (throughput analysis)
print_scale_summary() {
    local impls="$1"
    local operation="$2"

    echo ""
    echo "### $operation Throughput Summary"
    echo ""
    echo "| Threads | $(echo "$impls" | tr ' ' '\n' | tr '\n' '|' | sed 's/|$//') |"
    echo "|---------|$(echo "$impls" | tr ' ' '\n' | while read -r i; do echo -n "--------|"; done)"

    # Collect results by thread count
    local counts
    counts=$(for r in "${BENCH_RESULTS[@]}"; do
        IFS='|' read -r impl cat bench threads mean stddev min max <<< "$r"
        [[ "$cat" == "scale" && "$bench" == "$operation" ]] && echo "$threads"
    done | sort -nu)

    for count in $counts; do
        printf "| %7d |" "$count"
        for impl in $impls; do
            for r in "${BENCH_RESULTS[@]}"; do
                IFS='|' read -r i cat bench threads mean stddev min max <<< "$r"
                if [[ "$cat" == "scale" && "$bench" == "$operation" && "$threads" == "$count" && "$i" == "$impl" ]]; then
                    printf " %6sms |" "$mean"
                fi
            done
        done
        echo ""
    done
    echo ""
}

# Run if executed directly
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    impls=$(verify_impls "${1:-}")
    iterations="${2:-50}"
    scale_points="${3:-$DEFAULT_SCALE_POINTS}"

    if [[ -z "$impls" ]]; then
        echo "No valid implementations to benchmark" >&2
        exit 1
    fi

    run_scale_benchmarks "$impls" "$iterations" "$scale_points"

    # Print throughput summary
    print_scale_summary "$impls" "list"
    print_scale_summary "$impls" "stats"
fi

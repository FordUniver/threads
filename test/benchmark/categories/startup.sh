#!/usr/bin/env bash
# Startup benchmarks - measures cold start overhead
# Tests: --help, empty list

set -euo pipefail

CATEGORY_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$CATEGORY_DIR/../lib/common.sh"
source "$CATEGORY_DIR/../lib/timing.sh"
source "$CATEGORY_DIR/../lib/workspace.sh"
source "$CATEGORY_DIR/../lib/output.sh"

# Run startup benchmarks
# Args: implementations (space-separated), iterations
run_startup_benchmarks() {
    local impls="${1:-$DEFAULT_IMPLS}"
    local iterations="${2:-100}"
    local warmup="${3:-3}"

    print_header "Startup Benchmarks"
    echo "Iterations: $iterations, Warmup: $warmup"
    echo ""

    # Create empty workspace for consistent environment
    local ws
    ws=$(mktemp -d)
    mkdir -p "$ws/.threads"
    export WORKSPACE="$ws"
    # shellcheck disable=SC2064
    trap "rm -rf '$ws'" RETURN

    # --help benchmark (pure startup + help text)
    print_subheader "--help (cold start)"
    for impl in $(sorted_impls "$impls"); do
        local cmd
        cmd=$(get_impl_cmd "$impl")
        echo "### $impl"

        if has_hyperfine; then
            run_bench "${impl}_help" "$cmd --help" "$iterations" "$warmup"
        else
            run_bench "${impl}_help" "$cmd --help" "$iterations"
        fi

        add_result "$impl" "startup" "help" 0 \
            "${BENCH_MEAN_MS:-0}" "${BENCH_STDDEV_MS:-0}" \
            "${BENCH_MIN_MS:-0}" "${BENCH_MAX_MS:-0}"
        echo ""
    done

    # empty list benchmark (startup + workspace scan with 0 threads)
    print_subheader "list (empty workspace)"
    for impl in $(sorted_impls "$impls"); do
        local cmd
        cmd=$(get_impl_cmd "$impl")
        echo "### $impl"

        if has_hyperfine; then
            run_bench "${impl}_empty_list" "$cmd list" "$iterations" "$warmup"
        else
            run_bench "${impl}_empty_list" "$cmd list" "$iterations"
        fi

        add_result "$impl" "startup" "empty-list" 0 \
            "${BENCH_MEAN_MS:-0}" "${BENCH_STDDEV_MS:-0}" \
            "${BENCH_MIN_MS:-0}" "${BENCH_MAX_MS:-0}"
        echo ""
    done

    # Direct comparison if hyperfine available
    if has_hyperfine && [[ $(echo "$impls" | wc -w) -gt 1 ]]; then
        print_subheader "Direct Comparison (--help)"
        IMPLS="$impls"
        run_comparison "--help" "$iterations" "$warmup"
    fi
}

# Run if executed directly
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    impls=$(verify_impls "${1:-}")
    iterations="${2:-100}"

    if [[ -z "$impls" ]]; then
        echo "No valid implementations to benchmark" >&2
        exit 1
    fi

    run_startup_benchmarks "$impls" "$iterations"
fi

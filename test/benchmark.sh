#!/usr/bin/env bash
# Benchmark orchestrator for threads CLI implementations
# Usage: ./benchmark.sh [options]
#
# Options:
#   --quick           Use quick profile (5 min)
#   --full            Use full profile (30 min)
#   --category=NAME   Run specific category (startup|query|mutation|scale)
#   --impl=LIST       Comma-separated implementations (shell,go,python,perl)
#   --output=FORMAT   Output format (console|csv|json|markdown)
#   --iterations=N    Override iteration count
#   --help            Show this help

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BENCH_DIR="$SCRIPT_DIR/benchmark"

# Source library files
source "$BENCH_DIR/lib/common.sh"
source "$BENCH_DIR/lib/timing.sh"
source "$BENCH_DIR/lib/workspace.sh"
source "$BENCH_DIR/lib/output.sh"

# Source category files
source "$BENCH_DIR/categories/startup.sh"
source "$BENCH_DIR/categories/query.sh"
source "$BENCH_DIR/categories/mutation.sh"
source "$BENCH_DIR/categories/scale.sh"

# Defaults
PROFILE=""
CATEGORIES=""
IMPLS=""
OUTPUT_FORMAT="console"
ITERATIONS=""
WARMUP=""
RESULTS_SUBDIR=""

# Parse arguments
parse_args() {
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --quick)
                PROFILE="quick"
                shift
                ;;
            --full)
                PROFILE="full"
                shift
                ;;
            --category=*)
                CATEGORIES="${1#*=}"
                shift
                ;;
            --impl=*)
                IMPLS="${1#*=}"
                IMPLS="${IMPLS//,/ }"
                shift
                ;;
            --output=*)
                OUTPUT_FORMAT="${1#*=}"
                shift
                ;;
            --iterations=*)
                ITERATIONS="${1#*=}"
                shift
                ;;
            --help|-h)
                show_help
                exit 0
                ;;
            *)
                echo "Unknown option: $1" >&2
                show_help
                exit 1
                ;;
        esac
    done
}

show_help() {
    cat << 'EOF'
threads CLI Benchmark Suite

Usage: ./benchmark.sh [options]

Options:
  --quick           Use quick profile (~5 min)
  --full            Use full profile (~30 min)
  --category=NAME   Run specific category (startup|query|mutation|scale)
  --impl=LIST       Comma-separated implementations (shell,go,python,perl)
  --output=FORMAT   Output format (console|csv|json|markdown)
  --iterations=N    Override iteration count
  --help            Show this help

Examples:
  ./benchmark.sh --quick                    # Quick sanity check
  ./benchmark.sh --full --output=markdown   # Full run with markdown output
  ./benchmark.sh --category=startup         # Just startup benchmarks
  ./benchmark.sh --impl=go,shell            # Compare Go and Shell only
EOF
}

# Load profile settings
load_profile() {
    local profile="$1"
    local profile_file="$BENCH_DIR/profiles/${profile}.conf"

    if [[ -f "$profile_file" ]]; then
        # shellcheck source=/dev/null
        source "$profile_file"
    else
        echo "Warning: Profile '$profile' not found at $profile_file" >&2
    fi
}

# Main benchmark runner
main() {
    parse_args "$@"

    # Load profile if specified
    if [[ -n "$PROFILE" ]]; then
        load_profile "$PROFILE"
    fi

    # Apply defaults if not set by profile or args
    CATEGORIES="${CATEGORIES:-startup query}"
    ITERATIONS="${ITERATIONS:-100}"
    WARMUP="${WARMUP:-3}"
    QUERY_THREADS="${QUERY_THREADS:-200}"
    SCALE_POINTS="${SCALE_POINTS:-50 200 500}"
    MUTATION_ITERATIONS="${MUTATION_ITERATIONS:-50}"
    SKIP_MUTATION="${SKIP_MUTATION:-0}"

    # Verify implementations
    IMPLS=$(verify_impls "$IMPLS")
    if [[ -z "$IMPLS" ]]; then
        echo "Error: No valid implementations to benchmark" >&2
        exit 1
    fi

    # Print header
    echo "========================================"
    echo "threads CLI Benchmark Suite"
    echo "========================================"
    echo ""
    echo "Date: $(date)"
    echo "Profile: ${PROFILE:-custom}"
    echo "Implementations: $IMPLS"
    echo "Categories: $CATEGORIES"
    echo "Iterations: $ITERATIONS"
    echo "Hyperfine: $(has_hyperfine && echo "yes" || echo "no (using manual timing)")"
    echo ""

    # Clear previous results
    clear_results

    # Create results directory if outputting to file
    if [[ "$OUTPUT_FORMAT" != "console" ]]; then
        RESULTS_SUBDIR=$(create_results_dir)
        echo "Results directory: $RESULTS_SUBDIR"
        echo ""
    fi

    # Run requested categories
    for category in $CATEGORIES; do
        case "$category" in
            startup)
                run_startup_benchmarks "$IMPLS" "$ITERATIONS" "$WARMUP"
                ;;
            query)
                run_query_benchmarks "$IMPLS" "$ITERATIONS" "$QUERY_THREADS" "$WARMUP"
                run_recursive_benchmarks "$IMPLS" "$ITERATIONS" "$WARMUP"
                ;;
            mutation)
                if [[ "$SKIP_MUTATION" == "0" ]]; then
                    run_mutation_benchmarks "$IMPLS" "$MUTATION_ITERATIONS"
                else
                    echo "Skipping mutation benchmarks (SKIP_MUTATION=1)"
                    echo ""
                fi
                ;;
            scale)
                run_scale_benchmarks "$IMPLS" "$ITERATIONS" "$SCALE_POINTS" "$WARMUP"
                ;;
            *)
                echo "Unknown category: $category" >&2
                ;;
        esac
    done

    # Output results
    case "$OUTPUT_FORMAT" in
        csv)
            local csv_file="${RESULTS_SUBDIR}/benchmark.csv"
            output_csv "$csv_file"
            echo ""
            echo "CSV results written to: $csv_file"
            ;;
        json)
            local json_file="${RESULTS_SUBDIR}/benchmark.json"
            output_json "$json_file"
            echo ""
            echo "JSON results written to: $json_file"
            ;;
        markdown)
            local md_file="${RESULTS_SUBDIR}/benchmark.md"
            output_markdown "$md_file"
            echo ""
            echo "Markdown results written to: $md_file"
            cat "$md_file"
            ;;
        console|*)
            # Already printed to console during run
            echo ""
            echo "========================================"
            echo "Benchmark complete"
            echo "========================================"
            ;;
    esac
}

# Run main
main "$@"

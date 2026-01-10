#!/usr/bin/env bash
# Output formatters for benchmark results
# Supports CSV, JSON, and Markdown formats

# Global results array (populated during benchmark runs)
declare -a BENCH_RESULTS

# Add a result entry
# Args: impl, category, benchmark, threads, mean_ms, stddev_ms, min_ms, max_ms
add_result() {
    local impl="$1"
    local category="$2"
    local benchmark="$3"
    local threads="$4"
    local mean_ms="$5"
    local stddev_ms="${6:-0}"
    local min_ms="${7:-$mean_ms}"
    local max_ms="${8:-$mean_ms}"

    BENCH_RESULTS+=("$impl|$category|$benchmark|$threads|$mean_ms|$stddev_ms|$min_ms|$max_ms")
}

# Clear results
clear_results() {
    BENCH_RESULTS=()
}

# Output CSV format
output_csv() {
    local outfile="${1:-/dev/stdout}"
    {
        echo "implementation,category,benchmark,threads,mean_ms,stddev_ms,min_ms,max_ms"
        for result in "${BENCH_RESULTS[@]}"; do
            echo "$result" | tr '|' ','
        done
    } > "$outfile"
}

# Output JSON format
output_json() {
    local outfile="${1:-/dev/stdout}"
    local timestamp
    timestamp=$(date -u +%Y-%m-%dT%H:%M:%SZ)

    {
        echo "{"
        echo "  \"meta\": {"
        echo "    \"timestamp\": \"$timestamp\","
        echo "    \"host\": \"$(hostname)\","
        echo "    \"hyperfine\": $(has_hyperfine && echo "true" || echo "false")"
        echo "  },"
        echo "  \"results\": ["

        local first=true
        for result in "${BENCH_RESULTS[@]}"; do
            IFS='|' read -r impl category benchmark threads mean stddev min max <<< "$result"

            $first || echo ","
            first=false

            echo -n "    {"
            echo -n "\"implementation\":\"$impl\","
            echo -n "\"category\":\"$category\","
            echo -n "\"benchmark\":\"$benchmark\","
            echo -n "\"threads\":$threads,"
            echo -n "\"mean_ms\":$mean,"
            echo -n "\"stddev_ms\":$stddev,"
            echo -n "\"min_ms\":$min,"
            echo -n "\"max_ms\":$max"
            echo -n "}"
        done

        echo ""
        echo "  ]"
        echo "}"
    } > "$outfile"
}

# Output Markdown format with comparison tables
output_markdown() {
    local outfile="${1:-/dev/stdout}"
    local timestamp
    timestamp=$(date +%Y-%m-%d)

    {
        echo "# Benchmark Results"
        echo ""
        echo "Date: $timestamp"
        echo ""

        # Group by category and benchmark
        local current_category=""
        local current_benchmark=""
        local -a table_rows

        for result in "${BENCH_RESULTS[@]}"; do
            IFS='|' read -r impl category benchmark threads mean stddev min max <<< "$result"

            # New category?
            if [[ "$category" != "$current_category" ]]; then
                # Flush previous table if exists
                if [[ ${#table_rows[@]} -gt 0 ]]; then
                    print_md_table "${table_rows[@]}"
                    table_rows=()
                fi
                current_category="$category"
                current_benchmark=""
                echo "## $category"
                echo ""
            fi

            # New benchmark within category?
            if [[ "$benchmark" != "$current_benchmark" ]]; then
                if [[ ${#table_rows[@]} -gt 0 ]]; then
                    print_md_table "${table_rows[@]}"
                    table_rows=()
                fi
                current_benchmark="$benchmark"
                local thread_label=""
                [[ "$threads" != "0" && "$threads" != "1" ]] && thread_label=" ($threads threads)"
                echo "### $benchmark$thread_label"
                echo ""
            fi

            table_rows+=("$impl|$mean|$min|$max|$stddev")
        done

        # Flush final table
        if [[ ${#table_rows[@]} -gt 0 ]]; then
            print_md_table "${table_rows[@]}"
        fi
    } > "$outfile"
}

# Print markdown table from rows (impl|mean|min|max|stddev format)
print_md_table() {
    local rows=("$@")

    # Find fastest for relative comparison
    local fastest=999999999
    for row in "${rows[@]}"; do
        local mean
        mean=$(echo "$row" | cut -d'|' -f2)
        ((mean < fastest)) && fastest=$mean
    done

    echo "| Implementation | Mean (ms) | Min | Max | StdDev | Relative |"
    echo "|----------------|-----------|-----|-----|--------|----------|"

    for row in "${rows[@]}"; do
        IFS='|' read -r impl mean min max stddev <<< "$row"
        local relative
        if [[ $fastest -gt 0 ]]; then
            # Use awk for portable floating point division (bc may not be available)
            relative=$(gawk -v m="$mean" -v f="$fastest" 'BEGIN { printf "%.1fx", m/f }')
        else
            relative="1.0x"
        fi
        printf "| %-14s | %9s | %3s | %3s | %6s | %8s |\n" \
            "$impl" "$mean" "$min" "$max" "$stddev" "$relative"
    done
    echo ""
}

# Print summary comparison (sorted by mean time)
print_summary() {
    local category="$1"
    local benchmark="$2"

    echo "### $category / $benchmark"
    echo ""

    # Filter and sort results
    local -a filtered
    for result in "${BENCH_RESULTS[@]}"; do
        IFS='|' read -r impl cat bench threads mean stddev min max <<< "$result"
        [[ "$cat" == "$category" && "$bench" == "$benchmark" ]] && filtered+=("$result")
    done

    if [[ ${#filtered[@]} -eq 0 ]]; then
        echo "No results."
        echo ""
        return
    fi

    # Sort by mean time
    printf '%s\n' "${filtered[@]}" | sort -t'|' -k5 -n | while IFS='|' read -r impl cat bench threads mean stddev min max; do
        printf "  %-10s %8sms\n" "$impl:" "$mean"
    done
    echo ""
}

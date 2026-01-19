#!/usr/bin/env bash
# Generate synthetic workspace for benchmarking
# Usage: ./generate-workspace.sh [num_threads] [output_dir]
set -euo pipefail

NUM_THREADS=${1:-1000}
OUTPUT_DIR=${2:-/tmp/threads-benchmark-workspace}

# Structure: 10 categories, 10 projects each = 100 project dirs + 10 category dirs + 1 workspace
NUM_CATEGORIES=10
NUM_PROJECTS=10

echo "Generating benchmark workspace..."
echo "  Threads: $NUM_THREADS"
echo "  Output: $OUTPUT_DIR"

# Clean and create
rm -rf "$OUTPUT_DIR"
mkdir -p "$OUTPUT_DIR/.threads"

# Create category and project structure
for c in $(seq 1 $NUM_CATEGORIES); do
    cat_name="category-$(printf '%02d' "$c")"
    mkdir -p "$OUTPUT_DIR/$cat_name/.threads"

    for p in $(seq 1 $NUM_PROJECTS); do
        proj_name="project-$(printf '%02d' "$p")"
        mkdir -p "$OUTPUT_DIR/$cat_name/$proj_name/.threads"
    done
done

# Distribute threads across levels
# 10% workspace, 30% category, 60% project
ws_threads=$((NUM_THREADS / 10))
cat_threads=$((NUM_THREADS * 3 / 10))
proj_threads=$((NUM_THREADS - ws_threads - cat_threads))

echo "  Distribution: $ws_threads workspace, $cat_threads category, $proj_threads project"

# Status options
STATUSES=(idea planning active blocked paused resolved)

# Generate a thread file
generate_thread() {
    local id="$1"
    local dir="$2"
    local idx="$3"

    local status="${STATUSES[$((RANDOM % ${#STATUSES[@]}))]}"
    local hex_id=$(printf '%06x' "$idx")
    local slug="benchmark-thread-$idx"
    local filepath="$dir/${hex_id}-${slug}.md"
    local date=$(date +%Y-%m-%d)
    local time=$(date +%H:%M)

    cat > "$filepath" << EOF
---
id: $hex_id
name: Benchmark Thread $idx
desc: A synthetic thread for benchmarking purposes
status: $status
---

This is the body of benchmark thread $idx. It contains some text to make
the file more realistic in size. The thread management system needs to
parse this content when reading threads.

## Notes

- Note 1: Some observation about this thread
- Note 2: Another important detail
- Note 3: Follow-up item to consider

## Todo

- [ ] First task for this thread
- [ ] Second task that needs completion
- [x] Completed task example

## Log

### $date

- **$time** Created thread for benchmarking.
- **$time** Added initial content and structure.
EOF
}

# Generate workspace-level threads
echo -n "  Generating workspace threads..."
for i in $(seq 1 $ws_threads); do
    generate_thread "$i" "$OUTPUT_DIR/.threads" "$i"
done
echo " done"

# Generate category-level threads
echo -n "  Generating category threads..."
thread_idx=$((ws_threads + 1))
threads_per_cat=$((cat_threads / NUM_CATEGORIES))
for c in $(seq 1 $NUM_CATEGORIES); do
    cat_name="category-$(printf '%02d' "$c")"
    for i in $(seq 1 $threads_per_cat); do
        generate_thread "$thread_idx" "$OUTPUT_DIR/$cat_name/.threads" "$thread_idx"
        ((thread_idx++))
    done
done
echo " done"

# Generate project-level threads
echo -n "  Generating project threads..."
threads_per_proj=$((proj_threads / (NUM_CATEGORIES * NUM_PROJECTS)))
for c in $(seq 1 $NUM_CATEGORIES); do
    cat_name="category-$(printf '%02d' "$c")"
    for p in $(seq 1 $NUM_PROJECTS); do
        proj_name="project-$(printf '%02d' "$p")"
        for i in $(seq 1 $threads_per_proj); do
            generate_thread "$thread_idx" "$OUTPUT_DIR/$cat_name/$proj_name/.threads" "$thread_idx"
            ((thread_idx++))
        done
    done
done
echo " done"

# Count actual threads
actual_count=$(find "$OUTPUT_DIR" -name "*.md" -path "*/.threads/*" | wc -l)
echo
echo "Generated $actual_count threads in $OUTPUT_DIR"
echo "Workspace structure:"
echo "  - 1 workspace level (.threads/)"
echo "  - $NUM_CATEGORIES categories (category-NN/.threads/)"
echo "  - $((NUM_CATEGORIES * NUM_PROJECTS)) projects (category-NN/project-NN/.threads/)"

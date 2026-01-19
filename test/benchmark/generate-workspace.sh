#!/usr/bin/env bash
# Generate synthetic workspace for benchmarking
# Usage: ./generate-workspace.sh [num_threads] [output_dir]
#
# Creates a deterministic, realistic workspace with:
# - Uneven thread distribution (some dirs empty, some packed)
# - Variable thread sizes (100 bytes to 20KB+)
# - Deep nesting in some areas
# - Mix of statuses weighted toward active work
set -euo pipefail

NUM_THREADS=${1:-10000}
OUTPUT_DIR=${2:-/tmp/threads-benchmark-workspace}

echo "Generating benchmark workspace..."
echo "  Threads: $NUM_THREADS"
echo "  Output: $OUTPUT_DIR"

# Clean and create
rm -rf "$OUTPUT_DIR"
mkdir -p "$OUTPUT_DIR/.threads"

# Initialize git repo (required for Go implementation which uses git root)
git init -q "$OUTPUT_DIR"

# Deterministic pseudo-random based on index (inline for speed)
# Returns 0-99 based on input, deterministic
# Usage: result=$(( (n * 7919 + 104729) % 100 ))

# Structure: 15 categories with varying project counts
# Some categories are "hot" (many projects/threads), some "cold" (few/none)
declare -a CATEGORY_PROJECTS=(
    20  # category-01: large active area
    15  # category-02: medium
    8   # category-03: small
    0   # category-04: empty category (edge case)
    25  # category-05: very large
    3   # category-06: tiny
    12  # category-07: medium
    1   # category-08: single project
    18  # category-09: large
    5   # category-10: small
    0   # category-11: another empty
    10  # category-12: medium
    30  # category-13: largest - stress test
    2   # category-14: tiny
    7   # category-15: small
)
NUM_CATEGORIES=${#CATEGORY_PROJECTS[@]}

# Create directory structure including some deep nesting
echo -n "  Creating directory structure..."
for c in $(seq 1 $NUM_CATEGORIES); do
    cat_name="category-$(printf '%02d' "$c")"
    num_projects=${CATEGORY_PROJECTS[$((c-1))]}

    mkdir -p "$OUTPUT_DIR/$cat_name/.threads"

    for p in $(seq 1 $num_projects); do
        proj_name="project-$(printf '%02d' "$p")"
        mkdir -p "$OUTPUT_DIR/$cat_name/$proj_name/.threads"

        # Add deep nesting for some projects (deterministic: every 7th project in large categories)
        if (( num_projects > 10 && p % 7 == 0 )); then
            mkdir -p "$OUTPUT_DIR/$cat_name/$proj_name/submodule-a/.threads"
            mkdir -p "$OUTPUT_DIR/$cat_name/$proj_name/submodule-b/nested/.threads"
        fi
    done
done
echo " done"

# Count total .threads directories for distribution
TOTAL_DIRS=$(find "$OUTPUT_DIR" -type d -name ".threads" | wc -l)
echo "  Created $TOTAL_DIRS .threads directories"

# Status and size are computed inline in generate_thread for speed
# Status weights: 0-39: active (40%), 40-59: planning (20%), 60-74: idea (15%)
#                 75-84: blocked (10%), 85-94: paused (10%), 95-99: resolved (5%)
# Size weights: 0-59: small (60%), 60-84: medium (25%), 85-94: large (10%), 95-99: huge (5%)

# Generate repeated content block for large files
generate_log_entries() {
    local count=$1
    local base_date="2025-01-01"
    for i in $(seq 1 $count); do
        local day=$((i % 28 + 1))
        local month=$(( (i / 28) % 12 + 1 ))
        printf "### 2025-%02d-%02d\n\n" $month $day
        echo "- **09:00** Morning standup, discussed progress on this thread."
        echo "- **11:30** Deep work session, made significant progress."
        echo "- **14:00** Code review feedback incorporated."
        echo "- **16:45** Updated documentation and tests."
        echo ""
    done
}

# Generate a thread file with variable size
generate_thread() {
    local idx="$1"
    local dir="$2"

    # Inline pseudo-random and status/size calculation (avoid subshells)
    local r=$(( (idx * 7919 + 104729) % 100 ))
    local status
    if (( r < 40 )); then status="active"
    elif (( r < 60 )); then status="planning"
    elif (( r < 75 )); then status="idea"
    elif (( r < 85 )); then status="blocked"
    elif (( r < 95 )); then status="paused"
    else status="resolved"
    fi

    local r2=$(( ((idx * 3 + 17) * 7919 + 104729) % 100 ))
    local size_cat
    if (( r2 < 60 )); then size_cat="small"
    elif (( r2 < 85 )); then size_cat="medium"
    elif (( r2 < 95 )); then size_cat="large"
    else size_cat="huge"
    fi

    printf -v hex_id '%06x' "$idx"
    local slug="benchmark-thread-$idx"
    local filepath="$dir/${hex_id}-${slug}.md"

    # Base content
    cat > "$filepath" << EOF
---
id: $hex_id
name: Benchmark Thread $idx
desc: A synthetic thread for benchmarking purposes with deterministic content
status: $status
EOF

    # Add extra frontmatter for some threads
    if (( idx % 5 == 0 )); then
        echo "tags: [benchmark, test, synthetic]" >> "$filepath"
    fi
    if (( idx % 8 == 0 )); then
        echo "priority: $((idx % 5 + 1))" >> "$filepath"
    fi

    echo "---" >> "$filepath"
    echo "" >> "$filepath"

    # Body varies by size category
    case $size_cat in
        small)
            # ~200 bytes body
            cat >> "$filepath" << 'EOF'
Quick note for tracking purposes.

## Todo

- [ ] Complete this item
EOF
            ;;
        medium)
            # ~1KB body
            cat >> "$filepath" << 'EOF'
This thread tracks a medium-complexity task that requires some coordination
and has multiple components to consider. The implementation involves several
files and may require input from other team members.

## Context

The current system has some limitations that this work addresses. We need to
ensure backward compatibility while adding the new functionality.

## Notes

- Consider edge cases around empty inputs
- Performance should remain acceptable for large datasets
- Add appropriate test coverage

## Todo

- [ ] Design the interface
- [ ] Implement core logic
- [ ] Add unit tests
- [ ] Update documentation
- [x] Initial research complete

## Log

### 2025-01-15

- **10:00** Started investigation into requirements.
- **14:30** Drafted initial design document.
EOF
            ;;
        large)
            # ~5KB body
            cat >> "$filepath" << 'EOF'
This is a major thread tracking significant work that spans multiple sprints
and involves coordination across several teams. The scope includes architectural
changes, new feature development, and extensive testing requirements.

## Background

The existing implementation was designed for smaller scale and needs to be
rearchitected to handle the increased load we're seeing in production. This
involves both code changes and infrastructure updates.

## Requirements

1. Support 10x current throughput
2. Maintain backward compatibility with existing API
3. Add new endpoints for bulk operations
4. Improve error handling and observability
5. Update all dependent services

## Design Decisions

### Database Schema

We considered several approaches:
- Horizontal partitioning by tenant
- Vertical partitioning by access pattern
- Hybrid approach with caching layer

After analysis, the hybrid approach provides the best balance of complexity
and performance gains.

### API Changes

New endpoints:
- POST /api/v2/bulk - batch operations
- GET /api/v2/status - health and metrics
- WebSocket /api/v2/stream - real-time updates

## Notes

- Migration must be zero-downtime
- Feature flags for gradual rollout
- Monitoring dashboards need updates
- On-call runbook requires new sections
- Load testing in staging before production

## Todo

- [ ] Complete design document
- [ ] Review with architecture team
- [ ] Implement database migrations
- [ ] Build new API endpoints
- [ ] Add comprehensive tests
- [ ] Update client libraries
- [ ] Staging deployment
- [ ] Load testing
- [ ] Production rollout plan
- [ ] Documentation updates
- [x] Initial scoping
- [x] Team alignment

EOF
            generate_log_entries 10 >> "$filepath"
            ;;
        huge)
            # ~20KB body - stress test for parsing
            cat >> "$filepath" << 'EOF'
# Epic: Major Platform Overhaul

This epic-level thread tracks a comprehensive platform modernization effort
spanning multiple quarters. It encompasses infrastructure upgrades, code
modernization, and process improvements.

## Executive Summary

Our platform has grown organically over several years and accumulated
technical debt that impacts development velocity and system reliability.
This initiative addresses the most critical issues through a phased approach.

## Scope

### Phase 1: Foundation (Q1)
- Upgrade core dependencies
- Implement structured logging
- Add distributed tracing
- Improve CI/CD pipeline

### Phase 2: Architecture (Q2)
- Decompose monolith into services
- Implement event-driven patterns
- Add API gateway
- Set up service mesh

### Phase 3: Data (Q3)
- Database migration to new schema
- Implement data warehouse
- Add real-time analytics
- GDPR compliance updates

### Phase 4: Polish (Q4)
- Performance optimization
- Security hardening
- Documentation overhaul
- Team training

## Technical Details

### Current Architecture

```
┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│   Client    │────▶│   Monolith  │────▶│  Database   │
└─────────────┘     └─────────────┘     └─────────────┘
```

### Target Architecture

```
┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│   Client    │────▶│ API Gateway │────▶│   Service   │
└─────────────┘     └─────────────┘     │    Mesh     │
                                        └──────┬──────┘
                    ┌──────────────────────────┼──────────────────────────┐
                    │                          │                          │
              ┌─────▼─────┐            ┌───────▼───────┐          ┌───────▼───────┐
              │  Service  │            │    Service    │          │    Service    │
              │     A     │            │       B       │          │       C       │
              └─────┬─────┘            └───────┬───────┘          └───────┬───────┘
                    │                          │                          │
              ┌─────▼─────┐            ┌───────▼───────┐          ┌───────▼───────┐
              │    DB A   │            │     DB B      │          │     DB C      │
              └───────────┘            └───────────────┘          └───────────────┘
```

## Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Timeline slip | Medium | High | Buffer time, MVP focus |
| Team burnout | Medium | High | Sustainable pace, rotations |
| Integration issues | High | Medium | Extensive testing, feature flags |
| Data migration errors | Low | Critical | Backups, validation, rollback plan |

## Dependencies

- Infrastructure team capacity
- Security review bandwidth
- External vendor contracts
- Training budget approval

## Notes

- Weekly sync with stakeholders required
- Monthly executive updates
- Quarterly board presentation
- External audit in Q3
- Compliance certification renewal

EOF
            # Add extensive todo list
            echo "## Todo" >> "$filepath"
            echo "" >> "$filepath"
            for t in $(seq 1 50); do
                if (( t % 4 == 0 )); then
                    echo "- [x] Task $t: Completed milestone item" >> "$filepath"
                else
                    echo "- [ ] Task $t: Pending work item with description" >> "$filepath"
                fi
            done
            echo "" >> "$filepath"

            # Add extensive log
            generate_log_entries 60 >> "$filepath"
            ;;
    esac
}

# Collect all .threads directories
readarray -t THREAD_DIRS < <(find "$OUTPUT_DIR" -type d -name ".threads" | sort)

# Distribute threads with realistic unevenness
# Use deterministic weights based on directory depth and position
echo "  Distributing $NUM_THREADS threads across $TOTAL_DIRS directories..."

# Calculate weight for each directory (deeper = more threads, with variation)
declare -a DIR_WEIGHTS=()
total_weight=0
for i in "${!THREAD_DIRS[@]}"; do
    dir="${THREAD_DIRS[$i]}"
    depth=$(echo "$dir" | tr -cd '/' | wc -c)
    # Base weight increases with depth, plus deterministic variation (inline)
    variation=$(( ((i * 13) * 7919 + 104729) % 100 ))
    weight=$((depth * 10 + variation / 10))
    # Some directories get extra weight (hot spots)
    if (( i % 17 == 0 )); then
        weight=$((weight * 3))
    fi
    # Some directories are nearly empty
    if (( i % 23 == 0 )); then
        weight=1
    fi
    DIR_WEIGHTS+=($weight)
    total_weight=$((total_weight + weight))
done

# Generate threads according to weights
echo -n "  Generating threads"
thread_idx=1
threads_generated=0
for i in "${!THREAD_DIRS[@]}"; do
    dir="${THREAD_DIRS[$i]}"
    weight=${DIR_WEIGHTS[$i]}
    # Calculate threads for this directory
    dir_threads=$(( NUM_THREADS * weight / total_weight ))
    # Ensure at least some directories get threads
    if (( dir_threads == 0 && weight > 5 )); then
        dir_threads=1
    fi

    for j in $(seq 1 $dir_threads); do
        if (( threads_generated >= NUM_THREADS )); then
            break 2
        fi
        generate_thread $thread_idx "$dir"
        ((++thread_idx))
        ((++threads_generated))

        # Progress indicator every 1000 threads
        if (( threads_generated % 1000 == 0 )); then
            echo -n "."
        fi
    done
done

# If we haven't hit the target, add remaining to workspace root
remaining=$((NUM_THREADS - threads_generated))
if (( remaining > 0 )); then
    for j in $(seq 1 $remaining); do
        generate_thread $thread_idx "$OUTPUT_DIR/.threads"
        ((++thread_idx))
        ((++threads_generated))
    done
fi
echo " done"

# Summary statistics
echo ""
echo "Generated $threads_generated threads"
echo ""
echo "Directory structure:"
echo "  - $NUM_CATEGORIES categories"
total_projects=$(find "$OUTPUT_DIR" -type d -name "project-*" | wc -l)
echo "  - $total_projects projects"
echo "  - $TOTAL_DIRS .threads directories"
echo ""
echo "Thread size distribution:"
small=$(find "$OUTPUT_DIR" -name "*.md" -path "*/.threads/*" -size -500c | wc -l)
medium=$(find "$OUTPUT_DIR" -name "*.md" -path "*/.threads/*" -size +500c -size -3000c | wc -l)
large=$(find "$OUTPUT_DIR" -name "*.md" -path "*/.threads/*" -size +3000c -size -10000c | wc -l)
huge=$(find "$OUTPUT_DIR" -name "*.md" -path "*/.threads/*" -size +10000c | wc -l)
echo "  - Small (<500B):    $small"
echo "  - Medium (500B-3K): $medium"
echo "  - Large (3K-10K):   $large"
echo "  - Huge (>10K):      $huge"
echo ""
total_size=$(du -sh "$OUTPUT_DIR" | cut -f1)
echo "Total workspace size: $total_size"

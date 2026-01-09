#!/usr/bin/env bash
# Helper functions for thread manipulation in tests

# Create a thread file directly (bypasses CLI)
# Usage: create_thread "abc123" "Thread Name" "active" ["Optional description"]
# Creates file at TEST_WS/.threads/abc123-thread-name.md
create_thread() {
    local id="$1"
    local name="$2"
    local status="${3:-idea}"
    local desc="${4:-}"
    local path="${5:-$TEST_WS}"  # Optional: path for nested threads

    local slug
    slug=$(echo "$name" | tr '[:upper:]' '[:lower:]' | sed 's/[^a-z0-9]/-/g' | sed 's/--*/-/g' | sed 's/^-//;s/-$//')
    local filename="${id}-${slug}.md"
    local threads_dir="$path/.threads"

    mkdir -p "$threads_dir"

    cat > "$threads_dir/$filename" << EOF
---
id: $id
name: $name
desc: $desc
status: $status
---

## Body

## Notes

## Todo

## Log
EOF
}

# Create thread at category level
# Usage: create_thread_at_category "abc123" "Thread Name" "cat1" "active"
create_thread_at_category() {
    local id="$1"
    local name="$2"
    local category="$3"
    local status="${4:-idea}"
    local desc="${5:-}"

    create_thread "$id" "$name" "$status" "$desc" "$TEST_WS/$category"
}

# Create thread at project level
# Usage: create_thread_at_project "abc123" "Thread Name" "cat1" "proj1" "active"
create_thread_at_project() {
    local id="$1"
    local name="$2"
    local category="$3"
    local project="$4"
    local status="${5:-idea}"
    local desc="${6:-}"

    create_thread "$id" "$name" "$status" "$desc" "$TEST_WS/$category/$project"
}

# Get path to thread file by ID
# Usage: path=$(get_thread_path "abc123")
get_thread_path() {
    local id="$1"
    local search_path="${2:-$TEST_WS}"

    find "$search_path" -name "${id}-*.md" -type f 2>/dev/null | head -1
}

# Count threads at a path
# Usage: count=$(count_threads) or count=$(count_threads "$TEST_WS/cat1")
count_threads() {
    local path="${1:-$TEST_WS}"
    find "$path/.threads" -name "*.md" -type f 2>/dev/null | wc -l | tr -d ' '
}

# Count threads recursively
count_threads_recursive() {
    local path="${1:-$TEST_WS}"
    find "$path" -path "*/.threads/*.md" -type f 2>/dev/null | wc -l | tr -d ' '
}

# Read frontmatter field from thread file
# Usage: status=$(get_thread_field "abc123" "status")
get_thread_field() {
    local id="$1"
    local field="$2"
    local path
    path=$(get_thread_path "$id")

    if [[ -z "$path" ]]; then
        return 1
    fi

    # Simple YAML extraction (between --- markers)
    sed -n '/^---$/,/^---$/p' "$path" | grep "^${field}:" | sed "s/^${field}: *//"
}

# Check if thread has specific field value
# Usage: if thread_has_field "abc123" "status" "active"; then ...
thread_has_field() {
    local id="$1"
    local field="$2"
    local expected="$3"
    local actual
    actual=$(get_thread_field "$id" "$field")

    [[ "$actual" == "$expected" ]]
}

# Get section content from thread
# Usage: body=$(get_thread_section "abc123" "Body")
get_thread_section() {
    local id="$1"
    local section="$2"
    local path
    path=$(get_thread_path "$id")

    if [[ -z "$path" ]]; then
        return 1
    fi

    # Extract content between ## Section and next ## or EOF
    # Uses portable awk (BSD/GNU compatible)
    awk -v section="$section" '
        /^## / {
            in_section = ($0 == "## " section)
            next
        }
        in_section { print }
    ' "$path"
}

# Check if section contains text
# Usage: if thread_section_contains "abc123" "Body" "some text"; then ...
thread_section_contains() {
    local id="$1"
    local section="$2"
    local needle="$3"
    local content
    content=$(get_thread_section "$id" "$section")

    [[ "$content" == *"$needle"* ]]
}

# List all thread IDs at path
# Usage: ids=$(list_thread_ids)
list_thread_ids() {
    local path="${1:-$TEST_WS}"
    find "$path/.threads" -name "*.md" -type f 2>/dev/null | \
        xargs -I{} basename {} .md | \
        sed 's/-.*//'
}

# Extract thread ID (6-char hex) from CLI output
# Usage: id=$(extract_id_from_output "$output")
extract_id_from_output() {
    local output="$1"
    # Match 6-char hex ID
    echo "$output" | grep -oE '[0-9a-f]{6}' | head -1
}

# Extract note/todo hash (4-char hex) from CLI output
# Pattern: "(id: XXXX)" in output like "Added note: text (id: da6d)"
# Usage: hash=$(extract_hash_from_output "$output")
extract_hash_from_output() {
    local output="$1"
    # Look for "(id: XXXX)" pattern specifically
    echo "$output" | grep -oE '\(id: [0-9a-f]{4}\)' | grep -oE '[0-9a-f]{4}' | head -1
}

# Wait for file to exist (with timeout)
# Usage: wait_for_file "/path/to/file" 5
wait_for_file() {
    local path="$1"
    local timeout="${2:-5}"
    local elapsed=0

    while [[ ! -f "$path" && $elapsed -lt $timeout ]]; do
        sleep 0.1
        elapsed=$((elapsed + 1))
    done

    [[ -f "$path" ]]
}

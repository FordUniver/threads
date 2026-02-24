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

    # Body: everything after the second --- (frontmatter end)
    # Other sections: content between ## Section and next ## or EOF
    if [[ "$section" == "Body" ]]; then
        awk '
            /^---$/ { delim++; next }
            delim >= 2 { print }
        ' "$path"
    else
        awk -v section="$section" '
            /^## / {
                in_section = ($0 == "## " section)
                next
            }
            in_section { print }
        ' "$path"
    fi
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

# Create a malformed thread file for validation testing
# Usage: create_malformed_thread "abc123" "no_frontmatter|invalid_yaml|missing_id|missing_name|missing_status"
create_malformed_thread() {
    local id="$1"
    local type="$2"
    local path="${3:-$TEST_WS}"
    local threads_dir="$path/.threads"
    local filename="${id}-malformed.md"

    mkdir -p "$threads_dir"

    case "$type" in
        no_frontmatter)
            cat > "$threads_dir/$filename" << 'EOF'
# No frontmatter here
Just some content without YAML frontmatter.
EOF
            ;;
        invalid_yaml)
            cat > "$threads_dir/$filename" << 'EOF'
---
id: abc123
name: [unclosed bracket
status: active
---

## Body
EOF
            ;;
        missing_id)
            cat > "$threads_dir/$filename" << 'EOF'
---
name: Thread without ID
status: active
---

## Body
EOF
            ;;
        missing_name)
            cat > "$threads_dir/$filename" << EOF
---
id: $id
status: active
---

## Body
EOF
            ;;
        missing_status)
            cat > "$threads_dir/$filename" << EOF
---
id: $id
name: Thread without status
---

## Body
EOF
            ;;
        unclosed_frontmatter)
            cat > "$threads_dir/$filename" << EOF
---
id: $id
name: Unclosed frontmatter
status: active

## Body
Content without closing delimiter
EOF
            ;;
        *)
            echo "Unknown malformed type: $type" >&2
            return 1
            ;;
    esac

    echo "$threads_dir/$filename"
}

# ====================================================================================
# Nested Git Repository Helpers (for boundary testing)
# ====================================================================================

# Create a nested git repository inside the test workspace
# Usage: create_nested_git_repo "$TEST_WS/subdir"
create_nested_git_repo() {
    local path="$1"
    mkdir -p "$path"
    (
        cd "$path" || exit 1
        git init -q
        git config user.email "nested@test.test"
        git config user.name "Nested Test"
        git commit -q -m "Initial nested repo" --allow-empty
    )
}

# Create a nested git repo with .threads directory
# Usage: create_nested_repo_with_threads "$TEST_WS/nested"
create_nested_repo_with_threads() {
    local path="$1"
    create_nested_git_repo "$path"
    mkdir -p "$path/.threads"
}

# Create thread at specific depth level
# Usage: create_thread_at_depth 2 "abc123" "Thread Name" "active"
# Creates at TEST_WS/level0/level1/.threads/
create_thread_at_depth() {
    local depth="$1"
    local id="$2"
    local name="$3"
    local status="${4:-idea}"
    local path="$TEST_WS"

    for ((i=0; i<depth; i++)); do
        path="$path/level$i"
    done

    mkdir -p "$path/.threads"
    create_thread "$id" "$name" "$status" "" "$path"
}

# Create deeply nested directory structure with threads at each level
# Usage: setup_deep_nested_workspace 3
# Creates threads at TEST_WS, TEST_WS/level0, TEST_WS/level0/level1, TEST_WS/level0/level1/level2
setup_deep_nested_workspace() {
    local depth="${1:-3}"
    setup_test_workspace

    local path="$TEST_WS"
    for ((i=0; i<depth; i++)); do
        path="$path/level$i"
        mkdir -p "$path/.threads"
    done
}

# ====================================================================================
# JSON/YAML Output Helpers
# ====================================================================================

# Extract field from JSON output using jq
# Usage: value=$(get_json_field "$output" ".id")
get_json_field() {
    local output="$1"
    local field="$2"
    echo "$output" | jq -r "$field" 2>/dev/null
}

# Get array length from JSON output
# Usage: count=$(get_json_array_length "$output" ".threads")
get_json_array_length() {
    local output="$1"
    local field="${2:-.}"
    echo "$output" | jq -r "$field | length" 2>/dev/null
}

# Check if JSON output contains a specific value in array
# Usage: if json_array_contains "$output" ".threads[].id" "abc123"; then ...
json_array_contains() {
    local output="$1"
    local selector="$2"
    local value="$3"

    local values
    values=$(echo "$output" | jq -r "$selector" 2>/dev/null)
    [[ "$values" == *"$value"* ]]
}

# ====================================================================================
# Path Resolution Helpers
# ====================================================================================

# Get the git root of the test workspace
# Usage: root=$(get_git_root)
get_git_root() {
    git -C "$TEST_WS" rev-parse --show-toplevel 2>/dev/null
}

# Convert absolute path to git-root-relative path
# Usage: rel_path=$(to_git_relative "$abs_path")
to_git_relative() {
    local abs_path="$1"
    local git_root
    git_root=$(get_git_root)

    if [[ "$abs_path" == "$git_root"* ]]; then
        echo "${abs_path#$git_root/}"
    else
        echo "$abs_path"
    fi
}

# Verify path exists and is inside git root
# Usage: if path_in_git_root "$path"; then ...
path_in_git_root() {
    local path="$1"
    local git_root
    git_root=$(get_git_root)
    local abs_path

    # Resolve to absolute path
    if [[ "$path" = /* ]]; then
        abs_path="$path"
    else
        abs_path="$TEST_WS/$path"
    fi

    [[ "$abs_path" == "$git_root"* ]]
}

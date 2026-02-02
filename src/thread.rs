use std::fs;
use std::path::Path;
use std::sync::LazyLock;

use chrono::Local;
use md5::{Digest, Md5};
use regex::Regex;
use serde::{Deserialize, Serialize};

// Canonical section names in order of appearance
const CANONICAL_SECTIONS: &[&str] = &["Body", "Notes", "Todo", "Log"];

/// Check if a line is a canonical section header (## Body, ## Notes, ## Todo, ## Log)
fn is_canonical_section_header(line: &str) -> bool {
    CANONICAL_SECTIONS
        .iter()
        .any(|&s| line.starts_with(&format!("## {}", s)))
}

// Cached regexes for hot paths
static ID_PREFIX_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^([0-9a-f]{6})-").unwrap());

static HASH_COMMENT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"<!--\s*([a-f0-9]{4})\s*-->").unwrap());

static LOG_SECTION_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?m)^## Log").unwrap());

static NOTES_SECTION_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)(^## Notes)\n").unwrap());

static TODO_SECTION_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)(^## Todo)\n").unwrap());

/// Closed statuses (threads that don't need attention)
pub const CLOSED_STATUSES: &[&str] = &["resolved", "superseded", "deferred", "rejected"];

/// Open statuses (threads that need attention)
#[allow(dead_code)]
pub const OPEN_STATUSES: &[&str] = &["idea", "planning", "active", "blocked", "paused"];

/// Frontmatter represents the YAML frontmatter of a thread
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Frontmatter {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub desc: String,
    #[serde(default)]
    pub status: String,
}

/// Thread represents a parsed thread file
#[derive(Debug, Clone)]
pub struct Thread {
    pub path: String,
    pub frontmatter: Frontmatter,
    pub content: String,
    pub body_start: usize,
}

impl Thread {
    /// Parse a thread file
    pub fn parse(path: &Path) -> Result<Self, String> {
        let content = fs::read_to_string(path).map_err(|e| format!("reading file: {}", e))?;

        let mut thread = Thread {
            path: path.to_string_lossy().to_string(),
            frontmatter: Frontmatter::default(),
            content: content.clone(),
            body_start: 0,
        };

        thread.parse_frontmatter()?;

        // Extract ID from filename if not in frontmatter
        if thread.frontmatter.id.is_empty()
            && let Some(id) = extract_id_from_path(path)
        {
            thread.frontmatter.id = id;
        }

        Ok(thread)
    }

    fn parse_frontmatter(&mut self) -> Result<(), String> {
        if !self.content.starts_with("---\n") {
            return Err("missing frontmatter delimiter".to_string());
        }

        // Find closing delimiter
        let rest = &self.content[4..];
        let end = rest
            .find("\n---")
            .ok_or_else(|| "unclosed frontmatter".to_string())?;

        let yaml_content = &rest[..end];
        self.body_start = 4 + end + 4; // skip opening ---, yaml, closing ---, and newline

        self.frontmatter =
            serde_yaml::from_str(yaml_content).map_err(|e| format!("parsing YAML: {}", e))?;

        Ok(())
    }

    /// Get the thread ID
    pub fn id(&self) -> &str {
        &self.frontmatter.id
    }

    /// Get the thread name/title
    pub fn name(&self) -> &str {
        &self.frontmatter.name
    }

    /// Get the thread status
    pub fn status(&self) -> &str {
        &self.frontmatter.status
    }

    /// Get base status without reason suffix
    pub fn base_status(&self) -> String {
        base_status(&self.frontmatter.status)
    }

    /// Get the body content after frontmatter
    #[allow(dead_code)]
    pub fn body(&self) -> &str {
        if self.body_start >= self.content.len() {
            ""
        } else {
            &self.content[self.body_start..]
        }
    }

    /// Set a frontmatter field and rebuild content
    pub fn set_frontmatter_field(&mut self, field: &str, value: &str) -> Result<(), String> {
        match field {
            "id" => self.frontmatter.id = value.to_string(),
            "name" => self.frontmatter.name = value.to_string(),
            "desc" => self.frontmatter.desc = value.to_string(),
            "status" => self.frontmatter.status = value.to_string(),
            _ => return Err(format!("unknown field: {}", field)),
        }
        self.rebuild_content()
    }

    fn rebuild_content(&mut self) -> Result<(), String> {
        let mut sb = String::new();
        sb.push_str("---\n");

        let yaml = serde_yaml::to_string(&self.frontmatter)
            .map_err(|e| format!("serializing YAML: {}", e))?;
        sb.push_str(&yaml);
        sb.push_str("---\n");

        // Preserve body
        if self.body_start < self.content.len() {
            sb.push_str(&self.content[self.body_start..]);
        }

        self.content = sb;
        Ok(())
    }

    /// Write the thread to disk
    pub fn write(&self) -> Result<(), String> {
        fs::write(&self.path, &self.content).map_err(|e| format!("writing file: {}", e))
    }

    /// Get path relative to workspace
    #[allow(dead_code)]
    pub fn rel_path(&self, ws: &Path) -> String {
        let path = Path::new(&self.path);
        path.strip_prefix(ws)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| self.path.clone())
    }
}

/// Extract ID from filename (6-char hex prefix)
pub fn extract_id_from_path(path: &Path) -> Option<String> {
    let filename = path.file_name()?.to_string_lossy();
    let filename = filename.trim_end_matches(".md");

    ID_PREFIX_RE.captures(filename).map(|c| c[1].to_string())
}

/// Extract name from filename (after ID prefix)
pub fn extract_name_from_path(path: &Path) -> String {
    let filename = path
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_default();
    let filename = filename.trim_end_matches(".md");

    if ID_PREFIX_RE.is_match(filename) && filename.len() > 7 {
        filename[7..].to_string()
    } else {
        filename.to_string()
    }
}

/// Strip reason suffix from status (e.g., "blocked (waiting)" -> "blocked")
pub fn base_status(status: &str) -> String {
    if let Some(idx) = status.find(" (") {
        status[..idx].to_string()
    } else {
        status.to_string()
    }
}

/// Check if a status is closed (using default status lists)
pub fn is_closed(status: &str) -> bool {
    let base = base_status(status);
    CLOSED_STATUSES.contains(&base.as_str())
}

/// Check if a status is closed (using config status lists)
#[allow(dead_code)]
pub fn is_closed_with_config(status: &str, closed_statuses: &[String]) -> bool {
    let base = base_status(status);
    closed_statuses.iter().any(|s| s == &base)
}

/// Check if a status is valid (using default status lists)
#[allow(dead_code)]
pub fn is_valid_status(status: &str) -> bool {
    let base = base_status(status);
    OPEN_STATUSES.contains(&base.as_str()) || CLOSED_STATUSES.contains(&base.as_str())
}

/// Check if a status is valid (using config status lists)
pub fn is_valid_status_with_config(
    status: &str,
    open_statuses: &[String],
    closed_statuses: &[String],
) -> bool {
    let base = base_status(status);
    open_statuses.iter().any(|s| s == &base) || closed_statuses.iter().any(|s| s == &base)
}

/// Escape $ characters in replacement strings for regex ($ is backreference, $$ is literal $)
fn escape_for_replacement(text: &str) -> String {
    text.replace('$', "$$")
}

/// Generate a 4-character hash for an item
pub fn generate_hash(text: &str) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let data = format!("{}{}", text, now);

    let mut hasher = Md5::new();
    hasher.update(data.as_bytes());
    let result = hasher.finalize();
    format!("{:02x}{:02x}", result[0], result[1])
}

/// Insert a log entry with full timestamp
pub fn insert_log_entry(content: &str, entry: &str) -> String {
    let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let bullet_entry = format!("- [{}] {}", timestamp, entry);

    // Check if Log section exists
    if LOG_SECTION_RE.is_match(content) {
        // Insert after ## Log
        return LOG_SECTION_RE
            .replace(
                content,
                format!("## Log\n\n{}", escape_for_replacement(&bullet_entry)),
            )
            .to_string();
    }

    // No Log section - append one
    format!("{}\n## Log\n\n{}\n", content, bullet_entry)
}

/// Ensure a section exists, placing it before another section
pub fn ensure_section(content: &str, name: &str, before: &str) -> String {
    let pattern = format!(r"(?m)^## {}", regex::escape(name));
    let re = Regex::new(&pattern).unwrap();
    if re.is_match(content) {
        return content.to_string();
    }

    let before_pattern = format!(r"(?m)(^## {})", regex::escape(before));
    let before_re = Regex::new(&before_pattern).unwrap();

    if before_re.is_match(content) {
        return before_re
            .replace(content, format!("## {}\n\n$1", name))
            .to_string();
    }

    // If before section doesn't exist, append at end
    format!("{}\n## {}\n\n", content, name)
}

/// Replace section content
pub fn replace_section(content: &str, name: &str, new_content: &str) -> String {
    let boundary = section_boundary_pattern(name);
    let pattern = format!(r"(?ms)(^## {})\n(.+?)({})", regex::escape(name), boundary);
    let re = Regex::new(&pattern).unwrap();

    if !re.is_match(content) {
        return content.to_string();
    }

    re.replace(
        content,
        format!("$1\n\n{}\n\n$3", escape_for_replacement(new_content)),
    )
    .to_string()
}

/// Append to section content
pub fn append_to_section(content: &str, name: &str, addition: &str) -> String {
    let section_content = extract_section(content, name);
    let mut new_content = section_content.trim().to_string();
    if !new_content.is_empty() {
        new_content.push('\n');
    }
    new_content.push_str(addition);
    replace_section(content, name, &new_content)
}

/// Build regex alternation for canonical section boundaries (sections that come after `name`)
fn section_boundary_pattern(name: &str) -> String {
    let pos = CANONICAL_SECTIONS.iter().position(|&s| s == name);
    match pos {
        Some(idx) if idx + 1 < CANONICAL_SECTIONS.len() => {
            // Build alternation of sections that come after this one
            let following: Vec<&str> = CANONICAL_SECTIONS[idx + 1..].to_vec();
            let alt = following
                .iter()
                .map(|s| regex::escape(s))
                .collect::<Vec<_>>()
                .join("|");
            format!(r"(?:^## (?:{})(?:\s*$|\n)|\z)", alt)
        }
        _ => {
            // Unknown section or last section (Log) - only stop at end
            r"\z".to_string()
        }
    }
}

/// Extract section content with normalization
pub fn extract_section(content: &str, name: &str) -> String {
    let boundary = section_boundary_pattern(name);
    let pattern = format!(r"(?ms)^## {}\n(.+?){}", regex::escape(name), boundary);
    let re = Regex::new(&pattern).unwrap();

    let raw = if let Some(caps) = re.captures(content) {
        caps.get(1)
            .map(|m| m.as_str().trim().to_string())
            .unwrap_or_default()
    } else {
        return String::new();
    };

    // Apply section-specific normalization
    match name {
        "Body" => normalize_body(&raw),
        "Notes" | "Todo" | "Log" => normalize_list_section(&raw),
        _ => raw,
    }
}

/// Normalize Body section: convert `## ` headers to `### ` (h2 → h3)
/// Body should use h3+ headers since h2 is reserved for canonical sections.
fn normalize_body(content: &str) -> String {
    // Match `## ` at start of line that isn't followed by a canonical section name
    let mut result = String::new();
    for line in content.lines() {
        if line.starts_with("## ") && !is_canonical_section_header(line) {
            // Downgrade h2 to h3
            result.push_str("###");
            result.push_str(&line[2..]);
        } else {
            result.push_str(line);
        }
        result.push('\n');
    }
    result.trim_end().to_string()
}

/// Normalize list-based sections (Notes, Todo, Log): collapse multiple empty lines
/// and remove empty lines between list items.
fn normalize_list_section(content: &str) -> String {
    let mut result = Vec::new();
    let mut prev_was_item = false;
    let mut prev_was_empty = false;

    for line in content.lines() {
        let trimmed = line.trim();
        let is_item = trimmed.starts_with("- ") || trimmed.starts_with("### ");
        let is_empty = trimmed.is_empty();

        if is_empty {
            // Skip empty lines between consecutive items
            if prev_was_item {
                prev_was_empty = true;
                continue;
            }
            // Collapse multiple empty lines to one
            if prev_was_empty {
                continue;
            }
            prev_was_empty = true;
        } else {
            prev_was_empty = false;
        }

        result.push(line);
        prev_was_item = is_item;
    }

    result.join("\n").trim().to_string()
}

/// Add a note to the Notes section
pub fn add_note(content: &str, text: &str) -> (String, String) {
    let content = ensure_section(content, "Notes", "Todo");

    let hash = generate_hash(text);
    let note_entry = format!("- {}  <!-- {} -->", text, hash);

    // Insert at top of Notes section
    let new_content = NOTES_SECTION_RE
        .replace(
            &content,
            format!("$1\n\n{}\n", escape_for_replacement(&note_entry)),
        )
        .to_string();

    (new_content, hash)
}

/// Add a todo item
pub fn add_todo_item(content: &str, text: &str) -> (String, String) {
    let hash = generate_hash(text);
    let todo_entry = format!("- [ ] {}  <!-- {} -->", text, hash);

    // Insert at top of Todo section
    let new_content = TODO_SECTION_RE
        .replace(
            content,
            format!("$1\n\n{}\n", escape_for_replacement(&todo_entry)),
        )
        .to_string();

    (new_content, hash)
}

/// Get all todo items as (checked, text, hash) tuples
pub fn get_todo_items(content: &str) -> Vec<(bool, String, String)> {
    let section = extract_section(content, "Todo");
    let mut items = Vec::new();

    for line in section.lines() {
        let line = line.trim();
        // Match: - [ ] text  <!-- hash --> or - [x] text  <!-- hash -->
        if let Some(rest) = line.strip_prefix("- [") {
            let checked = rest.starts_with('x');
            if let Some(after_bracket) = rest
                .strip_prefix("x] ")
                .or_else(|| rest.strip_prefix(" ] "))
                && let Some((text, hash_part)) = after_bracket.rsplit_once("<!--")
            {
                let text = text.trim().to_string();
                let hash = hash_part.trim().trim_end_matches("-->").trim().to_string();
                if !hash.is_empty() {
                    items.push((checked, text, hash));
                }
            }
        }
    }

    items
}

/// Get all notes as (text, hash) tuples
pub fn get_notes(content: &str) -> Vec<(String, String)> {
    let section = extract_section(content, "Notes");
    let mut items = Vec::new();

    for line in section.lines() {
        let line = line.trim();
        // Match: - text  <!-- hash -->
        if let Some(rest) = line.strip_prefix("- ") {
            // Skip todo-style items (shouldn't be in Notes, but just in case)
            if rest.starts_with('[') {
                continue;
            }
            if let Some((text, hash_part)) = rest.rsplit_once("<!--") {
                let text = text.trim().to_string();
                let hash = hash_part.trim().trim_end_matches("-->").trim().to_string();
                if !hash.is_empty() {
                    items.push((text, hash));
                }
            }
        }
    }

    items
}

/// Remove item by hash from a section
pub fn remove_by_hash(content: &str, section: &str, hash: &str) -> Result<String, String> {
    let lines: Vec<&str> = content.lines().collect();
    let mut result = Vec::new();
    let mut in_section = false;
    let hash_pattern = format!("<!-- {}", hash);
    let mut found = false;

    for line in lines {
        if line.starts_with(&format!("## {}", section)) {
            in_section = true;
        } else if is_canonical_section_header(line) {
            in_section = false;
        }

        if in_section && line.contains(&hash_pattern) && !found {
            found = true;
            continue; // skip this line
        }
        result.push(line);
    }

    if !found {
        return Err(format!("no item with hash '{}' found", hash));
    }

    Ok(result.join("\n"))
}

/// Edit item by hash
pub fn edit_by_hash(
    content: &str,
    section: &str,
    hash: &str,
    new_text: &str,
) -> Result<String, String> {
    let lines: Vec<&str> = content.lines().collect();
    let mut result = Vec::new();
    let mut in_section = false;
    let hash_pattern = format!("<!-- {}", hash);
    let mut found = false;

    for line in lines {
        if line.starts_with(&format!("## {}", section)) {
            in_section = true;
        } else if is_canonical_section_header(line) {
            in_section = false;
        }

        if in_section && line.contains(&hash_pattern) && !found {
            found = true;
            // Extract hash and rebuild
            if let Some(caps) = HASH_COMMENT_RE.captures(line) {
                let h = &caps[1];
                result.push(format!("- {}  <!-- {} -->", new_text, h));
                continue;
            }
        }
        result.push(line.to_string());
    }

    if !found {
        return Err(format!("no item with hash '{}' found", hash));
    }

    Ok(result.join("\n"))
}

/// Set todo item checked state by hash
pub fn set_todo_checked(
    content: &str,
    section: &str,
    hash: &str,
    checked: bool,
) -> Result<String, String> {
    let lines: Vec<&str> = content.lines().collect();
    let mut result = Vec::new();
    let mut in_section = false;
    let hash_pattern = format!("<!-- {}", hash);
    let section_header = format!("## {}", section);
    let mut found = false;

    for line in lines {
        let mut line = line.to_string();
        if line.starts_with(&section_header) {
            in_section = true;
        } else if is_canonical_section_header(&line) {
            in_section = false;
        }

        if in_section && line.contains(&hash_pattern) && !found {
            found = true;
            if checked {
                line = line.replace("- [ ]", "- [x]");
            } else {
                line = line.replace("- [x]", "- [ ]");
            }
        }
        result.push(line);
    }

    if !found {
        return Err(format!("no item with hash '{}' found", hash));
    }

    Ok(result.join("\n"))
}

/// Count items matching a hash prefix in a section
pub fn count_matching_items(content: &str, section: &str, hash: &str) -> usize {
    let lines: Vec<&str> = content.lines().collect();
    let mut in_section = false;
    let hash_pattern = format!("<!-- {}", hash);
    let mut count = 0;

    for line in lines {
        if line.starts_with(&format!("## {}", section)) {
            in_section = true;
        } else if is_canonical_section_header(line) {
            in_section = false;
        }

        if in_section && line.contains(&hash_pattern) {
            count += 1;
        }
    }

    count
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_extract_id_from_path() {
        let cases = vec![
            ("abc123-my-thread.md", Some("abc123")),
            ("/path/to/abc123-my-thread.md", Some("abc123")),
            ("deadbe-another-one.md", Some("deadbe")),
            ("no-id-here.md", None),
            ("ABC123-uppercase.md", None), // only lowercase hex
            ("ab123-too-short.md", None),  // need 6 chars
            ("abc1234-too-long.md", None), // only 6 chars
        ];

        for (path, want) in cases {
            let got = extract_id_from_path(Path::new(path));
            assert_eq!(
                got.as_deref(),
                want,
                "extract_id_from_path({:?}) = {:?}, want {:?}",
                path,
                got,
                want
            );
        }
    }

    #[test]
    fn test_extract_name_from_path() {
        let cases = vec![
            ("abc123-my-thread.md", "my-thread"),
            ("/path/to/abc123-my-thread.md", "my-thread"),
            ("abc123-multi-word-name.md", "multi-word-name"),
            ("no-id-here.md", "no-id-here"),
        ];

        for (path, want) in cases {
            let got = extract_name_from_path(Path::new(path));
            assert_eq!(
                got, want,
                "extract_name_from_path({:?}) = {:?}, want {:?}",
                path, got, want
            );
        }
    }

    #[test]
    fn test_base_status() {
        let cases = vec![
            ("active", "active"),
            ("blocked (waiting for review)", "blocked"),
            ("resolved (done)", "resolved"),
            ("paused (vacation)", "paused"),
            ("idea", "idea"),
        ];

        for (status, want) in cases {
            let got = base_status(status);
            assert_eq!(
                got, want,
                "base_status({:?}) = {:?}, want {:?}",
                status, got, want
            );
        }
    }

    #[test]
    fn test_is_closed() {
        let cases = vec![
            ("active", false),
            ("blocked", false),
            ("blocked (waiting)", false),
            ("resolved", true),
            ("resolved (done)", true),
            ("superseded", true),
            ("deferred", true),
            ("rejected", true),
        ];

        for (status, want) in cases {
            let got = is_closed(status);
            assert_eq!(
                got, want,
                "is_closed({:?}) = {:?}, want {:?}",
                status, got, want
            );
        }
    }

    #[test]
    fn test_is_valid_status() {
        let cases = vec![
            ("active", true),
            ("blocked", true),
            ("blocked (reason)", true),
            ("resolved", true),
            ("invalid", false),
            ("random", false),
        ];

        for (status, want) in cases {
            let got = is_valid_status(status);
            assert_eq!(
                got, want,
                "is_valid_status({:?}) = {:?}, want {:?}",
                status, got, want
            );
        }
    }

    #[test]
    fn test_extract_section_with_nested_headers() {
        // Body section contains ## headers that should NOT be treated as section boundaries
        let content = r#"---
id: 'abc123'
name: Test
status: active
---

## Body

Some intro text.

## Subsection One

Content under subsection one.

## Subsection Two

Content under subsection two.

## Notes

A note here.

## Todo

- [ ] A task

## Log

- [2026-01-01] Created
"#;

        let body = extract_section(content, "Body");
        assert!(
            body.contains("Subsection One"),
            "Body should contain 'Subsection One', got: {}",
            body
        );
        assert!(
            body.contains("Subsection Two"),
            "Body should contain 'Subsection Two', got: {}",
            body
        );
        assert!(
            !body.contains("A note here"),
            "Body should NOT contain Notes content, got: {}",
            body
        );

        let notes = extract_section(content, "Notes");
        assert!(
            notes.contains("A note here"),
            "Notes should contain 'A note here', got: {}",
            notes
        );

        // Verify h2 headers in Body are normalized to h3
        assert!(
            body.contains("### Subsection One"),
            "h2 in Body should be normalized to h3, got: {}",
            body
        );
    }

    #[test]
    fn test_normalize_body_converts_h2_to_h3() {
        let body = "Some intro.\n\n## My Header\n\nContent.\n\n## Another\n\nMore.";
        let normalized = normalize_body(body);

        // Check h2 converted to h3
        assert!(
            normalized.contains("### My Header"),
            "## should become ###, got: {}",
            normalized
        );
        assert!(
            normalized.contains("### Another"),
            "## should become ###, got: {}",
            normalized
        );

        // Ensure no bare "## " remains (h2 headers should all be converted)
        // Note: "### " contains "## " as a substring, so we check for h2 pattern at line start
        let has_h2 = normalized.lines().any(|l| l.starts_with("## "));
        assert!(!has_h2, "No h2 headers should remain, got: {}", normalized);
    }

    #[test]
    fn test_normalize_list_section_removes_empty_lines() {
        let section = "- Item one\n\n- Item two\n\n\n- Item three";
        let normalized = normalize_list_section(section);

        assert_eq!(
            normalized, "- Item one\n- Item two\n- Item three",
            "Empty lines between items should be removed"
        );
    }

    // ========================================================================
    // Regression tests for severe truncation bug
    // Bug: Body with ## headers caused extract_section to truncate early
    // ========================================================================

    #[test]
    fn test_extract_body_not_truncated_by_h2_headers() {
        // This is the exact pattern that caused the bug:
        // Body contains ## headers that looked like section boundaries
        let content = r#"---
id: 'abc123'
name: Test thread
status: active
---

## Body

Introduction paragraph.

## First Topic

Content under first topic.

## Second Topic

Content under second topic.

## Third Topic

This content was being truncated before the fix.

## Notes

- A note

## Todo

- [ ] A task

## Log

- [2026-01-01] Created
"#;

        let body = extract_section(content, "Body");

        // The critical assertion: ALL content before ## Notes must be present
        assert!(
            body.contains("Introduction paragraph"),
            "Body should contain intro"
        );
        assert!(
            body.contains("First Topic"),
            "Body should contain First Topic"
        );
        assert!(
            body.contains("Second Topic"),
            "Body should contain Second Topic"
        );
        assert!(
            body.contains("Third Topic"),
            "Body should contain Third Topic - this was truncated before fix"
        );
        assert!(
            body.contains("was being truncated"),
            "Body should contain content under Third Topic"
        );

        // Verify sections are properly separated
        assert!(
            !body.contains("A note"),
            "Body should NOT contain Notes content"
        );
        assert!(
            !body.contains("A task"),
            "Body should NOT contain Todo content"
        );
    }

    #[test]
    fn test_extract_section_respects_only_canonical_boundaries() {
        // Verify that only Body/Notes/Todo/Log act as boundaries
        let content = r#"---
id: 'test'
name: Test
status: active
---

## Body

Intro.

## Random Header

This is NOT a canonical section - should be part of Body.

## Another Random

Also part of Body.

## Notes

Real notes section.

## Todo

- [ ] Real task

## Log

- Entry
"#;

        let body = extract_section(content, "Body");
        let notes = extract_section(content, "Notes");

        // Body should contain non-canonical ## headers
        assert!(
            body.contains("Random Header"),
            "Non-canonical ## should be in Body"
        );
        assert!(
            body.contains("Another Random"),
            "Non-canonical ## should be in Body"
        );

        // Notes should only have its own content
        assert!(
            notes.contains("Real notes section"),
            "Notes should have its content"
        );
        assert!(
            !notes.contains("Random"),
            "Notes should not have Body content"
        );
    }

    #[test]
    fn test_extract_all_sections_with_complex_body() {
        // Full integration test with realistic thread structure
        let content = r#"---
id: '9559e8'
name: 'Paper proofreading'
desc: Technical issues for review
status: active
---

## Body

## Overview

Paper needs several fixes.

## Technical Issues

### Type signature mismatch

Details here.

### Reflection axis error

More details.

## Terminology

Key terms need definition.

## Notes

- First note  <!-- a1b2 -->
- Second note  <!-- c3d4 -->

## Todo

- [ ] Fix issue one  <!-- e5f6 -->
- [ ] Fix issue two  <!-- g7h8 -->

## Log

- [2026-01-01 10:00:00] Created
- [2026-01-02 11:00:00] Updated
"#;

        let body = extract_section(content, "Body");
        let notes = extract_section(content, "Notes");
        let todo = extract_section(content, "Todo");
        let log = extract_section(content, "Log");

        // Body: all subsections present
        assert!(body.contains("Overview"), "Body missing Overview");
        assert!(
            body.contains("Technical Issues"),
            "Body missing Technical Issues"
        );
        assert!(
            body.contains("Type signature mismatch"),
            "Body missing subsection"
        );
        assert!(
            body.contains("Reflection axis error"),
            "Body missing subsection"
        );
        assert!(body.contains("Terminology"), "Body missing Terminology");

        // Notes: items present, no Body contamination
        assert!(notes.contains("First note"), "Notes missing first note");
        assert!(notes.contains("Second note"), "Notes missing second note");
        assert!(!notes.contains("Overview"), "Notes contaminated with Body");

        // Todo: items present
        assert!(todo.contains("Fix issue one"), "Todo missing first item");
        assert!(todo.contains("Fix issue two"), "Todo missing second item");

        // Log: entries present
        assert!(log.contains("Created"), "Log missing first entry");
        assert!(log.contains("Updated"), "Log missing second entry");
    }

    #[test]
    fn test_body_h2_normalized_to_h3_in_extraction() {
        let content = r#"---
id: 'test'
name: Test
status: active
---

## Body

## My H2 Header

Content.

## Another H2

More content.

## Notes

Note here.
"#;

        let body = extract_section(content, "Body");

        // H2 headers should be normalized to H3
        assert!(
            body.contains("### My H2 Header"),
            "## should be normalized to ###, got: {}",
            body
        );
        assert!(
            body.contains("### Another H2"),
            "## should be normalized to ###"
        );

        // No bare ## should remain
        let has_h2 = body.lines().any(|l| l.starts_with("## "));
        assert!(!has_h2, "No h2 headers should remain after normalization");
    }
}

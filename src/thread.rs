use std::fs;
use std::path::Path;
use std::sync::LazyLock;

use chrono::Local;
use md5::{Digest, Md5};
use regex::Regex;
use serde::{Deserialize, Serialize};

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
        if thread.frontmatter.id.is_empty() {
            if let Some(id) = extract_id_from_path(path) {
                thread.frontmatter.id = id;
            }
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

/// Check if a status is closed
pub fn is_closed(status: &str) -> bool {
    let base = base_status(status);
    CLOSED_STATUSES.contains(&base.as_str())
}

/// Check if a status is valid
pub fn is_valid_status(status: &str) -> bool {
    let base = base_status(status);
    OPEN_STATUSES.contains(&base.as_str()) || CLOSED_STATUSES.contains(&base.as_str())
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
    let pattern = format!(r"(?ms)(^## {})\n(.+?)(^## |\z)", regex::escape(name));
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

/// Extract section content
pub fn extract_section(content: &str, name: &str) -> String {
    let pattern = format!(r"(?ms)^## {}\n(.+?)(?:^## |\z)", regex::escape(name));
    let re = Regex::new(&pattern).unwrap();

    if let Some(caps) = re.captures(content) {
        caps.get(1)
            .map(|m| m.as_str().trim().to_string())
            .unwrap_or_default()
    } else {
        String::new()
    }
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
        } else if line.starts_with("## ") {
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
        } else if line.starts_with("## ") {
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
pub fn set_todo_checked(content: &str, hash: &str, checked: bool) -> Result<String, String> {
    let lines: Vec<&str> = content.lines().collect();
    let mut result = Vec::new();
    let mut in_todo = false;
    let hash_pattern = format!("<!-- {}", hash);
    let mut found = false;

    for line in lines {
        let mut line = line.to_string();
        if line.starts_with("## Todo") {
            in_todo = true;
        } else if line.starts_with("## ") {
            in_todo = false;
        }

        if in_todo && line.contains(&hash_pattern) && !found {
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
        } else if line.starts_with("## ") {
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
}

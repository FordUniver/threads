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

/// Closed statuses (threads that don't need attention)
pub const CLOSED_STATUSES: &[&str] = &["resolved", "superseded", "deferred", "rejected"];

/// Open statuses (threads that need attention)
#[allow(dead_code)]
pub const OPEN_STATUSES: &[&str] = &["idea", "planning", "active", "blocked", "paused"];

// ============================================================================
// Item types for frontmatter-based structured data
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoteItem {
    pub text: String,
    pub hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TodoItem {
    pub text: String,
    pub hash: String,
    pub done: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub ts: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeadlineItem {
    pub date: String, // "YYYY-MM-DD"
    pub text: String,
    pub hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventItem {
    pub date: String, // "YYYY-MM-DD"
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time: Option<String>, // "HH:MM" or absent
    pub text: String,
    pub hash: String,
}

// ============================================================================
// Frontmatter
// ============================================================================

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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<NoteItem>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub todo: Vec<TodoItem>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub log: Vec<LogEntry>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub deadlines: Vec<DeadlineItem>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub events: Vec<EventItem>,
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

    /// Rebuild content from frontmatter + current body, updating body_start.
    pub fn rebuild_content(&mut self) -> Result<(), String> {
        // Extract old body before we overwrite content.
        // Normalize leading newlines: body_start may land on the '\n' of "---\n"
        // (off-by-one from parse), and repeated rebuilds can accumulate blank lines.
        // Strip all leading '\n', then prepend exactly one as the separator.
        let old_body = if self.body_start < self.content.len() {
            let raw = &self.content[self.body_start..];
            let trimmed = raw.trim_start_matches('\n');
            if trimmed.is_empty() {
                String::new()
            } else {
                format!("\n{}", trimmed)
            }
        } else {
            String::new()
        };

        let mut sb = String::new();
        sb.push_str("---\n");

        let yaml = serde_yaml::to_string(&self.frontmatter)
            .map_err(|e| format!("serializing YAML: {}", e))?;
        sb.push_str(yaml.trim_end());
        sb.push('\n');
        sb.push_str("---\n");

        let new_body_start = sb.len();
        sb.push_str(&old_body);

        self.content = sb;
        self.body_start = new_body_start;
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

    // ========================================================================
    // Frontmatter-based item access (with section fallback for old-format files)
    // ========================================================================

    /// Get all notes. Reads from frontmatter if present, otherwise parses the Notes section.
    pub fn get_notes(&self) -> Vec<NoteItem> {
        if !self.frontmatter.notes.is_empty() {
            return self.frontmatter.notes.clone();
        }
        get_notes_from_section(&self.content)
    }

    /// Get all todo items. Reads from frontmatter if present, otherwise parses the Todo section.
    pub fn get_todo_items(&self) -> Vec<TodoItem> {
        if !self.frontmatter.todo.is_empty() {
            return self.frontmatter.todo.clone();
        }
        get_todo_items_from_section(&self.content)
    }

    /// Get all log entries. Reads from frontmatter if present, otherwise parses the Log section.
    pub fn get_log_entries(&self) -> Vec<LogEntry> {
        if !self.frontmatter.log.is_empty() {
            return self.frontmatter.log.clone();
        }
        get_log_entries_from_section(&self.content)
    }

    /// Add a note to frontmatter (prepend). Returns the generated hash.
    pub fn add_note(&mut self, text: &str) -> Result<String, String> {
        let hash = generate_hash(text);
        self.frontmatter.notes.insert(
            0,
            NoteItem {
                text: text.to_string(),
                hash: hash.clone(),
            },
        );
        self.rebuild_content()?;
        Ok(hash)
    }

    /// Add a todo item to frontmatter (prepend). Returns the generated hash.
    pub fn add_todo_item(&mut self, text: &str) -> Result<String, String> {
        let hash = generate_hash(text);
        self.frontmatter.todo.insert(
            0,
            TodoItem {
                text: text.to_string(),
                hash: hash.clone(),
                done: false,
            },
        );
        self.rebuild_content()?;
        Ok(hash)
    }

    /// Add a log entry to frontmatter (prepend with current timestamp).
    pub fn insert_log_entry(&mut self, entry: &str) -> Result<(), String> {
        let ts = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        self.frontmatter.log.insert(
            0,
            LogEntry {
                ts,
                text: entry.to_string(),
            },
        );
        self.rebuild_content()
    }

    /// Count items matching a hash prefix in the given section.
    /// Checks frontmatter if populated, otherwise falls back to section parsing.
    pub fn count_matching_items(&self, section: &str, hash: &str) -> usize {
        match section {
            "Notes" => {
                if !self.frontmatter.notes.is_empty() {
                    return self
                        .frontmatter
                        .notes
                        .iter()
                        .filter(|n| n.hash.starts_with(hash))
                        .count();
                }
                count_matching_items_from_section(&self.content, section, hash)
            }
            "Todo" => {
                if !self.frontmatter.todo.is_empty() {
                    return self
                        .frontmatter
                        .todo
                        .iter()
                        .filter(|t| t.hash.starts_with(hash))
                        .count();
                }
                count_matching_items_from_section(&self.content, section, hash)
            }
            _ => count_matching_items_from_section(&self.content, section, hash),
        }
    }

    /// Remove an item by hash from the given section.
    /// Operates on frontmatter if populated, otherwise falls back to section content.
    pub fn remove_by_hash(&mut self, section: &str, hash: &str) -> Result<(), String> {
        match section {
            "Notes" => {
                if !self.frontmatter.notes.is_empty() {
                    let pos = self
                        .frontmatter
                        .notes
                        .iter()
                        .position(|n| n.hash.starts_with(hash))
                        .ok_or_else(|| format!("no item with hash '{}' found", hash))?;
                    self.frontmatter.notes.remove(pos);
                    return self.rebuild_content();
                }
            }
            "Todo" => {
                if !self.frontmatter.todo.is_empty() {
                    let pos = self
                        .frontmatter
                        .todo
                        .iter()
                        .position(|t| t.hash.starts_with(hash))
                        .ok_or_else(|| format!("no item with hash '{}' found", hash))?;
                    self.frontmatter.todo.remove(pos);
                    return self.rebuild_content();
                }
            }
            _ => {}
        }
        // Fallback to section-based removal
        self.content = remove_by_hash_from_section(&self.content, section, hash)?;
        Ok(())
    }

    /// Edit an item by hash in the given section.
    /// Operates on frontmatter if populated, otherwise falls back to section content.
    pub fn edit_by_hash(
        &mut self,
        section: &str,
        hash: &str,
        new_text: &str,
    ) -> Result<(), String> {
        if section == "Notes" && !self.frontmatter.notes.is_empty() {
            let item = self
                .frontmatter
                .notes
                .iter_mut()
                .find(|n| n.hash.starts_with(hash))
                .ok_or_else(|| format!("no item with hash '{}' found", hash))?;
            item.text = new_text.to_string();
            return self.rebuild_content();
        }
        // Fallback to section-based edit
        self.content = edit_by_hash_from_section(&self.content, section, hash, new_text)?;
        Ok(())
    }

    // ========================================================================
    // Deadline operations
    // ========================================================================

    /// Get all deadline items.
    pub fn get_deadlines(&self) -> Vec<DeadlineItem> {
        self.frontmatter.deadlines.clone()
    }

    /// Add a deadline to frontmatter (prepend). Returns the generated hash.
    pub fn add_deadline(&mut self, date: &str, text: &str) -> Result<String, String> {
        let hash = generate_hash(&format!("{}{}", date, text));
        // Check for collision
        if self
            .frontmatter
            .deadlines
            .iter()
            .any(|d| d.hash.starts_with(&hash))
        {
            return Err(format!("hash collision for '{}'", hash));
        }
        self.frontmatter.deadlines.insert(
            0,
            DeadlineItem {
                date: date.to_string(),
                text: text.to_string(),
                hash: hash.clone(),
            },
        );
        self.rebuild_content()?;
        Ok(hash)
    }

    /// Remove a deadline by hash prefix. Errors on ambiguous or missing hash.
    pub fn remove_deadline_by_hash(&mut self, hash: &str) -> Result<(), String> {
        let count = self
            .frontmatter
            .deadlines
            .iter()
            .filter(|d| d.hash.starts_with(hash))
            .count();
        if count == 0 {
            return Err(format!("no deadline with hash '{}' found", hash));
        }
        if count > 1 {
            return Err(format!(
                "ambiguous hash '{}' matches {} deadlines",
                hash, count
            ));
        }
        let pos = self
            .frontmatter
            .deadlines
            .iter()
            .position(|d| d.hash.starts_with(hash))
            .unwrap();
        self.frontmatter.deadlines.remove(pos);
        self.rebuild_content()
    }

    // ========================================================================
    // Event operations
    // ========================================================================

    /// Get all event items.
    pub fn get_events(&self) -> Vec<EventItem> {
        self.frontmatter.events.clone()
    }

    /// Add an event to frontmatter (prepend). Returns the generated hash.
    pub fn add_event(
        &mut self,
        date: &str,
        time: Option<&str>,
        text: &str,
    ) -> Result<String, String> {
        let hash = generate_hash(&format!("{}{}{}", date, time.unwrap_or(""), text));
        if self
            .frontmatter
            .events
            .iter()
            .any(|e| e.hash.starts_with(&hash))
        {
            return Err(format!("hash collision for '{}'", hash));
        }
        self.frontmatter.events.insert(
            0,
            EventItem {
                date: date.to_string(),
                time: time.map(str::to_string),
                text: text.to_string(),
                hash: hash.clone(),
            },
        );
        self.rebuild_content()?;
        Ok(hash)
    }

    /// Remove an event by hash prefix. Errors on ambiguous or missing hash.
    pub fn remove_event_by_hash(&mut self, hash: &str) -> Result<(), String> {
        let count = self
            .frontmatter
            .events
            .iter()
            .filter(|e| e.hash.starts_with(hash))
            .count();
        if count == 0 {
            return Err(format!("no event with hash '{}' found", hash));
        }
        if count > 1 {
            return Err(format!(
                "ambiguous hash '{}' matches {} events",
                hash, count
            ));
        }
        let pos = self
            .frontmatter
            .events
            .iter()
            .position(|e| e.hash.starts_with(hash))
            .unwrap();
        self.frontmatter.events.remove(pos);
        self.rebuild_content()
    }

    /// Set a todo item's checked state by hash.
    /// Operates on frontmatter if populated, otherwise falls back to section content.
    pub fn set_todo_checked(&mut self, hash: &str, checked: bool) -> Result<(), String> {
        if !self.frontmatter.todo.is_empty() {
            let item = self
                .frontmatter
                .todo
                .iter_mut()
                .find(|t| t.hash.starts_with(hash))
                .ok_or_else(|| format!("no item with hash '{}' found", hash))?;
            item.done = checked;
            return self.rebuild_content();
        }
        // Fallback to section-based
        self.content = set_todo_checked_from_section(&self.content, "Todo", hash, checked)?;
        Ok(())
    }
}

// ============================================================================
// Path utilities
// ============================================================================

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

// ============================================================================
// Status utilities
// ============================================================================

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

// ============================================================================
// Hash generation
// ============================================================================

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

// ============================================================================
// Section-based operations (legacy / fallback / migration use)
// ============================================================================

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
        "Notes" | "Todo" | "Log" => normalize_list_section(&raw),
        _ => raw,
    }
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

/// Parse notes from the Notes markdown section (fallback for old-format files).
pub fn get_notes_from_section(content: &str) -> Vec<NoteItem> {
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
                    items.push(NoteItem { text, hash });
                }
            }
        }
    }

    items
}

/// Parse todo items from the Todo markdown section (fallback for old-format files).
pub fn get_todo_items_from_section(content: &str) -> Vec<TodoItem> {
    let section = extract_section(content, "Todo");
    let mut items = Vec::new();

    for line in section.lines() {
        let line = line.trim();
        // Match: - [ ] text  <!-- hash --> or - [x] text  <!-- hash -->
        if let Some(rest) = line.strip_prefix("- [") {
            let done = rest.starts_with('x');
            if let Some(after_bracket) = rest
                .strip_prefix("x] ")
                .or_else(|| rest.strip_prefix(" ] "))
                && let Some((text, hash_part)) = after_bracket.rsplit_once("<!--")
            {
                let text = text.trim().to_string();
                let hash = hash_part.trim().trim_end_matches("-->").trim().to_string();
                if !hash.is_empty() {
                    items.push(TodoItem { text, hash, done });
                }
            }
        }
    }

    items
}

/// Parse log entries from the Log markdown section (fallback for old-format files).
pub fn get_log_entries_from_section(content: &str) -> Vec<LogEntry> {
    let section = extract_section(content, "Log");
    if section.is_empty() {
        return Vec::new();
    }

    // Regexes for the various legacy log formats
    let bracket_ts_re = Regex::new(r"^- \[(\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2})\](.*)$").unwrap();
    let bold_ts_re = Regex::new(r"^- \*\*(\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2})\*\*(.*)$").unwrap();
    let time_re = Regex::new(r"^- \*\*(\d{2}:\d{2})\*\*(.*)$").unwrap();
    let date_re = Regex::new(r"^### (\d{4}-\d{2}-\d{2})$").unwrap();

    let mut entries = Vec::new();
    let mut current_date = String::new();

    for line in section.lines() {
        if let Some(caps) = date_re.captures(line) {
            current_date = caps[1].to_string();
            continue;
        }

        if let Some(caps) = bracket_ts_re.captures(line) {
            entries.push(LogEntry {
                ts: caps[1].to_string(),
                text: caps[2].trim().to_string(),
            });
        } else if let Some(caps) = bold_ts_re.captures(line) {
            entries.push(LogEntry {
                ts: caps[1].to_string(),
                text: caps[2].trim().to_string(),
            });
        } else if let Some(caps) = time_re.captures(line) {
            let time = &caps[1];
            let text = caps[2].trim().to_string();
            let ts = if !current_date.is_empty() {
                format!("{} {}:00", current_date, time)
            } else {
                format!("1970-01-01 {}:00", time)
            };
            entries.push(LogEntry { ts, text });
        } else if let Some(content) = line.strip_prefix("- ") {
            // Plain bullet without timestamp - use placeholder ts
            entries.push(LogEntry {
                ts: String::new(),
                text: content.trim().to_string(),
            });
        }
    }

    entries
}

/// Remove item by hash from a markdown section
pub fn remove_by_hash_from_section(
    content: &str,
    section: &str,
    hash: &str,
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
            continue; // skip this line
        }
        result.push(line);
    }

    if !found {
        return Err(format!("no item with hash '{}' found", hash));
    }

    Ok(result.join("\n"))
}

/// Edit item by hash in a markdown section
pub fn edit_by_hash_from_section(
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

/// Set todo item checked state by hash in a markdown section
pub fn set_todo_checked_from_section(
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

/// Count items matching a hash prefix in a markdown section
pub fn count_matching_items_from_section(content: &str, section: &str, hash: &str) -> usize {
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

/// Strip old Notes/Todo/Log sections from a markdown body string.
/// Returns the stripped content with trailing whitespace trimmed.
/// Used by the migrate command.
pub fn strip_old_sections(body: &str) -> String {
    let sections_to_strip = ["Notes", "Todo", "Log"];
    let mut result_lines: Vec<&str> = Vec::new();
    let mut in_stripped_section = false;

    for line in body.lines() {
        if let Some(section_name) = line.strip_prefix("## ") {
            let section_name = section_name.trim();
            if sections_to_strip.contains(&section_name) {
                in_stripped_section = true;
                continue;
            } else {
                in_stripped_section = false;
            }
        }

        if !in_stripped_section {
            result_lines.push(line);
        }
    }

    // Trim trailing empty lines
    while result_lines
        .last()
        .map(|l| l.trim().is_empty())
        .unwrap_or(false)
    {
        result_lines.pop();
    }

    if result_lines.is_empty() {
        String::new()
    } else {
        result_lines.join("\n") + "\n"
    }
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
        // Body section contains ## headers that should NOT be treated as section boundaries.
        // With the new format, ## headers in Body are no longer bumped to ###.
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

        // h2 headers in Body are now preserved as h2 (not bumped to h3)
        assert!(
            body.contains("## Subsection One"),
            "h2 in Body should be preserved as h2, got: {}",
            body
        );
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

    // ========================================================================
    // Frontmatter-based item tests
    // ========================================================================

    fn make_thread_with_content(content: &str) -> Thread {
        // Parse from a temp-like content without going to disk
        let mut t = Thread {
            path: "test.md".to_string(),
            frontmatter: Frontmatter::default(),
            content: content.to_string(),
            body_start: 0,
        };
        t.parse_frontmatter().expect("parse_frontmatter failed");
        t
    }

    #[test]
    fn test_frontmatter_round_trip_with_items() {
        let content = r#"---
id: abc123
name: Test
status: active
notes:
  - text: My note
    hash: a1b2
todo:
  - text: Do the thing
    hash: c3d4
    done: false
  - text: Done item
    hash: e5f6
    done: true
log:
  - ts: '2026-02-23 14:30:00'
    text: Created
---

## Body

Some body content.
"#;

        let t = make_thread_with_content(content);

        assert_eq!(t.frontmatter.notes.len(), 1);
        assert_eq!(t.frontmatter.notes[0].text, "My note");
        assert_eq!(t.frontmatter.notes[0].hash, "a1b2");

        assert_eq!(t.frontmatter.todo.len(), 2);
        assert_eq!(t.frontmatter.todo[0].text, "Do the thing");
        assert!(!t.frontmatter.todo[0].done);
        assert!(t.frontmatter.todo[1].done);

        assert_eq!(t.frontmatter.log.len(), 1);
        assert_eq!(t.frontmatter.log[0].ts, "2026-02-23 14:30:00");
        assert_eq!(t.frontmatter.log[0].text, "Created");
    }

    #[test]
    fn test_get_notes_reads_from_frontmatter() {
        let content = r#"---
id: abc123
name: Test
status: active
notes:
  - text: Frontmatter note
    hash: ff00
---
"#;

        let t = make_thread_with_content(content);
        let notes = t.get_notes();
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].text, "Frontmatter note");
        assert_eq!(notes[0].hash, "ff00");
    }

    #[test]
    fn test_get_notes_falls_back_to_section() {
        let content = r#"---
id: abc123
name: Test
status: active
---

## Notes

- Section note  <!-- a1b2 -->
- Another note  <!-- c3d4 -->

## Log

- [2026-01-01 12:00:00] Entry
"#;

        let t = make_thread_with_content(content);
        let notes = t.get_notes();
        assert_eq!(notes.len(), 2);
        assert_eq!(notes[0].text, "Section note");
        assert_eq!(notes[0].hash, "a1b2");
        assert_eq!(notes[1].text, "Another note");
        assert_eq!(notes[1].hash, "c3d4");
    }

    #[test]
    fn test_get_todo_items_from_section_fallback() {
        let content = r#"---
id: abc123
name: Test
status: active
---

## Todo

- [ ] Unchecked  <!-- a1b2 -->
- [x] Checked  <!-- c3d4 -->
"#;

        let t = make_thread_with_content(content);
        let items = t.get_todo_items();
        assert_eq!(items.len(), 2);
        assert!(!items[0].done);
        assert_eq!(items[0].text, "Unchecked");
        assert!(items[1].done);
        assert_eq!(items[1].text, "Checked");
    }

    #[test]
    fn test_add_note_writes_to_frontmatter() {
        let content = r#"---
id: abc123
name: Test
status: active
---

## Body

Some body.
"#;

        let mut t = make_thread_with_content(content);
        let hash = t.add_note("New note").expect("add_note failed");

        // Hash is 4-char hex
        assert_eq!(hash.len(), 4);

        // frontmatter updated
        assert_eq!(t.frontmatter.notes.len(), 1);
        assert_eq!(t.frontmatter.notes[0].text, "New note");
        assert_eq!(t.frontmatter.notes[0].hash, hash);

        // Content rebuilt with notes in YAML
        assert!(
            t.content.contains("notes:"),
            "content should contain notes:"
        );
        assert!(
            t.content.contains("New note"),
            "content should contain note text"
        );

        // Body still present
        assert!(t.content.contains("Some body."), "body should be preserved");
    }

    #[test]
    fn test_add_todo_writes_to_frontmatter() {
        let content = r#"---
id: abc123
name: Test
status: active
---
"#;

        let mut t = make_thread_with_content(content);
        let hash = t
            .add_todo_item("Do something")
            .expect("add_todo_item failed");

        assert_eq!(t.frontmatter.todo.len(), 1);
        assert_eq!(t.frontmatter.todo[0].text, "Do something");
        assert!(!t.frontmatter.todo[0].done);
        assert_eq!(t.frontmatter.todo[0].hash, hash);
    }

    #[test]
    fn test_insert_log_entry_writes_to_frontmatter() {
        let content = r#"---
id: abc123
name: Test
status: active
---
"#;

        let mut t = make_thread_with_content(content);
        t.insert_log_entry("Did a thing")
            .expect("insert_log_entry failed");

        assert_eq!(t.frontmatter.log.len(), 1);
        assert_eq!(t.frontmatter.log[0].text, "Did a thing");
        assert!(!t.frontmatter.log[0].ts.is_empty());
    }

    #[test]
    fn test_set_todo_checked_frontmatter() {
        let content = r#"---
id: abc123
name: Test
status: active
todo:
  - text: Task
    hash: a1b2
    done: false
---
"#;

        let mut t = make_thread_with_content(content);
        t.set_todo_checked("a1b2", true)
            .expect("set_todo_checked failed");
        assert!(t.frontmatter.todo[0].done);

        t.set_todo_checked("a1b2", false)
            .expect("set_todo_checked failed");
        assert!(!t.frontmatter.todo[0].done);
    }

    #[test]
    fn test_remove_note_by_hash_frontmatter() {
        let content = r#"---
id: abc123
name: Test
status: active
notes:
  - text: Note one
    hash: a1b2
  - text: Note two
    hash: c3d4
---
"#;

        let mut t = make_thread_with_content(content);
        t.remove_by_hash("Notes", "a1b2")
            .expect("remove_by_hash failed");

        assert_eq!(t.frontmatter.notes.len(), 1);
        assert_eq!(t.frontmatter.notes[0].text, "Note two");
    }

    #[test]
    fn test_edit_note_by_hash_frontmatter() {
        let content = r#"---
id: abc123
name: Test
status: active
notes:
  - text: Original text
    hash: a1b2
---
"#;

        let mut t = make_thread_with_content(content);
        t.edit_by_hash("Notes", "a1b2", "Updated text")
            .expect("edit_by_hash failed");

        assert_eq!(t.frontmatter.notes[0].text, "Updated text");
        assert_eq!(t.frontmatter.notes[0].hash, "a1b2");
    }

    #[test]
    fn test_count_matching_items_frontmatter() {
        let content = r#"---
id: abc123
name: Test
status: active
notes:
  - text: Note one
    hash: a1b2
  - text: Note two
    hash: a1c3
  - text: Note three
    hash: ff00
---
"#;

        let t = make_thread_with_content(content);

        // Exact match
        assert_eq!(t.count_matching_items("Notes", "a1b2"), 1);
        // Prefix match
        assert_eq!(t.count_matching_items("Notes", "a1"), 2);
        // No match
        assert_eq!(t.count_matching_items("Notes", "zzzz"), 0);
    }

    #[test]
    fn test_rebuild_content_updates_body_start() {
        let content = r#"---
id: abc123
name: Test
status: active
---

Body here.
"#;

        let mut t = make_thread_with_content(content);
        let original_body_start = t.body_start;

        // Add a note (causes rebuild)
        t.add_note("New note").expect("add_note failed");
        let new_body_start = t.body_start;

        // body_start should have changed (YAML is now longer)
        assert!(
            new_body_start > original_body_start,
            "body_start should increase when frontmatter grows"
        );

        // Body content should be preserved at the new position
        let body = &t.content[t.body_start..];
        assert!(body.contains("Body here."), "body preserved after rebuild");

        // Add log entry (second rebuild)
        t.insert_log_entry("test entry")
            .expect("insert_log_entry failed");
        let body2 = &t.content[t.body_start..];
        assert!(
            body2.contains("Body here."),
            "body preserved after second rebuild"
        );
    }

    #[test]
    fn test_rebuild_content_no_blank_line_accumulation() {
        let content = "---\nid: abc123\nname: Test\nstatus: active\n---\n\nBody here.\n";
        let mut t = make_thread_with_content(content);

        // Simulate 5 mutation cycles (each mutates frontmatter twice: item + log)
        for i in 0..5 {
            t.add_note(&format!("note {}", i)).expect("add_note failed");
            t.insert_log_entry(&format!("log {}", i))
                .expect("insert_log_entry failed");
        }

        // Count leading newlines in body (after "---\n" closing)
        let body = &t.content[t.body_start..];
        let leading_newlines = body.chars().take_while(|&c| c == '\n').count();
        assert_eq!(
            leading_newlines, 1,
            "body should have exactly one leading newline after 5 rebuild cycles, got {}",
            leading_newlines
        );
        assert!(body.contains("Body here."), "body content preserved");
    }

    #[test]
    fn test_get_log_entries_from_section() {
        let content = r#"---
id: abc123
name: Test
status: active
---

## Log

- [2026-02-23 14:30:00] Created
- [2026-01-15 09:00:00] Updated
"#;

        let t = make_thread_with_content(content);
        let entries = t.get_log_entries();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].ts, "2026-02-23 14:30:00");
        assert_eq!(entries[0].text, "Created");
        assert_eq!(entries[1].ts, "2026-01-15 09:00:00");
        assert_eq!(entries[1].text, "Updated");
    }

    #[test]
    fn test_strip_old_sections() {
        let body = r#"

## Body

Some content here.

## Notes

- Note one  <!-- a1b2 -->

## Todo

- [ ] Task  <!-- c3d4 -->

## Log

- [2026-02-23 12:00:00] Entry
"#;

        let stripped = strip_old_sections(body);

        assert!(
            stripped.contains("Some content here."),
            "Body content preserved"
        );
        assert!(!stripped.contains("Note one"), "Notes removed");
        assert!(!stripped.contains("Task"), "Todo removed");
        assert!(!stripped.contains("Entry"), "Log removed");
        assert!(!stripped.contains("## Notes"), "Notes header removed");
        assert!(!stripped.contains("## Todo"), "Todo header removed");
        assert!(!stripped.contains("## Log"), "Log header removed");
    }

    #[test]
    fn test_strip_old_sections_body_only() {
        let body = r#"## Body

Just body content.
"#;

        let stripped = strip_old_sections(body);
        assert!(stripped.contains("Just body content."), "Body preserved");
        assert!(stripped.contains("## Body"), "Body header preserved");
    }

    #[test]
    fn test_migration_idempotent() {
        // A thread that has already been migrated (items in frontmatter, no sections)
        let content = r#"---
id: abc123
name: Test
status: active
notes:
  - text: Already migrated note
    hash: a1b2
---

## Body

Some content.
"#;

        let t = make_thread_with_content(content);

        // get_notes should return from frontmatter, not parse empty section
        let notes = t.get_notes();
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].text, "Already migrated note");

        // Section extraction should return empty (no section exists)
        let section_notes = extract_section(&t.content, "Notes");
        assert!(
            section_notes.is_empty(),
            "No Notes section in migrated file"
        );
    }
}

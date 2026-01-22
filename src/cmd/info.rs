use std::fs;
use std::path::Path;
use std::process::Command;

use chrono::{DateTime, Local, Utc};
use clap::Args;
use clap_complete::engine::ArgValueCompleter;
use colored::Colorize;
use serde::Serialize;
use tabled::settings::object::Columns;
use tabled::settings::{Alignment, Modify, Padding, Style};
use tabled::settings::style::HorizontalLine;
use tabled::Table;

use crate::output::{self, OutputFormat};
use crate::thread::Thread;
use crate::workspace;

#[derive(Args)]
pub struct InfoArgs {
    /// Thread ID or name reference
    #[arg(add = ArgValueCompleter::new(crate::workspace::complete_thread_ids))]
    id: String,

    /// Output format
    #[arg(short = 'f', long, value_enum, default_value = "pretty")]
    format: OutputFormat,

    /// Output as JSON (shorthand for --format=json)
    #[arg(long, conflicts_with = "format")]
    json: bool,
}

/// Git log entry with diff stats
#[derive(Clone)]
struct GitLogEntry {
    relative_time: String,
    hash: String,
    message: String,
    insertions: usize,
    deletions: usize,
}

impl std::fmt::Display for GitLogEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {} {}", self.relative_time, self.hash, self.message)
    }
}

/// Thread info data
struct ThreadInfoData {
    id: String,
    status: String,
    path: String,
    path_absolute: String,
    name: String,
    title: String,
    desc: String,
    created_dt: Option<DateTime<Local>>,
    updated_dt: Option<DateTime<Local>>,
    git_status: String,
    log_count: usize,
    todo_count: usize,
    todo_done: usize,
    git_history: Vec<GitLogEntry>,
}

impl ThreadInfoData {
    fn created_plain(&self) -> String {
        self.created_dt
            .map(|dt| dt.format("%Y-%m-%d").to_string())
            .unwrap_or_else(|| "?".to_string())
    }

    fn updated_plain(&self) -> String {
        self.updated_dt
            .map(|dt| dt.format("%Y-%m-%d").to_string())
            .unwrap_or_else(|| "?".to_string())
    }

    fn created_iso(&self) -> String {
        self.created_dt
            .map(|dt| dt.with_timezone(&Utc).to_rfc3339())
            .unwrap_or_default()
    }

    fn updated_iso(&self) -> String {
        self.updated_dt
            .map(|dt| dt.with_timezone(&Utc).to_rfc3339())
            .unwrap_or_default()
    }
}

pub fn run(args: InfoArgs, ws: &Path) -> Result<(), String> {
    let format = if args.json {
        OutputFormat::Json
    } else {
        args.format.resolve()
    };

    let file = workspace::find_by_ref(ws, &args.id)?;
    let thread = Thread::parse(&file)?;

    let rel_path = file
        .strip_prefix(ws)
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| file.to_string_lossy().to_string());

    let title = if !thread.name().is_empty() {
        thread.name().to_string()
    } else {
        crate::thread::extract_name_from_path(&file).replace('-', " ")
    };

    let (created_dt, updated_dt) = get_timestamps(&file);
    let git_status = get_git_status(ws, &rel_path);
    let (log_count, todo_count, todo_done) = count_items(&thread.content);
    let git_history = get_git_history(ws, &rel_path);

    let info = ThreadInfoData {
        id: thread.id().to_string(),
        status: thread.base_status(),
        path: rel_path,
        path_absolute: file.to_string_lossy().to_string(),
        name: crate::thread::extract_name_from_path(&file),
        title,
        desc: thread.frontmatter.desc.clone(),
        created_dt,
        updated_dt,
        git_status,
        log_count,
        todo_count,
        todo_done,
        git_history,
    };

    match format {
        OutputFormat::Pretty => output_pretty(&info),
        OutputFormat::Plain => output_plain(&info),
        OutputFormat::Json => output_json(&info),
        OutputFormat::Yaml => output_yaml(&info),
    }
}

fn output_pretty(info: &ThreadInfoData) -> Result<(), String> {
    let term_width = output::terminal_width().min(80).saturating_sub(4); // account for box borders + padding

    // Right side stats: log · todos · status (no symbols)
    let todo_text = if info.todo_count == 0 {
        "0".dimmed().to_string()
    } else if info.todo_done == info.todo_count {
        format!("{}/{} ✓", info.todo_done, info.todo_count)
            .green()
            .to_string()
    } else if info.todo_done > 0 {
        format!("{}/{}", info.todo_done, info.todo_count)
            .yellow()
            .to_string()
    } else {
        format!("0/{}", info.todo_count)
    };

    let status_styled = output::style_status(&info.status).to_string();
    let right_side = format!("{} · {} · {}", info.log_count, todo_text, status_styled);

    // Title line: title followed by stats (no HFILL - table handles width)
    let title = info.title.bold().to_string();
    let title_line = format!("{}  {}", title, right_side);

    // Description (not bold)
    let desc_line = info.desc.clone();

    // Build header content
    let header_content = if desc_line.is_empty() {
        title_line
    } else {
        format!("{}\n{}", title_line, desc_line)
    };

    // Build history section with diff stats
    let history_header = "History".bold().to_string();
    let history_lines: Vec<String> = if info.git_history.is_empty() {
        vec!["No commits (untracked)".dimmed().to_string()]
    } else {
        let max_entries = 5;
        let total = info.git_history.len();
        let mut lines: Vec<String> = info.git_history
            .iter()
            .take(max_entries)
            .map(|entry| format_git_entry(entry, term_width))
            .collect();

        if total > max_entries {
            lines.push(format!("... {} more commits", total - max_entries).dimmed().to_string());
        }
        lines
    };
    let history_content = format!("{}\n{}", history_header, history_lines.join("\n"));

    // Path line (grey, no separator before)
    let path_line = info.path.dimmed().to_string();

    // Construct table: header, history+path (combined to avoid separator before path)
    let history_and_path = format!("{}\n\n{}", history_content, path_line);

    let rows: Vec<Vec<String>> = vec![
        vec![header_content],
        vec![history_and_path],
    ];

    let mut table = Table::from_iter(rows);

    // Single horizontal line after header
    let hline = HorizontalLine::new('─').left('├').right('┤').intersection('─');
    let style = Style::rounded().horizontals([(1, hline)]);

    table
        .with(style)
        .with(Padding::new(1, 1, 0, 0))
        .with(Modify::new(Columns::single(0)).with(Alignment::left()));

    println!("{}", table);

    Ok(())
}

/// Format a git log entry: "3h  abc1234 +5 -2  commit message"
fn format_git_entry(entry: &GitLogEntry, max_width: usize) -> String {
    let time_str = format!("{:>3}", entry.relative_time);
    let hash_str = output::style_id(&entry.hash).to_string();

    // Format diff stats with colors
    let diff_str = if entry.insertions > 0 || entry.deletions > 0 {
        let ins = if entry.insertions > 0 {
            format!("+{}", entry.insertions).green().to_string()
        } else {
            String::new()
        };
        let del = if entry.deletions > 0 {
            format!("-{}", entry.deletions).red().to_string()
        } else {
            String::new()
        };
        if !ins.is_empty() && !del.is_empty() {
            format!("{} {}", ins, del)
        } else {
            format!("{}{}", ins, del)
        }
    } else {
        String::new()
    };

    // Calculate visible lengths for proper spacing
    let time_visible = entry.relative_time.len().max(3);
    let hash_visible = 7;
    let diff_visible = if entry.insertions > 0 { 1 + entry.insertions.to_string().len() } else { 0 }
        + if entry.deletions > 0 { 1 + entry.deletions.to_string().len() } else { 0 }
        + if entry.insertions > 0 && entry.deletions > 0 { 1 } else { 0 };

    // Space needed: time(3) + space(2) + hash(7) + space(1) + diff + space(2) + message
    let prefix_len = time_visible + 2 + hash_visible + 1;
    let diff_space = if diff_visible > 0 { diff_visible + 2 } else { 1 };
    let msg_max = max_width.saturating_sub(prefix_len + diff_space);
    let message = output::truncate_back(&entry.message, msg_max);

    if diff_str.is_empty() {
        format!("{} {} {}", time_str.dimmed(), hash_str, message)
    } else {
        format!("{} {} {} {}", time_str.dimmed(), hash_str, diff_str, message)
    }
}

fn output_plain(info: &ThreadInfoData) -> Result<(), String> {
    // Title with status
    println!("{} [{}]", info.title, info.status);
    if !info.desc.is_empty() {
        println!("{}", info.desc);
    }
    println!();

    // Dates - show updated only if different
    let dates_same = info.created_plain() == info.updated_plain();
    if dates_same {
        print!("Created {}", info.created_plain());
    } else {
        print!("Created {} | updated {}", info.created_plain(), info.updated_plain());
    }
    // Git status only if not clean
    if info.git_status != "clean" {
        println!(" | {}", info.git_status);
    } else {
        println!();
    }
    println!();

    // Stats with proper pluralization
    let log_word = if info.log_count == 1 { "entry" } else { "entries" };
    let todo_display = if info.todo_count == 0 {
        "no todos".to_string()
    } else {
        format!("{}/{} todos", info.todo_done, info.todo_count)
    };
    println!("{} log {} | {}", info.log_count, log_word, todo_display);
    println!();

    // History
    if info.git_history.is_empty() {
        println!("No history (untracked)");
    } else {
        println!("History:");
        for line in &info.git_history {
            println!("  {}", line);
        }
    }
    println!();

    // Path last (with ID for reference)
    println!("{} | {}", info.id, info.path);

    Ok(())
}

fn output_json(info: &ThreadInfoData) -> Result<(), String> {
    #[derive(Serialize)]
    struct JsonInfo {
        id: String,
        status: String,
        path: String,
        path_absolute: String,
        name: String,
        title: String,
        desc: String,
        created: String,
        updated: String,
        git_status: String,
        log_count: usize,
        todo_count: usize,
        todo_done: usize,
        git_history: Vec<String>,
    }

    // Convert GitLogEntry to strings for JSON output
    let history_strings: Vec<String> = info.git_history.iter().map(|e| e.to_string()).collect();

    let output = JsonInfo {
        id: info.id.clone(),
        status: info.status.clone(),
        path: info.path.clone(),
        path_absolute: info.path_absolute.clone(),
        name: info.name.clone(),
        title: info.title.clone(),
        desc: info.desc.clone(),
        created: info.created_iso(),
        updated: info.updated_iso(),
        git_status: info.git_status.clone(),
        log_count: info.log_count,
        todo_count: info.todo_count,
        todo_done: info.todo_done,
        git_history: history_strings,
    };

    let json = serde_json::to_string_pretty(&output)
        .map_err(|e| format!("JSON serialization failed: {}", e))?;
    println!("{}", json);
    Ok(())
}

fn output_yaml(info: &ThreadInfoData) -> Result<(), String> {
    #[derive(Serialize)]
    struct YamlInfo {
        id: String,
        status: String,
        path: String,
        path_absolute: String,
        name: String,
        title: String,
        desc: String,
        created: String,
        updated: String,
        git_status: String,
        log_count: usize,
        todo_count: usize,
        todo_done: usize,
        git_history: Vec<String>,
    }

    // Convert GitLogEntry to strings for YAML output
    let history_strings: Vec<String> = info.git_history.iter().map(|e| e.to_string()).collect();

    let output = YamlInfo {
        id: info.id.clone(),
        status: info.status.clone(),
        path: info.path.clone(),
        path_absolute: info.path_absolute.clone(),
        name: info.name.clone(),
        title: info.title.clone(),
        desc: info.desc.clone(),
        created: info.created_iso(),
        updated: info.updated_iso(),
        git_status: info.git_status.clone(),
        log_count: info.log_count,
        todo_count: info.todo_count,
        todo_done: info.todo_done,
        git_history: history_strings,
    };

    let yaml =
        serde_yaml::to_string(&output).map_err(|e| format!("YAML serialization failed: {}", e))?;
    print!("{}", yaml);
    Ok(())
}

fn get_timestamps(path: &Path) -> (Option<DateTime<Local>>, Option<DateTime<Local>>) {
    let metadata = match fs::metadata(path) {
        Ok(m) => m,
        Err(_) => return (None, None),
    };

    let updated: Option<DateTime<Local>> = metadata.modified().ok().map(|t| t.into());
    let created: Option<DateTime<Local>> = metadata.created().ok().map(|t| t.into()).or(updated);

    (created, updated)
}

fn get_git_status(ws: &Path, rel_path: &str) -> String {
    let output = Command::new("git")
        .args([
            "-C",
            &ws.to_string_lossy(),
            "status",
            "--porcelain",
            "--",
            rel_path,
        ])
        .output();

    let output = match output {
        Ok(o) if o.status.success() => o,
        _ => return "unknown".to_string(),
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let line = stdout.trim();

    if line.is_empty() {
        return "clean".to_string();
    }

    if line.len() >= 2 {
        let index = line.chars().next().unwrap_or(' ');
        let worktree = line.chars().nth(1).unwrap_or(' ');

        match (index, worktree) {
            ('?', '?') => "untracked".to_string(),
            ('A', _) => "staged (new)".to_string(),
            ('M', ' ') => "staged".to_string(),
            (' ', 'M') => "modified".to_string(),
            ('M', 'M') => "staged + modified".to_string(),
            ('D', _) | (_, 'D') => "deleted".to_string(),
            _ => format!("changed ({}{})", index, worktree),
        }
    } else {
        "changed".to_string()
    }
}

fn count_items(content: &str) -> (usize, usize, usize) {
    let mut log_count = 0;
    let mut todo_count = 0;
    let mut todo_done = 0;
    let mut in_log = false;
    let mut in_todo = false;

    for line in content.lines() {
        if line.starts_with("## Log") {
            in_log = true;
            in_todo = false;
        } else if line.starts_with("## Todo") {
            in_todo = true;
            in_log = false;
        } else if line.starts_with("## ") {
            in_log = false;
            in_todo = false;
        }

        if in_log && line.starts_with("- **") {
            log_count += 1;
        }

        if in_todo {
            if line.starts_with("- [ ]") {
                todo_count += 1;
            } else if line.starts_with("- [x]") {
                todo_count += 1;
                todo_done += 1;
            }
        }
    }

    (log_count, todo_count, todo_done)
}

fn get_git_history(ws: &Path, rel_path: &str) -> Vec<GitLogEntry> {
    // Get commits with relative time, hash, message, and numstat for diff
    // Format: relative_time<TAB>hash<TAB>message
    let output = Command::new("git")
        .args([
            "-C",
            &ws.to_string_lossy(),
            "log",
            "--format=%cr\t%h\t%s",
            "--numstat",
            "--follow",
            "-10",
            "--",
            rel_path,
        ])
        .output();

    let output = match output {
        Ok(o) if o.status.success() => o,
        _ => return Vec::new(),
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut entries = Vec::new();
    let mut current_entry: Option<GitLogEntry> = None;

    for line in stdout.lines() {
        if line.is_empty() {
            continue;
        }

        let parts: Vec<&str> = line.split('\t').collect();

        // Commit lines have 3+ parts where second part is a 7-char hash (hex)
        // Numstat lines have 3 parts where first two are numbers
        if parts.len() >= 3 {
            // Check if this is a numstat line (first two parts are numbers)
            if let (Ok(ins), Ok(del)) = (parts[0].parse::<usize>(), parts[1].parse::<usize>()) {
                // This is a numstat line
                if let Some(ref mut entry) = current_entry {
                    entry.insertions += ins;
                    entry.deletions += del;
                }
            } else {
                // This is a commit line: relative_time<TAB>hash<TAB>message
                // Save previous entry if exists
                if let Some(entry) = current_entry.take() {
                    entries.push(entry);
                }

                // Shorten relative time: "3 hours ago" -> "3h", "2 days ago" -> "2d"
                let rel_time = shorten_relative_time(parts[0]);

                current_entry = Some(GitLogEntry {
                    relative_time: rel_time,
                    hash: parts[1].to_string(),
                    message: parts[2..].join("\t"),
                    insertions: 0,
                    deletions: 0,
                });
            }
        }
    }

    // Don't forget the last entry
    if let Some(entry) = current_entry {
        entries.push(entry);
    }

    entries
}

/// Shorten git's relative time: "3 hours ago" -> "3h", "2 days ago" -> "2d"
fn shorten_relative_time(s: &str) -> String {
    let s = s.trim();
    if s.contains("second") {
        "now".to_string()
    } else if let Some(n) = s.strip_suffix(" minutes ago").or(s.strip_suffix(" minute ago")) {
        format!("{}m", n)
    } else if let Some(n) = s.strip_suffix(" hours ago").or(s.strip_suffix(" hour ago")) {
        format!("{}h", n)
    } else if let Some(n) = s.strip_suffix(" days ago").or(s.strip_suffix(" day ago")) {
        format!("{}d", n)
    } else if let Some(n) = s.strip_suffix(" weeks ago").or(s.strip_suffix(" week ago")) {
        format!("{}w", n)
    } else if let Some(n) = s.strip_suffix(" months ago").or(s.strip_suffix(" month ago")) {
        format!("{}mo", n)
    } else if let Some(n) = s.strip_suffix(" years ago").or(s.strip_suffix(" year ago")) {
        format!("{}y", n)
    } else {
        s.to_string()
    }
}

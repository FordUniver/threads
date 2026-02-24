use std::fs;
use std::path::Path;
use std::process::Command;

use chrono::{Local, NaiveDateTime};
use clap::Args;
use clap_complete::engine::ArgValueCompleter;
use colored::Colorize;
use serde::Serialize;
use termimad::MadSkin;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::args::FormatArgs;
use crate::git;
use crate::output::{self, OutputFormat};
use crate::thread::{self, DeadlineItem, EventItem, LogEntry, NoteItem, Thread, TodoItem};
use crate::workspace;

#[derive(Args)]
pub struct ReadArgs {
    /// Thread ID or name reference
    #[arg(add = ArgValueCompleter::new(crate::workspace::complete_thread_ids))]
    id: String,

    #[command(flatten)]
    format: FormatArgs,

    /// Override terminal width (for testing)
    #[arg(long, hide = true)]
    width: Option<usize>,

    /// Debug: print width calculations
    #[arg(long, hide = true)]
    debug_widths: bool,
}

pub fn run(args: ReadArgs, ws: &Path) -> Result<(), String> {
    let file = workspace::find_by_ref(ws, &args.id)?;
    let content = fs::read_to_string(&file).map_err(|e| format!("reading file: {}", e))?;

    let format = args.format.resolve();

    match format {
        OutputFormat::Pretty => {
            output_pretty(&file, ws, args.width, args.debug_widths)?;
        }
        OutputFormat::Plain => {
            // Plain: raw markdown content
            print!("{}", content);
        }
        OutputFormat::Json | OutputFormat::Yaml => {
            let thread = Thread::parse(&file)?;
            let rel_path = file
                .strip_prefix(ws)
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| file.to_string_lossy().to_string());

            output_structured(&thread, &rel_path, &content, format)?;
        }
    }
    Ok(())
}

/// Structured output data for JSON/YAML
#[derive(Serialize)]
struct ThreadOutput {
    id: String,
    name: String,
    status: String,
    desc: String,
    path: String,
    body: String,
    notes: Vec<NoteItem>,
    todo: Vec<TodoItem>,
    log: Vec<LogEntry>,
    deadlines: Vec<DeadlineItem>,
    events: Vec<EventItem>,
    raw: String,
}

/// Output thread as JSON or YAML
fn output_structured(
    thread: &Thread,
    rel_path: &str,
    raw_content: &str,
    format: OutputFormat,
) -> Result<(), String> {
    let output = ThreadOutput {
        id: thread.frontmatter.id.clone(),
        name: thread.name().to_string(),
        status: thread.frontmatter.status.clone(),
        desc: thread.frontmatter.desc.clone(),
        path: rel_path.to_string(),
        body: thread.content[thread.body_start..].trim().to_string(),
        notes: thread.get_notes(),
        todo: thread.get_todo_items(),
        log: thread.get_log_entries(),
        deadlines: thread.get_deadlines(),
        events: thread.get_events(),
        raw: raw_content.to_string(),
    };

    match format {
        OutputFormat::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(&output).map_err(|e| format!("JSON error: {}", e))?
            );
        }
        OutputFormat::Yaml => {
            print!(
                "{}",
                serde_yaml::to_string(&output).map_err(|e| format!("YAML error: {}", e))?
            );
        }
        _ => unreachable!(),
    }

    Ok(())
}

/// Rich pretty output - single box with sections separated by horizontal lines
fn output_pretty(
    file: &Path,
    ws: &Path,
    width_override: Option<usize>,
    debug: bool,
) -> Result<(), String> {
    let thread = Thread::parse(file)?;
    let term_width = width_override.unwrap_or_else(|| output::terminal_width().min(100));

    if debug {
        eprintln!("DEBUG: term_width={}", term_width);
    }

    let rel_path = file
        .strip_prefix(ws)
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| file.to_string_lossy().to_string());

    // Get git history
    let git_history = get_git_history(ws, &rel_path);

    // === Section 1: Header ===
    let title = if !thread.name().is_empty() {
        thread.name().to_string()
    } else {
        thread::extract_name_from_path(file).replace('-', " ")
    };

    let status_styled = output::style_status(&thread.base_status()).to_string();

    // Get git status with diff stats if dirty
    let git_info = if let Ok(repo) = workspace::open() {
        let rel_path_for_git = file.strip_prefix(ws).unwrap_or(file);
        let file_status = git::file_status(&repo, rel_path_for_git);
        if file_status != git::FileStatus::Clean && file_status != git::FileStatus::Unknown {
            if let Some((ins, del)) = git::diff_stats(&repo, rel_path_for_git) {
                format!(" · {} (+{}/-{})", file_status, ins, del)
            } else {
                format!(" · {}", file_status)
            }
        } else {
            String::new()
        }
    } else {
        String::new()
    };

    // Title line: truncate if needed (title + status + git info on one line)
    let inner_width = term_width.saturating_sub(4);
    let status_git_len = strip_ansi_width(&status_styled) + strip_ansi_width(&git_info) + 2; // "  " separator
    let title_max = inner_width.saturating_sub(status_git_len);
    let title_truncated = output::truncate_back(&title, title_max);

    let title_line = format!(
        "{}  {}{}",
        title_truncated.cyan().bold(),
        status_styled,
        git_info.dimmed()
    );

    // Description: wrap to fit box width
    let header = if thread.frontmatter.desc.is_empty() {
        title_line
    } else {
        let desc_wrapped = crate::wrap::wrap(&thread.frontmatter.desc, inner_width);
        format!("{}\n{}", title_line, desc_wrapped.join("\n"))
    };

    // === Extract body and structured items ===
    let body = thread.content[thread.body_start..].trim().to_string();
    let notes_items = thread.get_notes();
    let todo_items = thread.get_todo_items();
    let deadline_items = thread.get_deadlines();
    let event_items = thread.get_events();
    let log_entries = thread.get_log_entries();

    // === Build sections dynamically ===
    let mut sections: Vec<String> = vec![header];

    if !body.is_empty() {
        sections.push(format_body(&body));
    }
    if !notes_items.is_empty() {
        sections.push(format_notes(&notes_items));
    }
    if !todo_items.is_empty() {
        sections.push(format_todos(&todo_items));
    }
    if !deadline_items.is_empty() {
        sections.push(format_deadlines(&deadline_items));
    }
    if !event_items.is_empty() {
        sections.push(format_events(&event_items));
    }
    if !log_entries.is_empty() {
        sections.push(format_log(&log_entries));
    }

    // Footer: history + path (truncate path from front if too long)
    let history_content = format_history(&git_history, inner_width);
    let path_truncated = output::truncate_front(&rel_path, inner_width);
    sections.push(format!(
        "{}\n\n{}",
        history_content,
        path_truncated.dimmed()
    ));

    // === Render box with sections ===
    print_boxed_sections(&sections, term_width, debug);

    Ok(())
}

/// Print sections in a rounded box with horizontal separators
fn print_boxed_sections(sections: &[String], max_width: usize, debug: bool) {
    let inner_width = max_width.saturating_sub(4); // Account for "│ " and " │"

    if debug {
        eprintln!(
            "DEBUG: max_width={}, inner_width={}",
            max_width, inner_width
        );
    }

    // Top border
    println!("╭{}╮", "─".repeat(max_width - 2));

    for (i, section) in sections.iter().enumerate() {
        // Print section content with padding
        for (line_num, line) in section.lines().enumerate() {
            // Wrap or truncate long lines
            let wrapped_lines = wrap_line(line, inner_width);
            for wrapped in wrapped_lines {
                let visible_width = strip_ansi_width(&wrapped);
                let padding = inner_width.saturating_sub(visible_width);
                let total = 4 + visible_width + padding; // "│ " + content + padding + " │"

                if debug && total != max_width {
                    eprintln!(
                        "DEBUG: section={} line={}: visible_width={}, padding={}, total={} (expected {})",
                        i, line_num, visible_width, padding, total, max_width
                    );
                    eprintln!("DEBUG:   content: {:?}", &wrapped[..wrapped.len().min(50)]);
                }

                println!("│ {}{} │", wrapped, " ".repeat(padding));
            }
        }

        // Separator between sections (not after last)
        if i < sections.len() - 1 {
            println!("├{}┤", "─".repeat(max_width - 2));
        }
    }

    // Bottom border
    println!("╰{}╯", "─".repeat(max_width - 2));
}

/// Wrap a line to fit within max_width (respecting ANSI codes).
/// Detects common prefixes (bullets, checkboxes, timestamps) and indents continuation lines.
fn wrap_line(line: &str, max_width: usize) -> Vec<String> {
    use crate::wrap;

    let visible_width = strip_ansi_width(line);
    if visible_width <= max_width {
        return vec![line.to_string()];
    }

    // Detect prefix patterns and split into (prefix, content)
    // These patterns match the formatted output from format_notes/todos/log
    if let Some((prefix, content)) = detect_prefix(line) {
        return wrap::wrap_with_prefix(&prefix, &content, max_width);
    }

    // No recognized prefix - wrap with no indent
    wrap::wrap(line, max_width)
}

/// Detect common prefix patterns in formatted lines.
/// Returns (prefix_with_styling, remaining_content) if a pattern is found.
fn detect_prefix(line: &str) -> Option<(String, String)> {
    // Strip ANSI to find the visible pattern, but preserve codes in prefix
    let stripped = strip_ansi_to_string(line);

    // Notes: "• " (bullet + space)
    if stripped.starts_with("• ") {
        return split_at_visible_pos(line, 2);
    }

    // Todos: "☐ " or "☑ " (checkbox + space)
    if stripped.starts_with("☐ ") || stripped.starts_with("☑ ") {
        return split_at_visible_pos(line, 2);
    }

    // Log with timestamp: right-aligned to 4 chars + space = 5 chars total
    // Examples: " 39m ", "  1h ", " now ", "12mo "
    // Format from format_log: "{:>4} content" where timestamp is cyan-styled
    if stripped.len() >= 5 {
        let end = stripped
            .char_indices()
            .nth(5)
            .map(|(i, _)| i)
            .unwrap_or(stripped.len());
        let first_five = &stripped[..end];
        let trimmed = first_five.trim_start();
        // Check if trimmed part (without leading spaces) looks like timestamp + space
        if trimmed.ends_with(' ') {
            let ts_part = trimmed.trim_end();
            if ts_part.ends_with('m')
                || ts_part.ends_with('h')
                || ts_part.ends_with('d')
                || ts_part.ends_with('w')
                || ts_part.ends_with('y')
                || ts_part == "now"
                || ts_part.ends_with("mo")
            {
                // Prefix is 5 visible chars (4 for timestamp + 1 space)
                return split_at_visible_pos(line, 5);
            }
        }
    }

    // Log without timestamp: "   · " (3 spaces + dimmed dot + space = 5 chars)
    if stripped.starts_with("   · ") {
        return split_at_visible_pos(line, 5);
    }

    None
}

/// Split a string at a visible character position, preserving ANSI codes in both parts.
fn split_at_visible_pos(line: &str, visible_pos: usize) -> Option<(String, String)> {
    let mut prefix = String::new();
    let mut visible_count = 0;
    let mut in_escape = false;
    let mut split_byte_idx = 0;

    for (byte_idx, c) in line.char_indices() {
        if visible_count >= visible_pos {
            split_byte_idx = byte_idx;
            break;
        }

        prefix.push(c);

        if c == '\x1b' {
            in_escape = true;
        } else if in_escape {
            if c == 'm' {
                in_escape = false;
            }
        } else {
            visible_count += c.width().unwrap_or(0);
        }

        split_byte_idx = byte_idx + c.len_utf8();
    }

    if split_byte_idx < line.len() {
        Some((prefix, line[split_byte_idx..].to_string()))
    } else {
        None
    }
}

/// Strip ANSI codes and return the visible string
fn strip_ansi_to_string(s: &str) -> String {
    let mut visible = String::new();
    let mut in_escape = false;

    for c in s.chars() {
        if c == '\x1b' {
            in_escape = true;
        } else if in_escape {
            if c == 'm' {
                in_escape = false;
            }
        } else {
            visible.push(c);
        }
    }

    visible
}

/// Calculate visible width of a string, ignoring ANSI escape codes
fn strip_ansi_width(s: &str) -> usize {
    // Simple ANSI escape code stripper
    let mut visible = String::new();
    let mut in_escape = false;

    for c in s.chars() {
        if c == '\x1b' {
            in_escape = true;
        } else if in_escape {
            if c == 'm' {
                in_escape = false;
            }
        } else {
            visible.push(c);
        }
    }

    visible.width()
}

/// Format body section - render markdown
fn format_body(body: &str) -> String {
    let skin = MadSkin::default();
    let mut buf = Vec::new();
    skin.write_text_on(&mut buf, body).ok();
    String::from_utf8_lossy(&buf).trim().to_string()
}

/// Format notes items with bullet points
fn format_notes(notes: &[NoteItem]) -> String {
    notes
        .iter()
        .map(|item| {
            let rendered = render_inline_markdown(&item.text);
            format!("{} {}", "•".cyan(), rendered)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Format todo items with colored checkboxes and markdown
fn format_todos(todos: &[TodoItem]) -> String {
    todos
        .iter()
        .map(|item| {
            let rendered = render_inline_markdown(&item.text);
            if item.done {
                format!("{} {}", "☑".green(), rendered)
            } else {
                format!("{} {}", "☐".yellow(), rendered)
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Format deadline items with date styling
fn format_deadlines(items: &[DeadlineItem]) -> String {
    use crate::cmd::deadline::style_deadline_date;
    let today = Local::now().date_naive();
    items
        .iter()
        .map(|item| {
            let date_styled = style_deadline_date(&item.date, today);
            format!("{}  {}  {}", date_styled, item.text, item.hash.dimmed())
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Format event items with date (and optional time) styling
fn format_events(items: &[EventItem]) -> String {
    use crate::cmd::deadline::style_deadline_date;
    let today = Local::now().date_naive();
    items
        .iter()
        .map(|item| {
            let date_styled = style_deadline_date(&item.date, today);
            let when = match &item.time {
                Some(t) => format!("{} {}", date_styled, t),
                None => date_styled,
            };
            format!("{}  {}  {}", when, item.text, item.hash.dimmed())
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Render inline markdown (bold, italic, code) without block formatting
fn render_inline_markdown(text: &str) -> String {
    let skin = MadSkin::default();
    let mut buf = Vec::new();
    skin.write_text_on(&mut buf, text).ok();
    // Take first line only to avoid block formatting artifacts
    String::from_utf8_lossy(&buf)
        .lines()
        .next()
        .unwrap_or("")
        .to_string()
}

/// Format log entries with relative timestamps and markdown
fn format_log(entries: &[LogEntry]) -> String {
    let now = Local::now().naive_local();

    entries
        .iter()
        .map(|entry| {
            let rendered = render_inline_markdown(&entry.text);
            if entry.ts.is_empty() {
                format!("   {} {}", "·".dimmed(), rendered)
            } else {
                let relative = timestamp_to_relative(&entry.ts, &now);
                format!("{:>4} {}", relative.cyan(), rendered)
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Convert timestamp string to relative time (e.g., "8m", "2h", "3d")
pub(crate) fn timestamp_to_relative(ts_str: &str, now: &NaiveDateTime) -> String {
    let parsed = NaiveDateTime::parse_from_str(ts_str, "%Y-%m-%d %H:%M:%S");
    let dt = match parsed {
        Ok(dt) => dt,
        Err(_) => return ts_str.to_string(),
    };

    let duration = *now - dt;
    let secs = duration.num_seconds();

    if secs < 60 {
        "now".to_string()
    } else if secs < 3600 {
        format!("{}m", secs / 60)
    } else if secs < 86400 {
        format!("{}h", secs / 3600)
    } else if secs < 604800 {
        format!("{}d", secs / 86400)
    } else if secs < 2592000 {
        format!("{}w", secs / 604800)
    } else if secs < 31536000 {
        format!("{}mo", secs / 2592000)
    } else {
        format!("{}y", secs / 31536000)
    }
}

/// Git log entry
struct GitLogEntry {
    relative_time: String,
    hash: String,
    message: String,
}

/// Get git history for a file
fn get_git_history(ws: &Path, rel_path: &str) -> Vec<GitLogEntry> {
    let output = Command::new("git")
        .args([
            "-C",
            &ws.to_string_lossy(),
            "log",
            "--follow",
            "--format=%cr\t%h\t%s",
            "--",
            rel_path,
        ])
        .output();

    let output = match output {
        Ok(o) if o.status.success() => o,
        _ => return Vec::new(),
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout
        .lines()
        .filter_map(|line| {
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() >= 3 {
                Some(GitLogEntry {
                    relative_time: shorten_relative_time(parts[0]),
                    hash: parts[1].to_string(),
                    message: parts[2..].join("\t"),
                })
            } else {
                None
            }
        })
        .collect()
}

/// Shorten git's relative time: "3 hours ago" -> "3h"
fn shorten_relative_time(s: &str) -> String {
    let s = s.trim();
    if s.contains("second") {
        "now".to_string()
    } else if let Some(n) = s
        .strip_suffix(" minutes ago")
        .or(s.strip_suffix(" minute ago"))
    {
        format!("{}m", n)
    } else if let Some(n) = s.strip_suffix(" hours ago").or(s.strip_suffix(" hour ago")) {
        format!("{}h", n)
    } else if let Some(n) = s.strip_suffix(" days ago").or(s.strip_suffix(" day ago")) {
        format!("{}d", n)
    } else if let Some(n) = s.strip_suffix(" weeks ago").or(s.strip_suffix(" week ago")) {
        format!("{}w", n)
    } else if let Some(n) = s
        .strip_suffix(" months ago")
        .or(s.strip_suffix(" month ago"))
    {
        format!("{}mo", n)
    } else if let Some(n) = s.strip_suffix(" years ago").or(s.strip_suffix(" year ago")) {
        format!("{}y", n)
    } else {
        s.to_string()
    }
}

/// Format git history section (like info command)
fn format_history(history: &[GitLogEntry], max_width: usize) -> String {
    if history.is_empty() {
        return "No commits (untracked)".dimmed().to_string();
    }

    let total = history.len();
    if total <= 5 {
        history
            .iter()
            .map(|e| format_git_entry(e, max_width))
            .collect::<Vec<_>>()
            .join("\n")
    } else {
        // Show first 4 + ellipsis + initial commit
        let mut lines: Vec<String> = history
            .iter()
            .take(4)
            .map(|e| format_git_entry(e, max_width))
            .collect();
        lines.push(
            format!("... {} more commits ...", total - 5)
                .dimmed()
                .to_string(),
        );
        if let Some(initial) = history.last() {
            lines.push(format_git_entry(initial, max_width));
        }
        lines.join("\n")
    }
}

/// Format a single git log entry
fn format_git_entry(entry: &GitLogEntry, max_width: usize) -> String {
    let time_str = format!("{:>3}", entry.relative_time);
    let hash_str = output::style_id(&entry.hash).to_string();

    // Calculate remaining space for message
    let prefix_len = 3 + 1 + 7 + 1; // time + space + hash + space
    let msg_max = max_width.saturating_sub(prefix_len);
    let message = output::truncate_back(&entry.message, msg_max);

    format!("{} {} {}", time_str.dimmed(), hash_str, message)
}

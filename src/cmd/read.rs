use std::fs;
use std::io::IsTerminal;
use std::path::Path;
use std::process::Command;

use chrono::{Local, NaiveDateTime};
use clap::Args;
use clap_complete::engine::ArgValueCompleter;
use colored::Colorize;
use regex::Regex;
use termimad::MadSkin;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::output;
use crate::thread::{self, Thread};
use crate::workspace;

#[derive(Args)]
pub struct ReadArgs {
    /// Thread ID or name reference
    #[arg(add = ArgValueCompleter::new(crate::workspace::complete_thread_ids))]
    id: String,

    /// Output raw markdown (skip rendering)
    #[arg(long)]
    raw: bool,

    /// Force pretty output (rich formatting even when not TTY)
    #[arg(long)]
    pretty: bool,

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

    // Raw mode: just print content
    if args.raw {
        print!("{}", content);
        return Ok(());
    }

    // Pretty mode: when --pretty flag OR when TTY (auto-detect)
    let use_pretty = args.pretty || std::io::stdout().is_terminal();

    if use_pretty {
        output_pretty(&file, ws, args.width, args.debug_widths)?;
    } else {
        // Non-TTY without --pretty: raw markdown
        print!("{}", content);
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
    let title_line = format!("{}  {}", title.bold(), status_styled);

    let header = if thread.frontmatter.desc.is_empty() {
        title_line
    } else {
        format!("{}\n{}", title_line, thread.frontmatter.desc)
    };

    // === Extract sections ===
    let body = thread::extract_section(&thread.content, "Body");
    let notes = thread::extract_section(&thread.content, "Notes");
    let todos = thread::extract_section(&thread.content, "Todo");
    let log = thread::extract_section(&thread.content, "Log");

    // === Build sections dynamically ===
    let mut sections: Vec<String> = vec![header];

    if !body.is_empty() {
        sections.push(format_body(&body));
    }
    if !notes.is_empty() {
        sections.push(format_notes(&notes));
    }
    if !todos.is_empty() {
        sections.push(format_todos(&todos));
    }
    if !log.is_empty() {
        sections.push(format_log(&log));
    }

    // Footer: history + path
    let history_content = format_history(&git_history, term_width.saturating_sub(4));
    sections.push(format!("{}\n\n{}", history_content, rel_path.dimmed()));

    // === Render box with sections ===
    print_boxed_sections(&sections, term_width, debug);

    Ok(())
}

/// Print sections in a rounded box with horizontal separators
fn print_boxed_sections(sections: &[String], max_width: usize, debug: bool) {
    let inner_width = max_width.saturating_sub(4); // Account for "│ " and " │"

    if debug {
        eprintln!("DEBUG: max_width={}, inner_width={}", max_width, inner_width);
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

/// Wrap a line to fit within max_width (respecting ANSI codes)
fn wrap_line(line: &str, max_width: usize) -> Vec<String> {
    let visible_width = strip_ansi_width(line);
    if visible_width <= max_width {
        return vec![line.to_string()];
    }

    // Simple truncation with ellipsis for now
    // (Full word-wrapping with ANSI is complex)
    let mut result = String::new();
    let mut visible_count = 0;
    let mut in_escape = false;

    for c in line.chars() {
        if c == '\x1b' {
            in_escape = true;
            result.push(c);
        } else if in_escape {
            result.push(c);
            if c == 'm' {
                in_escape = false;
            }
        } else {
            let char_width = c.width().unwrap_or(0);
            if visible_count + char_width > max_width.saturating_sub(1) {
                result.push('…');
                break;
            }
            visible_count += char_width;
            result.push(c);
        }
    }

    // Reset any open ANSI codes
    if result.contains("\x1b[") {
        result.push_str("\x1b[0m");
    }

    vec![result]
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

/// Format notes section with dimmed hashes
fn format_notes(notes: &str) -> String {
    let header = "Notes".bold().to_string();
    let hash_re = Regex::new(r"<!--\s*([a-f0-9]{4})\s*-->").unwrap();

    let formatted: Vec<String> = notes
        .lines()
        .map(|line| {
            if line.starts_with("- ") {
                // Dim the hash comment
                hash_re
                    .replace(line, |caps: &regex::Captures| {
                        format!("<!-- {} -->", caps[1].to_string().dimmed())
                    })
                    .to_string()
            } else {
                line.to_string()
            }
        })
        .collect();

    format!("{}\n{}", header, formatted.join("\n"))
}

/// Format todo section with colored checkboxes
fn format_todos(todos: &str) -> String {
    let hash_re = Regex::new(r"<!--\s*([a-f0-9]{4})\s*-->").unwrap();

    todos
        .lines()
        .map(|line| {
            let mut line = line.to_string();

            // Replace checkboxes with unicode squares
            if line.contains("- [x]") {
                line = line.replace("- [x]", &"☑".green().to_string());
            } else if line.contains("- [ ]") {
                line = line.replace("- [ ]", &"☐".yellow().to_string());
            }

            // Dim the hash comment
            hash_re
                .replace(&line, |caps: &regex::Captures| {
                    format!("<!-- {} -->", caps[1].to_string().dimmed())
                })
                .to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Format log section with relative timestamps
fn format_log(log: &str) -> String {
    // New format: - **2026-01-22 12:25:00** message
    let full_ts_re = Regex::new(r"^- \*\*(\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2})\*\*(.*)$").unwrap();
    // Old format: - **12:25** message (under ### date header)
    let time_re = Regex::new(r"^- \*\*(\d{2}:\d{2})\*\*(.*)$").unwrap();
    let date_re = Regex::new(r"^### (\d{4}-\d{2}-\d{2})$").unwrap();

    let now = Local::now().naive_local();
    let mut current_date = String::new();

    log.lines()
        .filter_map(|line| {
            // Skip old date headers
            if let Some(caps) = date_re.captures(line) {
                current_date = caps[1].to_string();
                return None;
            }

            // New format: full timestamp
            if let Some(caps) = full_ts_re.captures(line) {
                let ts_str = &caps[1];
                let rest = &caps[2];
                let relative = timestamp_to_relative(ts_str, &now);
                return Some(format!("{:>4} {}", relative.cyan(), rest.trim()));
            }

            // Old format: time only (use current_date context)
            if let Some(caps) = time_re.captures(line) {
                let time = &caps[1];
                let rest = &caps[2];
                if !current_date.is_empty() {
                    let ts_str = format!("{} {}:00", current_date, time);
                    let relative = timestamp_to_relative(&ts_str, &now);
                    return Some(format!("{:>4} {}", relative.cyan(), rest.trim()));
                }
                return Some(format!("{:>4} {}", time.cyan(), rest.trim()));
            }

            // Skip empty lines
            if line.trim().is_empty() {
                None
            } else {
                Some(line.to_string())
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Convert timestamp string to relative time (e.g., "8m", "2h", "3d")
fn timestamp_to_relative(ts_str: &str, now: &NaiveDateTime) -> String {
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

/// Format git history section (like info command)
fn format_history(history: &[GitLogEntry], max_width: usize) -> String {
    if history.is_empty() {
        return "No commits (untracked)".dimmed().to_string();
    }

    let total = history.len();
    if total <= 5 {
        history.iter().map(|e| format_git_entry(e, max_width)).collect::<Vec<_>>().join("\n")
    } else {
        // Show first 4 + ellipsis + initial commit
        let mut lines: Vec<String> = history
            .iter()
            .take(4)
            .map(|e| format_git_entry(e, max_width))
            .collect();
        lines.push(format!("... {} more commits ...", total - 5).dimmed().to_string());
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


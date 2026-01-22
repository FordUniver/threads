use std::fs;
use std::io::IsTerminal;
use std::path::Path;
use std::process::Command;

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
        output_pretty(&file, ws)?;
    } else {
        // Non-TTY without --pretty: raw markdown
        print!("{}", content);
    }
    Ok(())
}

/// Rich pretty output - single box with sections separated by horizontal lines
fn output_pretty(file: &Path, ws: &Path) -> Result<(), String> {
    let thread = Thread::parse(file)?;
    let term_width = output::terminal_width().min(100);

    let rel_path = file
        .strip_prefix(ws)
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| file.to_string_lossy().to_string());

    // Get git history
    let git_history = get_git_history(ws, &rel_path);

    // Count items
    let (log_count, todo_count, todo_done) = count_items(&thread.content);

    // === Section 1: Header (like info) ===
    let title = if !thread.name().is_empty() {
        thread.name().to_string()
    } else {
        thread::extract_name_from_path(file).replace('-', " ")
    };

    let todo_text = if todo_count == 0 {
        "0".dimmed().to_string()
    } else if todo_done == todo_count {
        format!("{}/{} ✓", todo_done, todo_count).green().to_string()
    } else if todo_done > 0 {
        format!("{}/{}", todo_done, todo_count).yellow().to_string()
    } else {
        format!("0/{}", todo_count)
    };

    let status_styled = output::style_status(&thread.base_status()).to_string();
    let stats = format!("{} · {} · {}", log_count, todo_text, status_styled);
    let title_line = format!("{}  {}", title.bold(), stats);

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
    print_boxed_sections(&sections, term_width);

    Ok(())
}

/// Print sections in a rounded box with horizontal separators
fn print_boxed_sections(sections: &[String], max_width: usize) {
    let inner_width = max_width.saturating_sub(4); // Account for "│ " and " │"

    // Top border
    println!("╭{}╮", "─".repeat(max_width - 2));

    for (i, section) in sections.iter().enumerate() {
        // Print section content with padding
        for line in section.lines() {
            // Wrap or truncate long lines
            let wrapped_lines = wrap_line(line, inner_width);
            for wrapped in wrapped_lines {
                let visible_width = strip_ansi_width(&wrapped);
                let padding = inner_width.saturating_sub(visible_width);
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
    let header = "Body".bold().to_string();
    let skin = MadSkin::default();
    let mut buf = Vec::new();
    skin.write_text_on(&mut buf, body).ok();
    let rendered = String::from_utf8_lossy(&buf).trim().to_string();
    format!("{}\n{}", header, rendered)
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
    let header = "Todo".bold().to_string();
    let hash_re = Regex::new(r"<!--\s*([a-f0-9]{4})\s*-->").unwrap();

    let formatted: Vec<String> = todos
        .lines()
        .map(|line| {
            let mut line = line.to_string();

            // Color checkboxes
            if line.contains("- [x]") {
                line = line.replace("- [x]", &format!("- {}", "[x]".green()));
            } else if line.contains("- [ ]") {
                line = line.replace("- [ ]", &format!("- {}", "[ ]".yellow()));
            }

            // Dim the hash comment
            hash_re
                .replace(&line, |caps: &regex::Captures| {
                    format!("<!-- {} -->", caps[1].to_string().dimmed())
                })
                .to_string()
        })
        .collect();

    format!("{}\n{}", header, formatted.join("\n"))
}

/// Format log section with highlighted timestamps
fn format_log(log: &str) -> String {
    let header = "Log".bold().to_string();
    let time_re = Regex::new(r"^(- \*\*)(\d{2}:\d{2})(\*\*)(.*)$").unwrap();
    let date_re = Regex::new(r"^(### )(\d{4}-\d{2}-\d{2})$").unwrap();

    let formatted: Vec<String> = log
        .lines()
        .map(|line| {
            // Format date headers
            if let Some(caps) = date_re.captures(line) {
                return format!("{}{}", "### ".dimmed(), caps[2].to_string().cyan());
            }

            // Format time entries
            if let Some(caps) = time_re.captures(line) {
                let time = &caps[2];
                let rest = &caps[4];
                return format!("- {} {}", time.cyan().bold(), rest.trim());
            }

            line.to_string()
        })
        .collect();

    format!("{}\n{}", header, formatted.join("\n"))
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
    let header = "History".bold().to_string();

    if history.is_empty() {
        return format!("{}\n{}", header, "No commits (untracked)".dimmed());
    }

    let total = history.len();
    let lines: Vec<String> = if total <= 5 {
        history.iter().map(|e| format_git_entry(e, max_width)).collect()
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
        lines
    };

    format!("{}\n{}", header, lines.join("\n"))
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

/// Count log entries and todo items
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

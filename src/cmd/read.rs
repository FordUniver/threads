use std::fs;
use std::io::IsTerminal;
use std::path::Path;
use std::process::Command;

use chrono::{DateTime, Local};
use clap::Args;
use clap_complete::engine::ArgValueCompleter;
use colored::Colorize;
use regex::Regex;
use tabled::settings::object::Columns;
use tabled::settings::style::HorizontalLine;
use tabled::settings::{Alignment, Modify, Padding, Style};
use tabled::Table;
use termimad::MadSkin;

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

/// Rich pretty output with header, body, and formatted sections
fn output_pretty(file: &Path, ws: &Path) -> Result<(), String> {
    let thread = Thread::parse(file)?;
    let term_width = output::terminal_width().min(100).saturating_sub(4);

    let rel_path = file
        .strip_prefix(ws)
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| file.to_string_lossy().to_string());

    // Get timestamps from git
    let (created_dt, updated_dt) = get_git_timestamps(ws, &rel_path);

    // Count items
    let (log_count, todo_count, todo_done) = count_items(&thread.content);

    // === Header ===
    let title = if !thread.name().is_empty() {
        thread.name().to_string()
    } else {
        thread::extract_name_from_path(file).replace('-', " ")
    };

    // Stats line
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
    let header_content = if thread.frontmatter.desc.is_empty() {
        title_line
    } else {
        format!("{}\n{}", title_line, thread.frontmatter.desc)
    };

    // Dates line
    let dates_line = format_dates(created_dt, updated_dt);

    // === Extract sections ===
    let body = thread::extract_section(&thread.content, "Body");
    let notes = thread::extract_section(&thread.content, "Notes");
    let todos = thread::extract_section(&thread.content, "Todo");
    let log = thread::extract_section(&thread.content, "Log");

    // === Build output sections ===
    let mut sections: Vec<String> = Vec::new();

    // Body section (rendered markdown, no header)
    if !body.is_empty() {
        sections.push(render_body(&body, term_width));
    }

    // Notes section
    if !notes.is_empty() {
        sections.push(format_notes(&notes));
    }

    // Todo section
    if !todos.is_empty() {
        sections.push(format_todos(&todos));
    }

    // Log section
    if !log.is_empty() {
        sections.push(format_log(&log));
    }

    // === Build table ===
    let mut rows: Vec<Vec<String>> = vec![
        vec![header_content],
        vec![dates_line],
    ];

    if !sections.is_empty() {
        rows.push(vec![sections.join("\n\n")]);
    }

    // Footer with path
    rows.push(vec![rel_path.dimmed().to_string()]);

    let mut table = Table::from_iter(rows);

    // Horizontal lines after header and dates
    let hline = HorizontalLine::new('─').left('├').right('┤').intersection('─');
    let style = Style::rounded().horizontals([(1, hline.clone()), (2, hline)]);

    table
        .with(style)
        .with(Padding::new(1, 1, 0, 0))
        .with(Modify::new(Columns::single(0)).with(Alignment::left()));

    println!("{}", table);

    Ok(())
}

/// Format created/updated dates
fn format_dates(created: Option<DateTime<Local>>, updated: Option<DateTime<Local>>) -> String {
    let created_str = created
        .map(|dt| output::format_relative_short(dt))
        .unwrap_or_else(|| "?".to_string());
    let updated_str = updated
        .map(|dt| output::format_relative_short(dt))
        .unwrap_or_else(|| "?".to_string());

    if created_str == updated_str {
        format!("{} {}", "created".dimmed(), created_str)
    } else {
        format!(
            "{} {}  {} {}",
            "created".dimmed(),
            created_str,
            "updated".dimmed(),
            updated_str
        )
    }
}

/// Render body markdown content (simplified - just clean up for terminal)
fn render_body(body: &str, _width: usize) -> String {
    let skin = MadSkin::default();
    // Use termimad to render but capture to string
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

/// Get git timestamps for a file
fn get_git_timestamps(ws: &Path, rel_path: &str) -> (Option<DateTime<Local>>, Option<DateTime<Local>>) {
    let output = Command::new("git")
        .args([
            "-C",
            &ws.to_string_lossy(),
            "log",
            "--follow",
            "--format=%ct",
            "--",
            rel_path,
        ])
        .output();

    let output = match output {
        Ok(o) if o.status.success() => o,
        _ => return (None, None),
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = stdout.lines().collect();

    if lines.is_empty() {
        return (None, None);
    }

    let parse_ts = |s: &str| -> Option<DateTime<Local>> {
        s.parse::<i64>()
            .ok()
            .and_then(|ts| DateTime::from_timestamp(ts, 0))
            .map(|dt| dt.with_timezone(&Local))
    };

    let updated = parse_ts(lines[0]);
    let created = parse_ts(lines[lines.len() - 1]);

    (created, updated)
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

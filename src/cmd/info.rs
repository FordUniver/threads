use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::SystemTime;

use chrono::{DateTime, Local};
use clap::Args;
use clap_complete::engine::ArgValueCompleter;

use crate::thread::Thread;
use crate::workspace;

#[derive(Args)]
pub struct InfoArgs {
    /// Thread ID or name reference
    #[arg(add = ArgValueCompleter::new(crate::workspace::complete_thread_ids))]
    id: String,
}

pub fn run(args: InfoArgs, ws: &Path) -> Result<(), String> {
    let file = workspace::find_by_ref(ws, &args.id)?;
    let thread = Thread::parse(&file)?;

    let rel_path = file
        .strip_prefix(ws)
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| file.to_string_lossy().to_string());

    // Get timestamps
    let (created, updated) = get_timestamps(&file);

    // Get git status
    let git_status = get_git_status(ws, &rel_path);

    // Count log entries and todos
    let (log_count, todo_count, todo_done) = count_items(&thread.content);

    // Header line: ID - created - updated - git status
    println!(
        "{} | created {} | updated {} | {}",
        thread.id(),
        created,
        updated,
        git_status
    );
    println!();

    // Title
    let title = if !thread.name().is_empty() {
        thread.name().to_string()
    } else {
        crate::thread::extract_name_from_path(&file).replace('-', " ")
    };
    println!("{}", title);

    // Description
    if !thread.frontmatter.desc.is_empty() {
        println!("{}", thread.frontmatter.desc);
    }
    println!();

    // Stats
    println!(
        "{} log entries | {} todos ({} done)",
        log_count, todo_count, todo_done
    );
    println!();

    // Git history
    print_git_history(ws, &rel_path);

    Ok(())
}

fn get_timestamps(path: &Path) -> (String, String) {
    let metadata = match fs::metadata(path) {
        Ok(m) => m,
        Err(_) => return ("?".to_string(), "?".to_string()),
    };

    let format_time = |time: SystemTime| -> String {
        let datetime: DateTime<Local> = time.into();
        datetime.format("%Y-%m-%d").to_string()
    };

    let updated = metadata
        .modified()
        .map(&format_time)
        .unwrap_or_else(|_| "?".to_string());

    let created = metadata
        .created()
        .map(format_time)
        .unwrap_or_else(|_| updated.clone());

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

    // Parse porcelain format: XY filename
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

fn print_git_history(ws: &Path, rel_path: &str) {
    let output = Command::new("git")
        .args([
            "-C",
            &ws.to_string_lossy(),
            "log",
            "--oneline",
            "--follow",
            "-10",
            "--",
            rel_path,
        ])
        .output();

    let output = match output {
        Ok(o) if o.status.success() => o,
        _ => return,
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = stdout.trim().lines().collect();

    if lines.is_empty() {
        println!("No git history (untracked)");
        return;
    }

    println!("Git history:");
    for line in lines {
        println!("  {}", line);
    }
}

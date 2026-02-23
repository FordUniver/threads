use std::path::Path;

use chrono::{Local, NaiveDate};
use clap::Args;
use clap_complete::engine::ArgValueCompleter;
use colored::Colorize;

use crate::args::{DirectionArgs, FilterArgs, FormatArgs};
use crate::config::{Config, env_bool, is_quiet};
use crate::git;
use crate::output::{self, OutputFormat};
use crate::thread::{self, DeadlineItem, Thread};
use crate::workspace;

#[derive(Args)]
pub struct DeadlineArgs {
    /// Thread ID (omit for agenda view across scope)
    #[arg(default_value = "", add = ArgValueCompleter::new(crate::workspace::complete_thread_ids))]
    id: String,

    /// Action: list, add, remove (default: list)
    #[arg(default_value = "list")]
    action: String,

    /// Date (YYYY-MM-DD) for add, or hash prefix for remove
    #[arg(default_value = "")]
    arg1: String,

    /// Deadline text (for add; trailing words joined)
    #[arg(default_value = "", trailing_var_arg = true)]
    text: Vec<String>,

    #[command(flatten)]
    direction: DirectionArgs,

    #[command(flatten)]
    filter: FilterArgs,

    #[command(flatten)]
    format: FormatArgs,

    /// Commit after editing
    #[arg(long)]
    commit: bool,

    /// Commit message
    #[arg(short = 'm', long)]
    message: Option<String>,
}

pub fn run(args: DeadlineArgs, ws: &Path, config: &Config) -> Result<(), String> {
    // Agenda mode: no id given (or only direction/filter flags used)
    if args.id.is_empty() && args.action == "list" {
        return run_agenda(&args, ws, config);
    }

    // Single-thread mode requires an id
    if args.id.is_empty() {
        return Err(
            "usage: threads deadline <id> [add <date> <text...> | remove <hash>]".to_string(),
        );
    }

    let file = workspace::find_by_ref(ws, &args.id)?;
    let mut t = Thread::parse(&file)?;

    match args.action.as_str() {
        "list" | "ls" => {
            let items = t.get_deadlines();
            if items.is_empty() {
                println!("No deadlines.");
            } else {
                let today = Local::now().date_naive();
                print_deadline_list(&items, today);
            }
            return Ok(());
        }
        "add" => {
            let date = &args.arg1;
            if date.is_empty() {
                return Err("usage: threads deadline <id> add <YYYY-MM-DD> <text...>".to_string());
            }
            // Validate date
            NaiveDate::parse_from_str(date, "%Y-%m-%d")
                .map_err(|_| format!("invalid date '{}': expected YYYY-MM-DD", date))?;

            let text = args.text.join(" ");
            if text.is_empty() {
                return Err("usage: threads deadline <id> add <YYYY-MM-DD> <text...>".to_string());
            }

            let hash = t.add_deadline(date, &text)?;
            let log_entry = format!("Added deadline: {} {}", date, text);
            t.insert_log_entry(&log_entry)?;
            println!("Added deadline: {} {} (id: {})", date, text, hash);
        }
        "remove" | "rm" => {
            let hash = &args.arg1;
            if hash.is_empty() {
                return Err("usage: threads deadline <id> remove <hash>".to_string());
            }
            t.remove_deadline_by_hash(hash)?;
            let log_entry = format!("Removed deadline {}", hash);
            t.insert_log_entry(&log_entry)?;
            println!("Removed deadline {}", hash);
        }
        _ => {
            return Err(format!(
                "unknown action '{}'. Use: list, add, remove",
                args.action
            ));
        }
    }

    t.write()?;

    let should_commit = args.commit || env_bool("THREADS_AUTO_COMMIT").unwrap_or(false);
    if should_commit {
        let repo = workspace::open()?;
        let rel_path = file.strip_prefix(ws).unwrap_or(&file);
        let msg = args
            .message
            .unwrap_or_else(|| git::generate_commit_message(&repo, &[rel_path]));
        git::auto_commit(&repo, &file, &msg)?;
    } else if !is_quiet(config) {
        output::print_uncommitted_hint(&args.id);
    }

    Ok(())
}

/// Agenda: collect deadlines from all threads in scope, sorted by date.
fn run_agenda(args: &DeadlineArgs, ws: &Path, _config: &Config) -> Result<(), String> {
    let format = args.format.resolve();

    let scope = workspace::infer_scope(ws, None)?;
    let start_path = scope.threads_dir.parent().unwrap_or(ws);
    let options = args.direction.to_find_options();
    let thread_files = workspace::find_threads_with_options(start_path, ws, &options)?;

    let include_closed = args.filter.include_closed();

    struct AgendaItem {
        date: String,
        text: String,
        hash: String,
        thread_id: String,
        thread_name: String,
        thread_path: String,
    }

    let mut agenda: Vec<AgendaItem> = Vec::new();

    for path in &thread_files {
        let t = match Thread::parse(path) {
            Ok(t) => t,
            Err(_) => continue,
        };

        // Respect closed filter
        if !include_closed && thread::is_closed(t.status()) {
            continue;
        }

        let rel_path = path
            .strip_prefix(ws)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| path.to_string_lossy().to_string());

        let thread_name = thread::extract_name_from_path(path);
        let thread_id = t.id().to_string();

        for d in t.get_deadlines() {
            agenda.push(AgendaItem {
                date: d.date,
                text: d.text,
                hash: d.hash,
                thread_id: thread_id.clone(),
                thread_name: thread_name.clone(),
                thread_path: rel_path.clone(),
            });
        }
    }

    if agenda.is_empty() {
        println!("No deadlines found.");
        return Ok(());
    }

    // Sort by date ascending
    agenda.sort_by(|a, b| a.date.cmp(&b.date));

    match format {
        OutputFormat::Json => {
            use serde::Serialize;
            #[derive(Serialize)]
            struct JsonItem<'a> {
                date: &'a str,
                text: &'a str,
                hash: &'a str,
                thread_id: &'a str,
                thread_name: &'a str,
                thread_path: &'a str,
            }
            let items: Vec<_> = agenda
                .iter()
                .map(|a| JsonItem {
                    date: &a.date,
                    text: &a.text,
                    hash: &a.hash,
                    thread_id: &a.thread_id,
                    thread_name: &a.thread_name,
                    thread_path: &a.thread_path,
                })
                .collect();
            println!(
                "{}",
                serde_json::to_string_pretty(&items).map_err(|e| format!("JSON error: {}", e))?
            );
        }
        OutputFormat::Plain => {
            println!("DATE | TEXT | HASH | THREAD_ID | NAME | PATH");
            for a in &agenda {
                println!(
                    "{} | {} | {} | {} | {} | {}",
                    a.date, a.text, a.hash, a.thread_id, a.thread_name, a.thread_path
                );
            }
        }
        _ => {
            let today = Local::now().date_naive();
            for a in &agenda {
                let date_styled = style_deadline_date(&a.date, today);
                println!(
                    "{}  {}  {}  {}",
                    date_styled,
                    a.text,
                    a.hash.dimmed(),
                    format!("[{}]", a.thread_id).dimmed()
                );
            }
        }
    }

    Ok(())
}

/// Print deadline list for a single thread with date styling.
fn print_deadline_list(items: &[DeadlineItem], today: NaiveDate) {
    for item in items {
        let date_styled = style_deadline_date(&item.date, today);
        println!("{}  {}  ({})", date_styled, item.text, item.hash.dimmed());
    }
}

/// Style a date string based on proximity to today.
pub fn style_deadline_date(date: &str, today: NaiveDate) -> String {
    match NaiveDate::parse_from_str(date, "%Y-%m-%d") {
        Ok(d) => {
            let days = (d - today).num_days();
            if days < 0 {
                date.red().to_string()
            } else if days == 0 {
                date.red().bold().to_string()
            } else if days <= 7 {
                date.yellow().to_string()
            } else {
                date.to_string()
            }
        }
        Err(_) => date.to_string(),
    }
}

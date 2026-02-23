use std::path::Path;

use chrono::Local;
use clap::Args;
use clap_complete::engine::ArgValueCompleter;
use colored::Colorize;

use crate::args::{DirectionArgs, FilterArgs, FormatArgs};
use crate::config::{Config, env_bool, is_quiet};
use crate::git;
use crate::input;
use crate::output::{self, OutputFormat};
use crate::thread::{self, Thread};
use crate::workspace;

#[derive(Args)]
pub struct LogArgs {
    /// Thread ID (omit for agenda view across scope)
    #[arg(default_value = "", add = ArgValueCompleter::new(crate::workspace::complete_thread_ids))]
    id: String,

    /// Log entry text (reads from stdin if omitted; ignored in agenda mode)
    #[arg(default_value = "")]
    entry: String,

    #[command(flatten)]
    direction: DirectionArgs,

    #[command(flatten)]
    filter: FilterArgs,

    #[command(flatten)]
    format: FormatArgs,

    /// Commit after adding
    #[arg(long)]
    commit: bool,

    /// Commit message
    #[arg(short = 'm', long)]
    message: Option<String>,
}

pub fn run(args: LogArgs, ws: &Path, config: &Config) -> Result<(), String> {
    if args.id.is_empty() {
        return run_agenda(&args, ws, config);
    }

    let mut entry = args.entry.clone();

    // Read entry from stdin if not provided
    if entry.is_empty() {
        entry = input::read_stdin(true);
    }

    if entry.is_empty() {
        return Err("no log entry provided".to_string());
    }

    let file = workspace::find_by_ref(ws, &args.id)?;

    let mut t = Thread::parse(&file)?;

    t.insert_log_entry(&entry)?;

    t.write()?;

    println!("Logged to: {}", file.display());

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

/// Agenda: collect log entries from all threads in scope, sorted by timestamp descending.
fn run_agenda(args: &LogArgs, ws: &Path, _config: &Config) -> Result<(), String> {
    let format = args.format.resolve();

    let scope = workspace::infer_scope(ws, None)?;
    let start_path = scope.threads_dir.parent().unwrap_or(ws);
    let options = args.direction.to_find_options();
    let thread_files = workspace::find_threads_with_options(start_path, ws, &options)?;

    let include_closed = args.filter.include_closed();

    struct AgendaItem {
        ts: String,
        text: String,
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

        if !include_closed && thread::is_closed(t.status()) {
            continue;
        }

        let rel_path = path
            .strip_prefix(ws)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| path.to_string_lossy().to_string());

        let thread_name = thread::extract_name_from_path(path);
        let thread_id = t.id().to_string();

        for entry in t.get_log_entries() {
            agenda.push(AgendaItem {
                ts: entry.ts,
                text: entry.text,
                thread_id: thread_id.clone(),
                thread_name: thread_name.clone(),
                thread_path: rel_path.clone(),
            });
        }
    }

    if agenda.is_empty() {
        println!("No log entries found.");
        return Ok(());
    }

    // Sort: entries with ts descending (most recent first); empty ts last
    agenda.sort_by(|a, b| match (a.ts.is_empty(), b.ts.is_empty()) {
        (true, true) => std::cmp::Ordering::Equal,
        (true, false) => std::cmp::Ordering::Greater,
        (false, true) => std::cmp::Ordering::Less,
        (false, false) => b.ts.cmp(&a.ts),
    });

    match format {
        OutputFormat::Json => {
            use serde::Serialize;
            #[derive(Serialize)]
            struct JsonItem<'a> {
                ts: &'a str,
                text: &'a str,
                thread_id: &'a str,
                thread_name: &'a str,
                thread_path: &'a str,
            }
            let items: Vec<_> = agenda
                .iter()
                .map(|a| JsonItem {
                    ts: &a.ts,
                    text: &a.text,
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
        OutputFormat::Yaml => {
            use serde::Serialize;
            #[derive(Serialize)]
            struct YamlItem<'a> {
                ts: &'a str,
                text: &'a str,
                thread_id: &'a str,
                thread_name: &'a str,
                thread_path: &'a str,
            }
            let items: Vec<_> = agenda
                .iter()
                .map(|a| YamlItem {
                    ts: &a.ts,
                    text: &a.text,
                    thread_id: &a.thread_id,
                    thread_name: &a.thread_name,
                    thread_path: &a.thread_path,
                })
                .collect();
            print!(
                "{}",
                serde_yaml::to_string(&items).map_err(|e| format!("YAML error: {}", e))?
            );
        }
        OutputFormat::Plain => {
            println!("TS | TEXT | THREAD_ID | NAME | PATH");
            for a in &agenda {
                println!(
                    "{} | {} | {} | {} | {}",
                    a.ts, a.text, a.thread_id, a.thread_name, a.thread_path
                );
            }
        }
        _ => {
            let now = Local::now().naive_local();
            for a in &agenda {
                if a.ts.is_empty() {
                    println!(
                        "   {}  {}  {}",
                        "·".dimmed(),
                        a.text,
                        format!("[{}]", a.thread_id).dimmed()
                    );
                } else {
                    let relative = crate::cmd::read::timestamp_to_relative(&a.ts, &now);
                    println!(
                        "{:>4}  {}  {}",
                        relative.cyan(),
                        a.text,
                        format!("[{}]", a.thread_id).dimmed()
                    );
                }
            }
        }
    }

    Ok(())
}

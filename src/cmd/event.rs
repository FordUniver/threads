use std::path::Path;

use chrono::{Local, NaiveDate};
use clap::Args;
use clap_complete::engine::ArgValueCompleter;
use colored::Colorize;
use regex::Regex;
use std::sync::LazyLock;

use crate::args::{DirectionArgs, FilterArgs, FormatArgs};
use crate::config::{Config, env_bool, is_quiet};
use crate::git;
use crate::output::{self, OutputFormat};
use crate::thread::{self, EventItem, Thread};
use crate::workspace;

static TIME_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\d{2}:\d{2}$").unwrap());

#[derive(Args)]
pub struct EventArgs {
    /// Thread ID (omit for agenda view across scope)
    #[arg(default_value = "", add = ArgValueCompleter::new(crate::workspace::complete_thread_ids))]
    id: String,

    /// Action: list, add, remove (default: list)
    #[arg(default_value = "list")]
    action: String,

    /// Date (YYYY-MM-DD) for add, or hash prefix for remove
    #[arg(default_value = "")]
    date_or_hash: String,

    /// Optional time (HH:MM) or first word of description; rest is description
    #[arg(default_value = "")]
    rest: Vec<String>,

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

pub fn run(args: EventArgs, ws: &Path, config: &Config) -> Result<(), String> {
    // Agenda mode: no id given
    if args.id.is_empty() && args.action == "list" {
        return run_agenda(&args, ws, config);
    }

    if args.id.is_empty() {
        return Err(
            "usage: threads event <id> [add <date> [HH:MM] <text...> | remove <hash>]".to_string(),
        );
    }

    let file = workspace::find_by_ref(ws, &args.id)?;
    let mut t = Thread::parse(&file)?;

    match args.action.as_str() {
        "list" | "ls" => {
            let format = args.format.resolve();
            let items = t.get_events();
            match format {
                OutputFormat::Json => {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&items).map_err(|e| e.to_string())?
                    );
                }
                OutputFormat::Yaml => {
                    print!(
                        "{}",
                        serde_yaml::to_string(&items).map_err(|e| e.to_string())?
                    );
                }
                _ => {
                    if items.is_empty() {
                        println!("No events.");
                    } else {
                        let has_time = items.iter().any(|e| e.time.is_some());
                        let today = Local::now().date_naive();
                        print_event_list(&items, has_time, today);
                    }
                }
            }
            return Ok(());
        }
        "add" => {
            let date = &args.date_or_hash;
            if date.is_empty() {
                return Err(
                    "usage: threads event <id> add <YYYY-MM-DD> [HH:MM] <text...>".to_string(),
                );
            }
            NaiveDate::parse_from_str(date, "%Y-%m-%d")
                .map_err(|_| format!("invalid date '{}': expected YYYY-MM-DD", date))?;

            // Parse optional time from first token of rest
            let (time, text) = parse_time_and_text(&args.rest);

            if text.is_empty() {
                return Err(
                    "usage: threads event <id> add <YYYY-MM-DD> [HH:MM] <text...>".to_string(),
                );
            }

            let hash = t.add_event(date, time.as_deref(), &text)?;
            let time_part = time
                .as_deref()
                .map(|tm| format!(" {}", tm))
                .unwrap_or_default();
            let log_entry = format!("Added event: {}{} {}", date, time_part, text);
            t.insert_log_entry(&log_entry)?;
            println!("Added event: {}{} {} (id: {})", date, time_part, text, hash);
        }
        "remove" | "rm" => {
            let hash = &args.date_or_hash;
            if hash.is_empty() {
                return Err("usage: threads event <id> remove <hash>".to_string());
            }
            t.remove_event_by_hash(hash)?;
            let log_entry = format!("Removed event {}", hash);
            t.insert_log_entry(&log_entry)?;
            println!("Removed event {}", hash);
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

/// Parse optional HH:MM time from the first token; return (time, joined text).
fn parse_time_and_text(tokens: &[String]) -> (Option<String>, String) {
    if tokens.is_empty() {
        return (None, String::new());
    }
    if TIME_RE.is_match(&tokens[0]) {
        (Some(tokens[0].clone()), tokens[1..].join(" "))
    } else {
        (None, tokens.join(" "))
    }
}

/// Agenda: collect events from all threads in scope, sorted by date then time.
fn run_agenda(args: &EventArgs, ws: &Path, _config: &Config) -> Result<(), String> {
    let format = args.format.resolve();

    let scope = workspace::infer_scope(ws, None)?;
    let start_path = scope.threads_dir.parent().unwrap_or(ws);
    let options = args.direction.to_find_options();
    let thread_files = workspace::find_threads_with_options(start_path, ws, &options)?;

    let include_closed = args.filter.include_closed();

    struct AgendaItem {
        date: String,
        time: Option<String>,
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

        if !include_closed && thread::is_closed(t.status()) {
            continue;
        }

        let rel_path = path
            .strip_prefix(ws)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| path.to_string_lossy().to_string());

        let thread_name = thread::extract_name_from_path(path);
        let thread_id = t.id().to_string();

        for e in t.get_events() {
            agenda.push(AgendaItem {
                date: e.date,
                time: e.time,
                text: e.text,
                hash: e.hash,
                thread_id: thread_id.clone(),
                thread_name: thread_name.clone(),
                thread_path: rel_path.clone(),
            });
        }
    }

    if agenda.is_empty() {
        println!("No events found.");
        return Ok(());
    }

    // Sort by date then time (None sorts before Some)
    agenda.sort_by(|a, b| {
        a.date.cmp(&b.date).then(
            a.time
                .as_deref()
                .unwrap_or("")
                .cmp(b.time.as_deref().unwrap_or("")),
        )
    });

    let has_time = agenda.iter().any(|a| a.time.is_some());

    match format {
        OutputFormat::Json => {
            use serde::Serialize;
            #[derive(Serialize)]
            struct JsonItem<'a> {
                date: &'a str,
                #[serde(skip_serializing_if = "Option::is_none")]
                time: Option<&'a str>,
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
                    time: a.time.as_deref(),
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
        OutputFormat::Yaml => {
            use serde::Serialize;
            #[derive(Serialize)]
            struct YamlItem<'a> {
                date: &'a str,
                #[serde(skip_serializing_if = "Option::is_none")]
                time: Option<&'a str>,
                text: &'a str,
                hash: &'a str,
                thread_id: &'a str,
                thread_name: &'a str,
                thread_path: &'a str,
            }
            let items: Vec<_> = agenda
                .iter()
                .map(|a| YamlItem {
                    date: &a.date,
                    time: a.time.as_deref(),
                    text: &a.text,
                    hash: &a.hash,
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
            let header = if has_time {
                "DATE | TIME | TEXT | HASH | THREAD_ID | NAME | PATH"
            } else {
                "DATE | TEXT | HASH | THREAD_ID | NAME | PATH"
            };
            println!("{}", header);
            for a in &agenda {
                if has_time {
                    println!(
                        "{} | {} | {} | {} | {} | {} | {}",
                        a.date,
                        a.time.as_deref().unwrap_or(""),
                        a.text,
                        a.hash,
                        a.thread_id,
                        a.thread_name,
                        a.thread_path
                    );
                } else {
                    println!(
                        "{} | {} | {} | {} | {} | {}",
                        a.date, a.text, a.hash, a.thread_id, a.thread_name, a.thread_path
                    );
                }
            }
        }
        _ => {
            let today = Local::now().date_naive();
            for a in &agenda {
                let date_styled = style_event_date(&a.date, today);
                let time_part = a
                    .time
                    .as_deref()
                    .map(|tm| format!("  {}", tm))
                    .unwrap_or_else(|| {
                        if has_time {
                            "      ".to_string()
                        } else {
                            String::new()
                        }
                    });
                println!(
                    "{}{}  {}  {}  {}",
                    date_styled,
                    time_part,
                    a.text,
                    a.hash.dimmed(),
                    format!("[{}]", a.thread_id).dimmed()
                );
            }
        }
    }

    Ok(())
}

/// Print event list for a single thread.
fn print_event_list(items: &[EventItem], has_time: bool, today: NaiveDate) {
    for item in items {
        let date_styled = style_event_date(&item.date, today);
        let time_part = if has_time {
            item.time
                .as_deref()
                .map(|tm| format!("  {}", tm))
                .unwrap_or_else(|| "      ".to_string())
        } else {
            String::new()
        };
        println!(
            "{}{}  {}  ({})",
            date_styled,
            time_part,
            item.text,
            item.hash.dimmed()
        );
    }
}

/// Style an event date based on proximity to today.
fn style_event_date(date: &str, today: NaiveDate) -> String {
    match NaiveDate::parse_from_str(date, "%Y-%m-%d") {
        Ok(d) => {
            let days = (d - today).num_days();
            if days < 0 {
                date.dimmed().to_string()
            } else if days == 0 {
                date.cyan().bold().to_string()
            } else if days <= 7 {
                date.cyan().to_string()
            } else {
                date.to_string()
            }
        }
        Err(_) => date.to_string(),
    }
}

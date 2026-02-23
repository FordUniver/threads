use std::path::Path;

use clap::Args;
use clap_complete::engine::ArgValueCompleter;
use colored::Colorize;

use crate::args::{DirectionArgs, FilterArgs, FormatArgs};
use crate::config::{Config, env_bool, is_quiet};
use crate::git;
use crate::output::{self, OutputFormat};
use crate::thread::{self, Thread};
use crate::workspace;

#[derive(Args)]
pub struct NoteArgs {
    /// Thread ID (omit for agenda view across scope)
    #[arg(default_value = "", add = ArgValueCompleter::new(crate::workspace::complete_thread_ids))]
    id: String,

    /// Action: list, add, edit, remove (default: list)
    #[arg(default_value = "list")]
    action: String,

    /// Note text (for add) or hash reference (for edit/remove)
    #[arg(default_value = "")]
    text: String,

    /// New text when editing (edit action only)
    #[arg(default_value = "")]
    new_text: String,

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

pub fn run(args: NoteArgs, ws: &Path, config: &Config) -> Result<(), String> {
    if args.id.is_empty() && args.action == "list" {
        return run_agenda(&args, ws, config);
    }

    if args.id.is_empty() {
        return Err(
            "usage: threads note <id> [list | add <text> | edit <hash> <text> | remove <hash>]"
                .to_string(),
        );
    }

    let file = workspace::find_by_ref(ws, &args.id)?;

    let mut t = Thread::parse(&file)?;

    match args.action.as_str() {
        "list" | "ls" => {
            let items = t.get_notes();
            if items.is_empty() {
                println!("No notes.");
            } else {
                for item in items {
                    println!("- {} ({})", item.text, item.hash);
                }
            }
            return Ok(());
        }
        "add" => {
            if args.text.is_empty() {
                return Err("usage: threads note <id> add \"text\"".to_string());
            }
            let text = &args.text;

            let hash = t.add_note(text)?;

            // Add log entry
            let log_entry = format!("Added note: {}", text);
            t.insert_log_entry(&log_entry)?;

            println!("Added note: {} (id: {})", text, hash);
        }
        "edit" => {
            if args.text.is_empty() || args.new_text.is_empty() {
                return Err("usage: threads note <id> edit <hash> \"new text\"".to_string());
            }
            let hash = &args.text;
            let new_text = &args.new_text;

            // Check for ambiguous hash
            let count = t.count_matching_items("Notes", hash);
            if count == 0 {
                return Err(format!("no note with hash '{}' found", hash));
            }
            if count > 1 {
                return Err(format!("ambiguous hash '{}' matches {} notes", hash, count));
            }

            t.edit_by_hash("Notes", hash, new_text)?;

            let log_entry = format!("Edited note {}", hash);
            t.insert_log_entry(&log_entry)?;

            println!("Edited note {}", hash);
        }
        "remove" => {
            if args.text.is_empty() {
                return Err("usage: threads note <id> remove <hash>".to_string());
            }
            let hash = &args.text;

            // Check for ambiguous hash
            let count = t.count_matching_items("Notes", hash);
            if count == 0 {
                return Err(format!("no note with hash '{}' found", hash));
            }
            if count > 1 {
                return Err(format!("ambiguous hash '{}' matches {} notes", hash, count));
            }

            t.remove_by_hash("Notes", hash)?;

            let log_entry = format!("Removed note {}", hash);
            t.insert_log_entry(&log_entry)?;

            println!("Removed note {}", hash);
        }
        _ => {
            return Err(format!(
                "unknown action '{}'. Use: list, add, edit, remove",
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

/// Agenda: collect notes from all threads in scope.
fn run_agenda(args: &NoteArgs, ws: &Path, _config: &Config) -> Result<(), String> {
    let format = args.format.resolve();

    let scope = workspace::infer_scope(ws, None)?;
    let start_path = scope.threads_dir.parent().unwrap_or(ws);
    let options = args.direction.to_find_options();
    let thread_files = workspace::find_threads_with_options(start_path, ws, &options)?;

    let include_closed = args.filter.include_closed();

    struct AgendaItem {
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

        for item in t.get_notes() {
            agenda.push(AgendaItem {
                text: item.text,
                hash: item.hash,
                thread_id: thread_id.clone(),
                thread_name: thread_name.clone(),
                thread_path: rel_path.clone(),
            });
        }
    }

    if agenda.is_empty() {
        println!("No notes found.");
        return Ok(());
    }

    match format {
        OutputFormat::Json => {
            use serde::Serialize;
            #[derive(Serialize)]
            struct JsonItem<'a> {
                text: &'a str,
                hash: &'a str,
                thread_id: &'a str,
                thread_name: &'a str,
                thread_path: &'a str,
            }
            let items: Vec<_> = agenda
                .iter()
                .map(|a| JsonItem {
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
                text: &'a str,
                hash: &'a str,
                thread_id: &'a str,
                thread_name: &'a str,
                thread_path: &'a str,
            }
            let items: Vec<_> = agenda
                .iter()
                .map(|a| YamlItem {
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
            println!("TEXT | HASH | THREAD_ID | NAME | PATH");
            for a in &agenda {
                println!(
                    "{} | {} | {} | {} | {}",
                    a.text, a.hash, a.thread_id, a.thread_name, a.thread_path
                );
            }
        }
        _ => {
            for a in &agenda {
                println!(
                    "{}  {}  {}",
                    format!("• {}", a.text),
                    a.hash.dimmed(),
                    format!("[{}]", a.thread_id).dimmed()
                );
            }
        }
    }

    Ok(())
}

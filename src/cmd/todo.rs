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
pub struct TodoArgs {
    /// Thread ID (omit for agenda view across scope)
    #[arg(default_value = "", add = ArgValueCompleter::new(crate::workspace::complete_thread_ids))]
    id: String,

    /// Action: list, add, check, uncheck, remove (default: list)
    #[arg(default_value = "list")]
    action: String,

    /// Item text or hash (depending on action)
    #[arg(default_value = "")]
    item: String,

    #[command(flatten)]
    direction: DirectionArgs,

    #[command(flatten)]
    filter: FilterArgs,

    #[command(flatten)]
    format: FormatArgs,

    /// Include completed (checked) todos in agenda view
    #[arg(long)]
    include_done: bool,

    /// Commit after editing
    #[arg(long)]
    commit: bool,

    /// Commit message
    #[arg(short = 'm', long)]
    m: Option<String>,
}

pub fn run(args: TodoArgs, ws: &Path, config: &Config) -> Result<(), String> {
    if args.id.is_empty() && args.action == "list" {
        return run_agenda(&args, ws, config);
    }

    if args.id.is_empty() {
        return Err("usage: threads todo <id> [add <text> | check <hash> | uncheck <hash> | remove <hash>]".to_string());
    }

    let file = workspace::find_by_ref(ws, &args.id)?;

    let mut t = Thread::parse(&file)?;

    match args.action.as_str() {
        "list" | "ls" => {
            let items = t.get_todo_items();
            if items.is_empty() {
                println!("No todo items.");
            } else {
                for item in items {
                    let mark = if item.done { "[x]" } else { "[ ]" };
                    println!("{} {} ({})", mark, item.text, item.hash);
                }
            }
            return Ok(());
        }
        "add" => {
            if args.item.is_empty() {
                return Err("usage: threads todo <id> add \"item text\"".to_string());
            }
            let text = &args.item;

            let hash = t.add_todo_item(text)?;

            println!("Added to Todo: {} (id: {})", text, hash);
        }
        "check" | "complete" | "done" => {
            if args.item.is_empty() {
                return Err("usage: threads todo <id> check <hash>".to_string());
            }
            let hash = &args.item;

            // Check for ambiguous hash
            let count = t.count_matching_items("Todo", hash);
            if count == 0 {
                return Err(format!("no unchecked item with hash '{}' found", hash));
            }
            if count > 1 {
                return Err(format!("ambiguous hash '{}' matches {} items", hash, count));
            }

            t.set_todo_checked(hash, true)?;

            println!("Checked item {}", hash);
        }
        "uncheck" => {
            if args.item.is_empty() {
                return Err("usage: threads todo <id> uncheck <hash>".to_string());
            }
            let hash = &args.item;

            // Check for ambiguous hash
            let count = t.count_matching_items("Todo", hash);
            if count == 0 {
                return Err(format!("no checked item with hash '{}' found", hash));
            }
            if count > 1 {
                return Err(format!("ambiguous hash '{}' matches {} items", hash, count));
            }

            t.set_todo_checked(hash, false)?;

            println!("Unchecked item {}", hash);
        }
        "remove" => {
            if args.item.is_empty() {
                return Err("usage: threads todo <id> remove <hash>".to_string());
            }
            let hash = &args.item;

            // Check for ambiguous hash
            let count = t.count_matching_items("Todo", hash);
            if count == 0 {
                return Err(format!("no item with hash '{}' found", hash));
            }
            if count > 1 {
                return Err(format!("ambiguous hash '{}' matches {} items", hash, count));
            }

            t.remove_by_hash("Todo", hash)?;

            println!("Removed item {}", hash);
        }
        _ => {
            return Err(format!(
                "unknown action '{}'. Use: list, add, check, uncheck, remove",
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
            .m
            .unwrap_or_else(|| git::generate_commit_message(&repo, &[rel_path]));
        git::auto_commit(&repo, &file, &msg)?;
    } else if !is_quiet(config) {
        output::print_uncommitted_hint(&args.id);
    }

    Ok(())
}

/// Agenda: collect todos from all threads in scope.
fn run_agenda(args: &TodoArgs, ws: &Path, _config: &Config) -> Result<(), String> {
    let format = args.format.resolve();

    let scope = workspace::infer_scope(ws, None)?;
    let start_path = scope.threads_dir.parent().unwrap_or(ws);
    let options = args.direction.to_find_options();
    let thread_files = workspace::find_threads_with_options(start_path, ws, &options)?;

    let include_closed = args.filter.include_closed();

    struct AgendaItem {
        text: String,
        hash: String,
        done: bool,
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

        for item in t.get_todo_items() {
            if item.done && !args.include_done {
                continue;
            }
            agenda.push(AgendaItem {
                text: item.text,
                hash: item.hash,
                done: item.done,
                thread_id: thread_id.clone(),
                thread_name: thread_name.clone(),
                thread_path: rel_path.clone(),
            });
        }
    }

    if agenda.is_empty() {
        if args.include_done {
            println!("No todos found.");
        } else {
            println!("No open todos found.");
        }
        return Ok(());
    }

    match format {
        OutputFormat::Json => {
            use serde::Serialize;
            #[derive(Serialize)]
            struct JsonItem<'a> {
                done: bool,
                text: &'a str,
                hash: &'a str,
                thread_id: &'a str,
                thread_name: &'a str,
                thread_path: &'a str,
            }
            let items: Vec<_> = agenda
                .iter()
                .map(|a| JsonItem {
                    done: a.done,
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
                done: bool,
                text: &'a str,
                hash: &'a str,
                thread_id: &'a str,
                thread_name: &'a str,
                thread_path: &'a str,
            }
            let items: Vec<_> = agenda
                .iter()
                .map(|a| YamlItem {
                    done: a.done,
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
            println!("DONE | TEXT | HASH | THREAD_ID | NAME | PATH");
            for a in &agenda {
                println!(
                    "{} | {} | {} | {} | {} | {}",
                    a.done, a.text, a.hash, a.thread_id, a.thread_name, a.thread_path
                );
            }
        }
        _ => {
            for a in &agenda {
                let mark = if a.done { "[x]" } else { "[ ]" };
                println!(
                    "{}  {}  {}  {}",
                    mark,
                    a.text,
                    a.hash.dimmed(),
                    format!("[{}]", a.thread_id).dimmed()
                );
            }
        }
    }

    Ok(())
}

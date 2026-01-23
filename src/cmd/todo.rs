use std::path::Path;

use clap::Args;
use clap_complete::engine::ArgValueCompleter;

use crate::config::{env_bool, is_quiet, resolve_section_name, Config};
use crate::git;
use crate::thread::{self, Thread};
use crate::workspace;

#[derive(Args)]
pub struct TodoArgs {
    /// Thread ID or name reference
    #[arg(add = ArgValueCompleter::new(crate::workspace::complete_thread_ids))]
    id: String,

    /// Action: add, check, uncheck, remove
    action: String,

    /// Item text or hash (depending on action)
    #[arg(default_value = "")]
    item: String,

    /// Commit after editing
    #[arg(long)]
    commit: bool,

    /// Commit message
    #[arg(short = 'm', long)]
    m: Option<String>,
}

pub fn run(args: TodoArgs, ws: &Path, config: &Config) -> Result<(), String> {
    // Get configured section name (or error if disabled)
    let section_name = resolve_section_name(&config.sections, "Todo")
        .ok_or("Todo section is disabled in config")?;

    let file = workspace::find_by_ref(ws, &args.id)?;

    let mut t = Thread::parse(&file)?;

    match args.action.as_str() {
        "add" => {
            if args.item.is_empty() {
                return Err("usage: threads todo <id> add \"item text\"".to_string());
            }
            let text = &args.item;

            let (new_content, hash) = thread::add_todo_item(&t.content, text);
            t.content = new_content;

            println!("Added to Todo: {} (id: {})", text, hash);
        }
        "check" | "complete" | "done" => {
            if args.item.is_empty() {
                return Err("usage: threads todo <id> check <hash>".to_string());
            }
            let hash = &args.item;

            // Check for ambiguous hash
            let count = thread::count_matching_items(&t.content, section_name, hash);
            if count == 0 {
                return Err(format!("no unchecked item with hash '{}' found", hash));
            }
            if count > 1 {
                return Err(format!("ambiguous hash '{}' matches {} items", hash, count));
            }

            t.content = thread::set_todo_checked(&t.content, section_name, hash, true)?;

            println!("Checked item {}", hash);
        }
        "uncheck" => {
            if args.item.is_empty() {
                return Err("usage: threads todo <id> uncheck <hash>".to_string());
            }
            let hash = &args.item;

            // Check for ambiguous hash
            let count = thread::count_matching_items(&t.content, section_name, hash);
            if count == 0 {
                return Err(format!("no checked item with hash '{}' found", hash));
            }
            if count > 1 {
                return Err(format!("ambiguous hash '{}' matches {} items", hash, count));
            }

            t.content = thread::set_todo_checked(&t.content, section_name, hash, false)?;

            println!("Unchecked item {}", hash);
        }
        "remove" => {
            if args.item.is_empty() {
                return Err("usage: threads todo <id> remove <hash>".to_string());
            }
            let hash = &args.item;

            // Check for ambiguous hash
            let count = thread::count_matching_items(&t.content, section_name, hash);
            if count == 0 {
                return Err(format!("no item with hash '{}' found", hash));
            }
            if count > 1 {
                return Err(format!("ambiguous hash '{}' matches {} items", hash, count));
            }

            t.content = thread::remove_by_hash(&t.content, section_name, hash)?;

            println!("Removed item {}", hash);
        }
        _ => {
            return Err(format!(
                "unknown action '{}'. Use: add, check, uncheck, remove",
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
        println!(
            "Note: Thread {} has uncommitted changes. Use 'threads commit {}' when ready.",
            args.id, args.id
        );
    }

    Ok(())
}

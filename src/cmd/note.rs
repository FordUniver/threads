use std::path::Path;

use clap::Args;
use clap_complete::engine::ArgValueCompleter;

use crate::config::{env_bool, is_quiet, Config};
use crate::git;
use crate::thread::{self, Thread};
use crate::workspace;

#[derive(Args)]
pub struct NoteArgs {
    /// Thread ID or name reference
    #[arg(add = ArgValueCompleter::new(crate::workspace::complete_thread_ids))]
    id: String,

    /// Action: add, edit, remove
    action: String,

    /// Note text (for add) or hash reference (for edit/remove)
    #[arg(default_value = "")]
    text: String,

    /// New text when editing (edit action only)
    #[arg(default_value = "")]
    new_text: String,

    /// Commit after editing
    #[arg(long)]
    commit: bool,

    /// Commit message
    #[arg(short = 'm', long)]
    m: Option<String>,
}

pub fn run(args: NoteArgs, ws: &Path, config: &Config) -> Result<(), String> {
    let file = workspace::find_by_ref(ws, &args.id)?;

    let mut t = Thread::parse(&file)?;

    match args.action.as_str() {
        "add" => {
            if args.text.is_empty() {
                return Err("usage: threads note <id> add \"text\"".to_string());
            }
            let text = &args.text;

            let (new_content, hash) = thread::add_note(&t.content, text);
            t.content = new_content;

            // Add log entry
            let log_entry = format!("Added note: {}", text);
            t.content = thread::insert_log_entry(&t.content, &log_entry);

            println!("Added note: {} (id: {})", text, hash);
        }
        "edit" => {
            if args.text.is_empty() || args.new_text.is_empty() {
                return Err("usage: threads note <id> edit <hash> \"new text\"".to_string());
            }
            let hash = &args.text;
            let new_text = &args.new_text;

            // Check for ambiguous hash
            let count = thread::count_matching_items(&t.content, "Notes", hash);
            if count == 0 {
                return Err(format!("no note with hash '{}' found", hash));
            }
            if count > 1 {
                return Err(format!("ambiguous hash '{}' matches {} notes", hash, count));
            }

            t.content = thread::edit_by_hash(&t.content, "Notes", hash, new_text)?;

            let log_entry = format!("Edited note {}", hash);
            t.content = thread::insert_log_entry(&t.content, &log_entry);

            println!("Edited note {}", hash);
        }
        "remove" => {
            if args.text.is_empty() {
                return Err("usage: threads note <id> remove <hash>".to_string());
            }
            let hash = &args.text;

            // Check for ambiguous hash
            let count = thread::count_matching_items(&t.content, "Notes", hash);
            if count == 0 {
                return Err(format!("no note with hash '{}' found", hash));
            }
            if count > 1 {
                return Err(format!("ambiguous hash '{}' matches {} notes", hash, count));
            }

            t.content = thread::remove_by_hash(&t.content, "Notes", hash)?;

            let log_entry = format!("Removed note {}", hash);
            t.content = thread::insert_log_entry(&t.content, &log_entry);

            println!("Removed note {}", hash);
        }
        _ => {
            return Err(format!(
                "unknown action '{}'. Use: add, edit, remove",
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

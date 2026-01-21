use std::path::Path;

use clap::Args;
use clap_complete::engine::ArgValueCompleter;

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

    /// Text or hash (depending on action)
    #[arg(default_value = "")]
    arg1: String,

    /// New text for edit action
    #[arg(default_value = "")]
    arg2: String,

    /// Commit after editing
    #[arg(long)]
    commit: bool,

    /// Commit message
    #[arg(short = 'm', long)]
    m: Option<String>,
}

pub fn run(args: NoteArgs, ws: &Path) -> Result<(), String> {
    let file = workspace::find_by_ref(ws, &args.id)?;

    let mut t = Thread::parse(&file)?;

    match args.action.as_str() {
        "add" => {
            if args.arg1.is_empty() {
                return Err("usage: threads note <id> add \"text\"".to_string());
            }
            let text = &args.arg1;

            let (new_content, hash) = thread::add_note(&t.content, text);
            t.content = new_content;

            // Add log entry
            let log_entry = format!("Added note: {}", text);
            t.content = thread::insert_log_entry(&t.content, &log_entry);

            println!("Added note: {} (id: {})", text, hash);
        }
        "edit" => {
            if args.arg1.is_empty() || args.arg2.is_empty() {
                return Err("usage: threads note <id> edit <hash> \"new text\"".to_string());
            }
            let hash = &args.arg1;
            let new_text = &args.arg2;

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
            if args.arg1.is_empty() {
                return Err("usage: threads note <id> remove <hash>".to_string());
            }
            let hash = &args.arg1;

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

    if args.commit {
        let msg = args.m.unwrap_or_else(|| {
            git::generate_commit_message(ws, &[file.to_string_lossy().to_string()])
        });
        git::auto_commit(ws, &file, &msg)?;
    } else {
        println!(
            "Note: Thread {} has uncommitted changes. Use 'threads commit {}' when ready.",
            args.id, args.id
        );
    }

    Ok(())
}

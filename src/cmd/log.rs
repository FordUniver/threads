use std::path::Path;

use clap::Args;
use clap_complete::engine::ArgValueCompleter;

use crate::config::env_bool;
use crate::git;
use crate::input;
use crate::thread::{self, Thread};
use crate::workspace;

#[derive(Args)]
pub struct LogArgs {
    /// Thread ID or name reference
    #[arg(add = ArgValueCompleter::new(crate::workspace::complete_thread_ids))]
    id: String,

    /// Log entry text
    #[arg(default_value = "")]
    entry: String,

    /// Commit after adding
    #[arg(long)]
    commit: bool,

    /// Commit message
    #[arg(short = 'm', long)]
    m: Option<String>,
}

pub fn run(args: LogArgs, ws: &Path) -> Result<(), String> {
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

    t.content = thread::insert_log_entry(&t.content, &entry);

    t.write()?;

    println!("Logged to: {}", file.display());

    let should_commit = args.commit || env_bool("THREADS_AUTO_COMMIT").unwrap_or(false);
    if should_commit {
        let repo = workspace::open()?;
        let rel_path = file.strip_prefix(ws).unwrap_or(&file);
        let msg = args
            .m
            .unwrap_or_else(|| git::generate_commit_message(&repo, &[rel_path]));
        git::auto_commit(&repo, &file, &msg)?;
    } else if !env_bool("THREADS_QUIET").unwrap_or(false) {
        println!(
            "Note: Thread {} has uncommitted changes. Use 'threads commit {}' when ready.",
            args.id, args.id
        );
    }

    Ok(())
}

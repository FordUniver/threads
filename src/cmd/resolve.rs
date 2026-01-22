use std::path::Path;

use clap::Args;
use clap_complete::engine::ArgValueCompleter;

use crate::git;
use crate::thread::{self, Thread};
use crate::workspace;

#[derive(Args)]
pub struct ResolveArgs {
    /// Thread ID or name reference
    #[arg(add = ArgValueCompleter::new(crate::workspace::complete_thread_ids))]
    id: String,

    /// Commit after resolving
    #[arg(long)]
    commit: bool,

    /// Commit message
    #[arg(short = 'm', long)]
    m: Option<String>,
}

pub fn run(args: ResolveArgs, ws: &Path) -> Result<(), String> {
    let file = workspace::find_by_ref(ws, &args.id)?;

    let mut t = Thread::parse(&file)?;

    let old_status = t.status().to_string();

    // Update status
    t.set_frontmatter_field("status", "resolved")?;

    // Add log entry
    t.content = thread::insert_log_entry(&t.content, "Resolved.");

    t.write()?;

    println!("Resolved: {} → resolved ({})", old_status, file.display());

    if args.commit {
        let repo = workspace::open()?;
        let rel_path = file.strip_prefix(ws).unwrap_or(&file);
        let msg = args.m.unwrap_or_else(|| {
            git::generate_commit_message(&repo, &[rel_path])
        });
        git::auto_commit(&repo, &file, &msg)?;
    } else {
        println!(
            "Note: Thread {} has uncommitted changes. Use 'threads commit {}' when ready.",
            args.id, args.id
        );
    }

    Ok(())
}

use std::path::Path;

use clap::Args;

use crate::git;
use crate::thread::{self, Thread};
use crate::workspace;

#[derive(Args)]
pub struct StatusArgs {
    /// Thread ID or name reference
    id: String,

    /// New status
    new_status: String,

    /// Commit after changing
    #[arg(long)]
    commit: bool,

    /// Commit message
    #[arg(short = 'm', long)]
    m: Option<String>,
}

pub fn run(args: StatusArgs, ws: &Path) -> Result<(), String> {
    if !thread::is_valid_status(&args.new_status) {
        return Err(format!(
            "Invalid status '{}'. Must be one of: idea, planning, active, blocked, paused, resolved, superseded, deferred, reject",
            args.new_status
        ));
    }

    let file = workspace::find_by_ref(ws, &args.id)?;

    let mut t = Thread::parse(&file)?;
    let old_status = t.status().to_string();

    t.set_frontmatter_field("status", &args.new_status)?;
    t.write()?;

    println!("Status changed: {} → {} ({})", old_status, args.new_status, file.display());

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

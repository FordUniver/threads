use std::fs;
use std::path::Path;

use clap::Args;
use clap_complete::engine::ArgValueCompleter;

use crate::git;
use crate::thread::Thread;
use crate::workspace;

#[derive(Args)]
pub struct RemoveArgs {
    /// Thread ID or name reference
    #[arg(add = ArgValueCompleter::new(crate::workspace::complete_thread_ids))]
    id: String,

    /// Commit after removing
    #[arg(long)]
    commit: bool,

    /// Commit message
    #[arg(short = 'm', long)]
    m: Option<String>,
}

pub fn run(args: RemoveArgs, ws: &Path) -> Result<(), String> {
    let file = workspace::find_by_ref(ws, &args.id)?;

    let t = Thread::parse(&file)?;
    let name = t.name().to_string();
    let rel_path = file
        .strip_prefix(ws)
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| file.to_string_lossy().to_string());

    // Check if file is tracked
    let was_tracked = git::is_tracked(ws, &rel_path);

    // Remove file
    fs::remove_file(&file)
        .map_err(|e| format!("removing file: {}", e))?;

    println!("Removed: {}", file.display());

    if !was_tracked {
        println!("Note: Thread was never committed to git, no commit needed.");
        return Ok(());
    }

    if args.commit {
        let msg = args.m.unwrap_or_else(|| format!("threads: remove '{}'", name));
        git::add(ws, &[&rel_path])?;
        git::commit(ws, &[rel_path.clone()], &msg)?;
        eprintln!("Note: Changes are local. Push with 'git push' when ready.");
    } else {
        println!("Note: To commit this removal, run:");
        println!(
            "  git -C \"$WORKSPACE\" add \"{}\" && git -C \"$WORKSPACE\" commit -m \"threads: remove '{}'\"",
            rel_path, name
        );
    }

    Ok(())
}

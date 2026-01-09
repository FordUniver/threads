use std::io::{self, BufRead, Write};
use std::path::Path;

use clap::Args;

use crate::git;
use crate::workspace;

#[derive(Args)]
pub struct CommitArgs {
    /// Thread IDs to commit
    ids: Vec<String>,

    /// Commit all modified threads
    #[arg(long)]
    pending: bool,

    /// Commit message
    #[arg(short = 'm', long)]
    m: Option<String>,

    /// Auto-accept generated message
    #[arg(long)]
    auto: bool,
}

pub fn run(args: CommitArgs, ws: &Path) -> Result<(), String> {
    let mut files = Vec::new();

    if args.pending {
        // Collect all thread files with uncommitted changes
        let threads = workspace::find_all_threads(ws)?;

        for t in threads {
            let rel_path = t
                .strip_prefix(ws)
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| t.to_string_lossy().to_string());

            if git::has_changes(ws, &rel_path) {
                files.push(t.to_string_lossy().to_string());
            }
        }
    } else {
        // Resolve provided IDs to files
        if args.ids.is_empty() {
            return Err("provide thread IDs or use --pending".to_string());
        }

        for id in &args.ids {
            let file = workspace::find_by_ref(ws, id)?;
            let rel_path = file
                .strip_prefix(ws)
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| file.to_string_lossy().to_string());

            if !git::has_changes(ws, &rel_path) {
                println!("No changes in thread: {}", id);
                continue;
            }
            files.push(file.to_string_lossy().to_string());
        }
    }

    if files.is_empty() {
        println!("No threads to commit.");
        return Ok(());
    }

    // Generate commit message if not provided
    let msg = if let Some(m) = args.m {
        m
    } else {
        let generated = git::generate_commit_message(ws, &files);
        println!("Generated message: {}", generated);

        if !args.auto && is_terminal() {
            print!("Proceed? [Y/n] ");
            io::stdout().flush().ok();

            let mut response = String::new();
            io::stdin().lock().read_line(&mut response).ok();
            let response = response.trim().to_lowercase();

            if response == "n" || response == "no" {
                println!("Aborted.");
                return Ok(());
            }
        }

        generated
    };

    // Stage and commit
    let rel_paths: Vec<String> = files
        .iter()
        .map(|f| {
            Path::new(f)
                .strip_prefix(ws)
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| f.clone())
        })
        .collect();

    git::commit(ws, &rel_paths, &msg)?;

    if let Err(e) = git::push(ws) {
        eprintln!("WARNING: git push failed (commit succeeded): {}", e);
    }

    println!("Committed {} thread(s).", files.len());
    Ok(())
}

fn is_terminal() -> bool {
    use std::io::IsTerminal;
    io::stdin().is_terminal()
}

use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};

use clap::Args;
use clap_complete::engine::ArgValueCompleter;

use crate::git;
use crate::workspace;

#[derive(Args)]
pub struct CommitArgs {
    /// Thread IDs to commit
    #[arg(add = ArgValueCompleter::new(crate::workspace::complete_thread_ids))]
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
    // Open repository for git operations
    let repo = workspace::open()?;

    let mut files: Vec<PathBuf> = Vec::new();

    if args.pending {
        // Collect all thread files with uncommitted changes
        let threads = workspace::find_all_threads(ws)?;

        for t in threads {
            let rel_path = t
                .strip_prefix(ws)
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|_| t.clone());

            if git::has_changes(&repo, &rel_path) {
                files.push(t);
            }
        }

        // Also include deleted thread files
        let deleted = git::find_deleted_thread_files(&repo);
        files.extend(deleted);
    } else {
        // Resolve provided IDs to files
        if args.ids.is_empty() {
            return Err("provide thread IDs or use --pending".to_string());
        }

        for id in &args.ids {
            let file = workspace::find_by_ref(ws, id)?;
            let rel_path = file
                .strip_prefix(ws)
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|_| file.clone());

            if !git::has_changes(&repo, &rel_path) {
                println!("No changes in thread: {}", id);
                continue;
            }
            files.push(file);
        }
    }

    if files.is_empty() {
        println!("No threads to commit.");
        return Ok(());
    }

    // Convert to relative paths for git operations
    let rel_paths: Vec<PathBuf> = files
        .iter()
        .map(|f| {
            f.strip_prefix(ws)
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|_| f.clone())
        })
        .collect();

    // Generate commit message if not provided
    let msg = if let Some(m) = args.m {
        m
    } else {
        let path_refs: Vec<&Path> = rel_paths.iter().map(|p| p.as_path()).collect();
        let generated = git::generate_commit_message(&repo, &path_refs);
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
    let path_refs: Vec<&Path> = rel_paths.iter().map(|p| p.as_path()).collect();
    git::commit(&repo, &path_refs, &msg)?;

    println!("Committed {} thread(s).", files.len());
    eprintln!("Note: Changes are local. Push with 'git push' when ready.");
    Ok(())
}

fn is_terminal() -> bool {
    use std::io::IsTerminal;
    io::stdin().is_terminal()
}

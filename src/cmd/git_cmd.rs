use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};

use clap::{Args, Subcommand};
use clap_complete::engine::ArgValueCompleter;
use colored::Colorize;
use serde::Serialize;

use crate::git;
use crate::output::OutputFormat;
use crate::thread;
use crate::workspace;

#[derive(Args)]
pub struct GitArgs {
    #[command(subcommand)]
    action: Option<GitAction>,
}

#[derive(Subcommand)]
enum GitAction {
    /// Show pending thread changes (default)
    Status {
        /// Output format
        #[arg(short = 'f', long, value_enum, default_value = "pretty")]
        format: OutputFormat,

        /// Output as JSON (shorthand for --format=json)
        #[arg(long, conflicts_with = "format")]
        json: bool,
    },

    /// Commit thread changes
    Commit {
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
    },
}

pub fn run(args: GitArgs, ws: &Path) -> Result<(), String> {
    match args.action {
        None => status(ws, OutputFormat::Pretty, false),
        Some(GitAction::Status { format, json }) => status(ws, format, json),
        Some(GitAction::Commit {
            ids,
            pending,
            m,
            auto,
        }) => commit(ws, ids, pending, m, auto),
    }
}

// ============================================================================
// Status
// ============================================================================

#[derive(Debug, Clone, Serialize)]
struct PendingThread {
    id: String,
    name: String,
    path: String,
    change_type: String,
}

fn status(ws: &Path, format: OutputFormat, json: bool) -> Result<(), String> {
    let format = if json {
        OutputFormat::Json
    } else {
        format.resolve()
    };

    let repo = workspace::open()?;
    let threads = workspace::find_all_threads(ws)?;

    let mut pending: Vec<PendingThread> = Vec::new();

    // Check existing threads for modifications
    for t in threads {
        let rel_path = t
            .strip_prefix(ws)
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|_| t.clone());

        if git::has_changes(&repo, &rel_path) {
            let id = thread::extract_id_from_path(&t).unwrap_or_default();
            let name = thread::extract_name_from_path(&t);
            let change_type = if t.exists() { "modified" } else { "deleted" };

            pending.push(PendingThread {
                id,
                name,
                path: rel_path.to_string_lossy().to_string(),
                change_type: change_type.to_string(),
            });
        }
    }

    // Check for deleted thread files
    let deleted = git::find_deleted_thread_files(&repo);
    for del_path in deleted {
        let id = thread::extract_id_from_path(&del_path).unwrap_or_default();
        let name = thread::extract_name_from_path(&del_path);

        // Avoid duplicates (already handled above if file still exists in worktree)
        if !pending.iter().any(|p| p.path == del_path.to_string_lossy()) {
            pending.push(PendingThread {
                id,
                name,
                path: del_path.to_string_lossy().to_string(),
                change_type: "deleted".to_string(),
            });
        }
    }

    match format {
        OutputFormat::Pretty => output_status_pretty(&pending),
        OutputFormat::Plain => output_status_plain(&pending),
        OutputFormat::Json => output_status_json(&pending)?,
        OutputFormat::Yaml => output_status_yaml(&pending)?,
    }

    Ok(())
}

fn output_status_pretty(pending: &[PendingThread]) {
    if pending.is_empty() {
        println!("No pending thread changes");
        return;
    }

    println!(
        "{} thread(s) with uncommitted changes",
        pending.len().to_string().bold()
    );
    println!();

    for p in pending {
        let change_marker = match p.change_type.as_str() {
            "modified" => "M".yellow(),
            "deleted" => "D".red(),
            _ => "?".normal(),
        };

        println!("  {} {} {}", change_marker, p.id.dimmed(), p.name);
    }

    println!();
    println!(
        "{}",
        "Run 'threads git commit --pending' to commit all".dimmed()
    );
}

fn output_status_plain(pending: &[PendingThread]) {
    if pending.is_empty() {
        println!("No pending thread changes");
        return;
    }

    println!("{} pending", pending.len());
    println!();
    println!("TYPE | ID | NAME | PATH");

    for p in pending {
        println!("{} | {} | {} | {}", p.change_type, p.id, p.name, p.path);
    }
}

fn output_status_json(pending: &[PendingThread]) -> Result<(), String> {
    #[derive(Serialize)]
    struct Output {
        count: usize,
        pending: Vec<PendingThread>,
    }

    let output = Output {
        count: pending.len(),
        pending: pending.to_vec(),
    };

    let json = serde_json::to_string_pretty(&output)
        .map_err(|e| format!("JSON serialization failed: {}", e))?;
    println!("{}", json);
    Ok(())
}

fn output_status_yaml(pending: &[PendingThread]) -> Result<(), String> {
    #[derive(Serialize)]
    struct Output {
        count: usize,
        pending: Vec<PendingThread>,
    }

    let output = Output {
        count: pending.len(),
        pending: pending.to_vec(),
    };

    let yaml =
        serde_yaml::to_string(&output).map_err(|e| format!("YAML serialization failed: {}", e))?;
    print!("{}", yaml);
    Ok(())
}

// ============================================================================
// Commit
// ============================================================================

fn commit(
    ws: &Path,
    ids: Vec<String>,
    pending: bool,
    m: Option<String>,
    auto: bool,
) -> Result<(), String> {
    let repo = workspace::open()?;

    let mut files: Vec<PathBuf> = Vec::new();

    if pending {
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
        if ids.is_empty() {
            return Err("provide thread IDs or use --pending".to_string());
        }

        for id in &ids {
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
        println!("No threads to commit");
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
    let msg = if let Some(m) = m {
        m
    } else {
        let path_refs: Vec<&Path> = rel_paths.iter().map(|p| p.as_path()).collect();
        let generated = git::generate_commit_message(&repo, &path_refs);
        println!("Generated message: {}", generated);

        if !auto && is_terminal() {
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

    println!("Committed {} thread(s)", files.len());
    eprintln!(
        "{}",
        "Note: Changes are local. Push with 'git push' when ready.".dimmed()
    );
    Ok(())
}

fn is_terminal() -> bool {
    use std::io::IsTerminal;
    io::stdin().is_terminal()
}

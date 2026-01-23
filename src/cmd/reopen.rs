use std::path::Path;

use clap::Args;
use clap_complete::engine::ArgValueCompleter;
use serde::Serialize;

use crate::args::FormatArgs;
use crate::config::env_bool;
use crate::git;
use crate::output::OutputFormat;
use crate::thread::{self, Thread};
use crate::workspace;

#[derive(Args)]
pub struct ReopenArgs {
    /// Thread ID or name reference
    #[arg(add = ArgValueCompleter::new(crate::workspace::complete_thread_ids))]
    id: String,

    /// Status to reopen to
    #[arg(long, default_value = "active")]
    status: String,

    /// Commit after reopening
    #[arg(long)]
    commit: bool,

    /// Commit message
    #[arg(short = 'm', long)]
    m: Option<String>,

    #[command(flatten)]
    format: FormatArgs,
}

#[derive(Serialize)]
struct ReopenOutput {
    id: String,
    old_status: String,
    new_status: String,
    path: String,
    committed: bool,
}

pub fn run(args: ReopenArgs, ws: &Path) -> Result<(), String> {
    let format = args.format.resolve();

    if !thread::is_valid_status(&args.status) {
        return Err(format!(
            "Invalid status '{}'. Must be one of: idea, planning, active, blocked, paused, resolved, superseded, deferred, rejected",
            args.status
        ));
    }

    let file = workspace::find_by_ref(ws, &args.id)?;

    let mut t = Thread::parse(&file)?;

    let old_status = t.status().to_string();
    let id = t.id().to_string();

    // Update status
    t.set_frontmatter_field("status", &args.status)?;

    // Add log entry
    t.content = thread::insert_log_entry(&t.content, "Reopened.");

    t.write()?;

    let should_commit = args.commit || env_bool("THREADS_AUTO_COMMIT").unwrap_or(false);
    let committed = if should_commit {
        let repo = workspace::open()?;
        let rel_path = file.strip_prefix(ws).unwrap_or(&file);
        let msg = args
            .m
            .unwrap_or_else(|| git::generate_commit_message(&repo, &[rel_path]));
        git::auto_commit(&repo, &file, &msg)?;
        true
    } else {
        false
    };

    let rel_path = workspace::path_relative_to_git_root(ws, &file);

    match format {
        OutputFormat::Pretty | OutputFormat::Plain => {
            println!("Reopened: {} → {} ({})", old_status, args.status, rel_path);
            if !committed && !env_bool("THREADS_QUIET").unwrap_or(false) {
                println!(
                    "Note: Thread {} has uncommitted changes. Use 'threads commit {}' when ready.",
                    id, id
                );
            }
        }
        OutputFormat::Json => {
            let output = ReopenOutput {
                id,
                old_status,
                new_status: args.status,
                path: rel_path,
                committed,
            };
            let json = serde_json::to_string_pretty(&output)
                .map_err(|e| format!("JSON serialization failed: {}", e))?;
            println!("{}", json);
        }
        OutputFormat::Yaml => {
            let output = ReopenOutput {
                id,
                old_status,
                new_status: args.status,
                path: rel_path,
                committed,
            };
            let yaml = serde_yaml::to_string(&output)
                .map_err(|e| format!("YAML serialization failed: {}", e))?;
            print!("{}", yaml);
        }
    }

    Ok(())
}

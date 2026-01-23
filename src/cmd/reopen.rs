use std::path::Path;

use clap::Args;
use clap_complete::engine::ArgValueCompleter;
use serde::Serialize;

use crate::args::FormatArgs;
use crate::config::{env_bool, Config};
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

pub fn run(args: ReopenArgs, ws: &Path, config: &Config) -> Result<(), String> {
    let format = args.format.resolve();

    let file = workspace::find_by_ref(ws, &args.id)?;

    // Resolve status: CLI flag > git history > config default
    let new_status = if args.status != "active" {
        // User explicitly set --status
        args.status.clone()
    } else if let Some(prev) = git::previous_status(ws, &file, &config.status.closed) {
        // Found previous open status in git history
        prev
    } else {
        // Fall back to config default
        config.defaults.open.clone()
    };

    // Validate status using config status lists
    if !thread::is_valid_status_with_config(&new_status, &config.status.open, &config.status.closed)
    {
        let all_statuses: Vec<&str> = config
            .status
            .open
            .iter()
            .chain(config.status.closed.iter())
            .map(|s| s.as_str())
            .collect();
        return Err(format!(
            "Invalid status '{}'. Must be one of: {}",
            new_status,
            all_statuses.join(", ")
        ));
    }

    let mut t = Thread::parse(&file)?;

    let old_status = t.status().to_string();
    let id = t.id().to_string();

    // Update status
    t.set_frontmatter_field("status", &new_status)?;

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
            println!("Reopened: {} → {} ({})", old_status, new_status, rel_path);
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
                new_status,
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
                new_status,
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

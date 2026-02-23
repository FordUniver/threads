use std::path::Path;

use clap::Args;
use clap_complete::engine::ArgValueCompleter;
use serde::Serialize;

use crate::args::FormatArgs;
use crate::config::{Config, env_bool, is_quiet};
use crate::git;
use crate::output::{self, OutputFormat};
use crate::thread::Thread;
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

    #[command(flatten)]
    format: FormatArgs,
}

#[derive(Serialize)]
struct ResolveOutput {
    id: String,
    old_status: String,
    path: String,
    committed: bool,
}

pub fn run(args: ResolveArgs, ws: &Path, config: &Config) -> Result<(), String> {
    let format = args.format.resolve();

    let file = workspace::find_by_ref(ws, &args.id)?;

    let mut t = Thread::parse(&file)?;

    let old_status = t.status().to_string();
    let id = t.id().to_string();

    // Update status using config default
    let closed_status = &config.defaults.closed;
    t.set_frontmatter_field("status", closed_status)?;

    // Add log entry
    let log_msg = if closed_status == "resolved" {
        "Resolved.".to_string()
    } else {
        format!("Closed ({}).", closed_status)
    };
    t.insert_log_entry(&log_msg)?;

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
            println!("Closed: {} → {} ({})", old_status, closed_status, rel_path);
            if !committed && !is_quiet(config) {
                output::print_uncommitted_hint(&id);
            }
        }
        OutputFormat::Json => {
            let output = ResolveOutput {
                id,
                old_status,
                path: rel_path,
                committed,
            };
            let json = serde_json::to_string_pretty(&output)
                .map_err(|e| format!("JSON serialization failed: {}", e))?;
            println!("{}", json);
        }
        OutputFormat::Yaml => {
            let output = ResolveOutput {
                id,
                old_status,
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

use std::path::Path;

use clap::Args;
use clap_complete::engine::ArgValueCompleter;
use serde::Serialize;

use crate::git;
use crate::output::OutputFormat;
use crate::thread::{self, Thread};
use crate::workspace;

#[derive(Args)]
pub struct StatusArgs {
    /// Thread ID or name reference
    #[arg(add = ArgValueCompleter::new(crate::workspace::complete_thread_ids))]
    id: String,

    /// New status
    new_status: String,

    /// Commit after changing
    #[arg(long)]
    commit: bool,

    /// Commit message
    #[arg(short = 'm', long)]
    m: Option<String>,

    /// Output format
    #[arg(short = 'f', long, value_enum, default_value = "pretty")]
    format: OutputFormat,

    /// Output as JSON (shorthand for --format=json)
    #[arg(long, conflicts_with = "format")]
    json: bool,
}

#[derive(Serialize)]
struct StatusOutput {
    id: String,
    old_status: String,
    new_status: String,
    path: String,
    committed: bool,
}

pub fn run(args: StatusArgs, ws: &Path) -> Result<(), String> {
    let format = if args.json {
        OutputFormat::Json
    } else {
        args.format.resolve()
    };

    if !thread::is_valid_status(&args.new_status) {
        return Err(format!(
            "Invalid status '{}'. Must be one of: idea, planning, active, blocked, paused, resolved, superseded, deferred, rejected",
            args.new_status
        ));
    }

    let file = workspace::find_by_ref(ws, &args.id)?;

    let mut t = Thread::parse(&file)?;
    let old_status = t.status().to_string();
    let id = t.id().to_string();

    t.set_frontmatter_field("status", &args.new_status)?;
    t.write()?;

    let committed = if args.commit {
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
            println!(
                "Changed: {} → {} ({})",
                old_status, args.new_status, rel_path
            );
            if !committed {
                println!(
                    "Note: Thread {} has uncommitted changes. Use 'threads commit {}' when ready.",
                    id, id
                );
            }
        }
        OutputFormat::Json => {
            let output = StatusOutput {
                id,
                old_status,
                new_status: args.new_status,
                path: rel_path,
                committed,
            };
            let json = serde_json::to_string_pretty(&output)
                .map_err(|e| format!("JSON serialization failed: {}", e))?;
            println!("{}", json);
        }
        OutputFormat::Yaml => {
            let output = StatusOutput {
                id,
                old_status,
                new_status: args.new_status,
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

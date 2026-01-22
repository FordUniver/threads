use std::fs;
use std::path::Path;

use clap::Args;
use clap_complete::engine::ArgValueCompleter;
use serde::Serialize;

use crate::git;
use crate::output::OutputFormat;
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

    /// Output format
    #[arg(short = 'f', long, value_enum, default_value = "pretty")]
    format: OutputFormat,

    /// Output as JSON (shorthand for --format=json)
    #[arg(long, conflicts_with = "format")]
    json: bool,
}

#[derive(Serialize)]
struct RemoveOutput {
    id: String,
    name: String,
    path: String,
    was_tracked: bool,
    committed: bool,
}

pub fn run(args: RemoveArgs, ws: &Path) -> Result<(), String> {
    let format = if args.json {
        OutputFormat::Json
    } else {
        args.format.resolve()
    };

    let file = workspace::find_by_ref(ws, &args.id)?;

    let t = Thread::parse(&file)?;
    let id = t.id().to_string();
    let name = t.name().to_string();
    let rel_path = file
        .strip_prefix(ws)
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|_| file.clone());

    // Open repository for git operations
    let repo = workspace::open()?;

    // Check if file is tracked
    let was_tracked = git::is_tracked(&repo, &rel_path);

    // Remove file
    fs::remove_file(&file).map_err(|e| format!("removing file: {}", e))?;

    let committed = if was_tracked && args.commit {
        let msg = args
            .m
            .unwrap_or_else(|| format!("threads: remove '{}'", name));
        git::add(&repo, &[rel_path.as_path()])?;
        git::commit(&repo, &[rel_path.as_path()], &msg)?;
        true
    } else {
        false
    };

    let rel_path_str = rel_path.to_string_lossy().to_string();

    match format {
        OutputFormat::Pretty | OutputFormat::Plain => {
            println!("Removed: {}", rel_path_str);
            if !was_tracked {
                println!("Note: Thread was never committed to git, no commit needed.");
            } else if !committed {
                println!(
                    "Note: Run 'threads rm {} --commit' or use git to commit the deletion.",
                    id
                );
            }
        }
        OutputFormat::Json => {
            let output = RemoveOutput {
                id,
                name,
                path: rel_path_str,
                was_tracked,
                committed,
            };
            let json = serde_json::to_string_pretty(&output)
                .map_err(|e| format!("JSON serialization failed: {}", e))?;
            println!("{}", json);
        }
        OutputFormat::Yaml => {
            let output = RemoveOutput {
                id,
                name,
                path: rel_path_str,
                was_tracked,
                committed,
            };
            let yaml = serde_yaml::to_string(&output)
                .map_err(|e| format!("YAML serialization failed: {}", e))?;
            print!("{}", yaml);
        }
    }

    Ok(())
}

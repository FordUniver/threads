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
pub struct MoveArgs {
    /// Thread ID or name reference
    #[arg(add = ArgValueCompleter::new(crate::workspace::complete_thread_ids))]
    id: String,

    /// New path (git-root-relative, ./pwd-relative, or absolute)
    new_path: String,

    /// Commit after moving
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
struct MoveOutput {
    id: String,
    source: String,
    dest: String,
    scope: String,
    committed: bool,
}

pub fn run(args: MoveArgs, git_root: &Path) -> Result<(), String> {
    let format = if args.json {
        OutputFormat::Json
    } else {
        args.format.resolve()
    };

    // Find source thread
    let src_file = workspace::find_by_ref(git_root, &args.id)?;

    // Get the thread ID for output
    let t = Thread::parse(&src_file)?;
    let id = t.id().to_string();

    // Resolve destination scope
    let scope = workspace::infer_scope(git_root, Some(&args.new_path))
        .map_err(|e| format!("invalid path '{}': {}", args.new_path, e))?;

    // Ensure dest .threads/ exists
    fs::create_dir_all(&scope.threads_dir)
        .map_err(|e| format!("creating threads directory: {}", e))?;

    // Move file
    let filename = src_file
        .file_name()
        .ok_or_else(|| "invalid source file".to_string())?;
    let dest_file = scope.threads_dir.join(filename);

    if dest_file.exists() {
        return Err(format!(
            "thread already exists at destination: {}",
            dest_file.display()
        ));
    }

    let rel_src = workspace::path_relative_to_git_root(git_root, &src_file);

    fs::rename(&src_file, &dest_file).map_err(|e| format!("moving file: {}", e))?;

    let rel_dest = workspace::path_relative_to_git_root(git_root, &dest_file);

    // Commit if requested
    let committed = if args.commit {
        let repo = workspace::open()?;
        let rel_src_path = src_file.strip_prefix(git_root).unwrap_or(&src_file);
        let rel_dest_path = dest_file.strip_prefix(git_root).unwrap_or(&dest_file);

        git::add(&repo, &[rel_src_path, rel_dest_path])?;

        let msg = args.m.unwrap_or_else(|| {
            format!(
                "threads: move {} to {}",
                src_file.file_name().unwrap().to_string_lossy(),
                scope.level_desc
            )
        });

        git::commit(&repo, &[rel_src_path, rel_dest_path], &msg)?;
        true
    } else {
        false
    };

    match format {
        OutputFormat::Pretty | OutputFormat::Plain => {
            println!("Moved: {} → {}", rel_src, rel_dest);
            if !committed {
                println!(
                    "Note: Thread {} has uncommitted changes. Use 'threads commit {}' when ready.",
                    id, id
                );
            }
        }
        OutputFormat::Json => {
            let output = MoveOutput {
                id,
                source: rel_src,
                dest: rel_dest,
                scope: scope.level_desc,
                committed,
            };
            let json = serde_json::to_string_pretty(&output)
                .map_err(|e| format!("JSON serialization failed: {}", e))?;
            println!("{}", json);
        }
        OutputFormat::Yaml => {
            let output = MoveOutput {
                id,
                source: rel_src,
                dest: rel_dest,
                scope: scope.level_desc,
                committed,
            };
            let yaml = serde_yaml::to_string(&output)
                .map_err(|e| format!("YAML serialization failed: {}", e))?;
            print!("{}", yaml);
        }
    }

    Ok(())
}

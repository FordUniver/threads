use std::fs;
use std::path::Path;

use clap::Args;
use clap_complete::engine::ArgValueCompleter;

use crate::git;
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
}

pub fn run(args: MoveArgs, git_root: &Path) -> Result<(), String> {
    // Find source thread
    let src_file = workspace::find_by_ref(git_root, &args.id)?;

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

    fs::rename(&src_file, &dest_file).map_err(|e| format!("moving file: {}", e))?;

    let rel_dest = workspace::path_relative_to_git_root(git_root, &dest_file);

    println!("Moved to {}", scope.level_desc);
    println!("  → {}", rel_dest);

    // Commit if requested
    if args.commit {
        let rel_src = workspace::path_relative_to_git_root(git_root, &src_file);

        git::add(git_root, &[&rel_src, &rel_dest])?;

        let msg = args.m.unwrap_or_else(|| {
            format!(
                "threads: move {} to {}",
                src_file.file_name().unwrap().to_string_lossy(),
                scope.level_desc
            )
        });

        git::commit(git_root, &[rel_src, rel_dest], &msg)?;

        eprintln!("Note: Changes are local. Push with 'git push' when ready.");
    } else {
        println!("Note: Use --commit to commit this move");
    }

    Ok(())
}

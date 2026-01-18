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

    /// New path (category or category/project)
    new_path: String,

    /// Commit after moving
    #[arg(long)]
    commit: bool,

    /// Commit message
    #[arg(short = 'm', long)]
    m: Option<String>,
}

pub fn run(args: MoveArgs, ws: &Path) -> Result<(), String> {
    // Find source thread
    let src_file = workspace::find_by_ref(ws, &args.id)?;

    // Resolve destination scope
    let scope = workspace::infer_scope(ws, &args.new_path)
        .map_err(|_| format!("invalid path: {}", args.new_path))?;

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

    fs::rename(&src_file, &dest_file)
        .map_err(|e| format!("moving file: {}", e))?;

    let rel_dest = dest_file
        .strip_prefix(ws)
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| dest_file.to_string_lossy().to_string());

    println!("Moved to {}", scope.level_desc);
    println!("  → {}", rel_dest);

    // Commit if requested
    if args.commit {
        let rel_src = src_file
            .strip_prefix(ws)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| src_file.to_string_lossy().to_string());

        git::add(ws, &[&rel_src, &rel_dest])?;

        let msg = args.m.unwrap_or_else(|| {
            format!(
                "threads: move {} to {}",
                src_file.file_name().unwrap().to_string_lossy(),
                scope.level_desc
            )
        });

        git::commit(ws, &[rel_src, rel_dest], &msg)?;

        eprintln!("Note: Changes are local. Push with 'git push' when ready.");
    } else {
        println!("Note: Use --commit to commit this move");
    }

    Ok(())
}

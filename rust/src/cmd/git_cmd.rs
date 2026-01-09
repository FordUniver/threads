use std::path::Path;

use clap::Args;

use crate::git;
use crate::workspace;

#[derive(Args)]
pub struct GitArgs {}

pub fn run(_args: GitArgs, ws: &Path) -> Result<(), String> {
    let threads = workspace::find_all_threads(ws)?;

    let mut modified = Vec::new();
    for t in threads {
        let rel_path = t
            .strip_prefix(ws)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| t.to_string_lossy().to_string());

        if git::has_changes(ws, &rel_path) {
            modified.push(rel_path);
        }
    }

    if modified.is_empty() {
        println!("No pending thread changes.");
        return Ok(());
    }

    println!("Pending thread changes:");
    for f in &modified {
        println!("  {}", f);
    }
    println!();
    println!("Suggested:");
    println!(
        "  git add {} && git commit -m \"threads: update\" && git push",
        modified.join(" ")
    );

    Ok(())
}

use std::path::Path;

use clap::Args;

use crate::thread::{self, Thread};
use crate::workspace;

#[derive(Args)]
pub struct ValidateArgs {
    /// Path to validate (specific file or all)
    #[arg(default_value = "")]
    path: String,

    /// Include nested categories/projects
    #[arg(short = 'r', long)]
    recursive: bool,
}

pub fn run(args: ValidateArgs, ws: &Path) -> Result<(), String> {
    let files = if !args.path.is_empty() {
        let target = &args.path;
        let abs_path = if Path::new(target).is_absolute() {
            target.clone()
        } else {
            ws.join(target).to_string_lossy().to_string()
        };
        vec![abs_path.into()]
    } else {
        workspace::find_all_threads(ws)?
    };

    let mut errors = 0;

    for file in files {
        let rel_path = file
            .strip_prefix(ws)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| file.to_string_lossy().to_string());

        let mut issues = Vec::new();

        match Thread::parse(&file) {
            Ok(t) => {
                if t.name().is_empty() {
                    issues.push("missing name/title field".to_string());
                }
                if t.status().is_empty() {
                    issues.push("missing status field".to_string());
                } else if !thread::is_valid_status(t.status()) {
                    issues.push(format!("invalid status '{}'", thread::base_status(t.status())));
                }
            }
            Err(e) => {
                issues.push(format!("parse error: {}", e));
            }
        }

        if !issues.is_empty() {
            println!("WARN: {}: {}", rel_path, issues.join(", "));
            errors += 1;
        } else {
            println!("OK: {}", rel_path);
        }
    }

    if errors > 0 {
        return Err(format!("{} validation error(s)", errors));
    }

    Ok(())
}

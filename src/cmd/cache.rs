use std::fs;
use std::path::Path;

use clap::{Args, Subcommand};
use colored::Colorize;

use crate::cache::TimestampCache;
use crate::output::OutputFormat;
use crate::workspace;

#[derive(Args)]
pub struct CacheArgs {
    #[command(subcommand)]
    action: CacheAction,
}

#[derive(Subcommand)]
enum CacheAction {
    /// Show cache status and statistics
    Status {
        /// Output format
        #[arg(short = 'f', long, value_enum, default_value = "pretty")]
        format: OutputFormat,

        /// Output as JSON (shorthand for --format=json)
        #[arg(long, conflicts_with = "format")]
        json: bool,
    },

    /// Clear the timestamp cache
    Clear,
}

pub fn run(args: CacheArgs, ws: &Path) -> Result<(), String> {
    match args.action {
        CacheAction::Status { format, json } => status(ws, format, json),
        CacheAction::Clear => clear(ws),
    }
}

fn cache_path(ws: &Path) -> std::path::PathBuf {
    ws.join(".threads-config").join("cache.json")
}

fn status(ws: &Path, format: OutputFormat, json: bool) -> Result<(), String> {
    let format = if json {
        OutputFormat::Json
    } else {
        format.resolve()
    };

    let path = cache_path(ws);
    let exists = path.exists();

    let (file_count, head, size_bytes) = if exists {
        let cache = TimestampCache::load(ws);
        let size = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        (cache.files.len(), cache.head, size)
    } else {
        (0, String::new(), 0)
    };

    // Check if cache is current
    let is_current = if exists {
        let repo = workspace::open()?;
        let cache = TimestampCache::load(ws);
        cache.is_current(&repo)
    } else {
        false
    };

    match format {
        OutputFormat::Pretty => {
            if !exists {
                println!("Cache: {}", "not present".dimmed());
                println!("Location: {}", path.display());
            } else {
                let status_str = if is_current {
                    "current".green().to_string()
                } else {
                    "stale".yellow().to_string()
                };
                println!("Cache: {}", status_str);
                println!("Location: {}", path.display());
                println!("Files: {}", file_count);
                println!("Size: {}", format_size(size_bytes));
                println!(
                    "HEAD: {}",
                    if head.is_empty() {
                        "-".to_string()
                    } else {
                        head[..8.min(head.len())].to_string()
                    }
                );
            }
        }
        OutputFormat::Plain => {
            if !exists {
                println!("status: not_present");
                println!("path: {}", path.display());
            } else {
                println!("status: {}", if is_current { "current" } else { "stale" });
                println!("path: {}", path.display());
                println!("files: {}", file_count);
                println!("size: {}", size_bytes);
                println!("head: {}", head);
            }
        }
        OutputFormat::Json => {
            let output = serde_json::json!({
                "exists": exists,
                "path": path.to_string_lossy(),
                "current": is_current,
                "files": file_count,
                "size_bytes": size_bytes,
                "head": head,
            });
            println!("{}", serde_json::to_string_pretty(&output).unwrap());
        }
        OutputFormat::Yaml => {
            let output = serde_json::json!({
                "exists": exists,
                "path": path.to_string_lossy(),
                "current": is_current,
                "files": file_count,
                "size_bytes": size_bytes,
                "head": head,
            });
            println!("{}", serde_yaml::to_string(&output).unwrap());
        }
    }

    Ok(())
}

fn clear(ws: &Path) -> Result<(), String> {
    let path = cache_path(ws);

    if !path.exists() {
        println!("Cache not present");
        return Ok(());
    }

    let cache = TimestampCache::load(ws);
    let file_count = cache.files.len();

    fs::remove_file(&path).map_err(|e| format!("Failed to remove cache: {}", e))?;

    println!("Cleared cache ({} entries)", file_count);

    Ok(())
}

fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

use std::collections::HashMap;
use std::path::Path;

use clap::Args;
use serde::Serialize;

use crate::output::OutputFormat;
use crate::thread::Thread;
use crate::workspace;

#[derive(Args)]
pub struct StatsArgs {
    /// Path to show stats for (git-root-relative, ./pwd-relative, or absolute)
    #[arg(default_value = "")]
    path: String,

    /// Include nested directories (recursive)
    #[arg(short = 'r', long)]
    recursive: bool,

    /// Output format (auto-detects TTY for fancy vs plain)
    #[arg(short = 'f', long, value_enum, default_value = "fancy")]
    format: OutputFormat,

    /// Output as JSON (shorthand for --format=json)
    #[arg(long, conflicts_with = "format")]
    json: bool,
}

#[derive(Serialize)]
struct StatusCount {
    status: String,
    count: usize,
}

pub fn run(args: StatsArgs, git_root: &Path) -> Result<(), String> {
    // Determine output format (handle --json shorthand)
    let format = if args.json {
        OutputFormat::Json
    } else {
        args.format.resolve()
    };

    // Parse path filter
    let path_filter = if args.path.is_empty() {
        None
    } else {
        Some(args.path.as_str())
    };

    // Resolve the scope
    let scope = workspace::infer_scope(git_root, path_filter)?;
    let filter_path = scope.path.clone();

    // Find all threads
    let threads = workspace::find_all_threads(git_root)?;

    let mut counts: HashMap<String, usize> = HashMap::new();
    let mut total = 0;

    for path in threads {
        let rel_path = workspace::parse_thread_path(git_root, &path);

        // Path filter: if not recursive, only show threads at the specified level
        if !args.recursive {
            if rel_path != filter_path {
                continue;
            }
        } else {
            // Recursive mode: show threads at or under the filter path
            if filter_path != "." {
                let filter_prefix = if filter_path.ends_with('/') {
                    filter_path.clone()
                } else {
                    format!("{}/", filter_path)
                };
                if rel_path != filter_path && !rel_path.starts_with(&filter_prefix) {
                    continue;
                }
            }
        }

        let t = match Thread::parse(&path) {
            Ok(t) => t,
            Err(_) => continue,
        };

        let status = t.base_status();
        let status = if status.is_empty() {
            "(none)".to_string()
        } else {
            status
        };

        *counts.entry(status).or_insert(0) += 1;
        total += 1;
    }

    // Sort by count descending
    let mut sorted: Vec<_> = counts.into_iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(&a.1));

    match format {
        OutputFormat::Fancy => output_fancy(&sorted, total, &filter_path, args.recursive),
        OutputFormat::Plain => output_plain(&sorted, total, git_root, &filter_path, args.recursive),
        OutputFormat::Json => output_json(&sorted, total, git_root, &filter_path),
        OutputFormat::Yaml => output_yaml(&sorted, total, git_root, &filter_path),
    }
}

fn output_fancy(
    sorted: &[(String, usize)],
    total: usize,
    filter_path: &str,
    recursive: bool,
) -> Result<(), String> {
    let path_desc = if filter_path == "." {
        "repo root".to_string()
    } else {
        filter_path.to_string()
    };

    let recursive_suffix = if recursive { " (recursive)" } else { "" };

    println!("Stats for threads in {}{}", path_desc, recursive_suffix);
    println!();

    if total == 0 {
        println!("No threads found.");
        if !recursive {
            println!("Hint: use -r to include nested directories");
        }
        return Ok(());
    }

    println!("| Status     | Count |");
    println!("|------------|-------|");
    for (status, count) in sorted {
        println!("| {:<10} | {:>5} |", status, count);
    }
    println!("|------------|-------|");
    println!("| {:<10} | {:>5} |", "Total", total);

    Ok(())
}

fn output_plain(
    sorted: &[(String, usize)],
    total: usize,
    git_root: &Path,
    filter_path: &str,
    recursive: bool,
) -> Result<(), String> {
    let pwd = std::env::current_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| "(unknown)".to_string());

    println!("PWD: {}", pwd);
    println!("Git root: {}", git_root.display());
    println!();

    let path_desc = if filter_path == "." {
        "repo root".to_string()
    } else {
        filter_path.to_string()
    };

    let recursive_suffix = if recursive { " (recursive)" } else { "" };

    println!("Stats for threads in {}{}", path_desc, recursive_suffix);
    println!();

    if total == 0 {
        println!("No threads found.");
        if !recursive {
            println!("Hint: use -r to include nested directories");
        }
        return Ok(());
    }

    println!("| Status     | Count |");
    println!("|------------|-------|");
    for (status, count) in sorted {
        println!("| {:<10} | {:>5} |", status, count);
    }
    println!("|------------|-------|");
    println!("| {:<10} | {:>5} |", "Total", total);

    Ok(())
}

fn output_json(
    sorted: &[(String, usize)],
    total: usize,
    git_root: &Path,
    filter_path: &str,
) -> Result<(), String> {
    #[derive(Serialize)]
    struct JsonOutput {
        git_root: String,
        path: String,
        counts: Vec<StatusCount>,
        total: usize,
    }

    let counts: Vec<StatusCount> = sorted
        .iter()
        .map(|(status, count)| StatusCount {
            status: status.clone(),
            count: *count,
        })
        .collect();

    let output = JsonOutput {
        git_root: git_root.to_string_lossy().to_string(),
        path: filter_path.to_string(),
        counts,
        total,
    };

    let json = serde_json::to_string_pretty(&output)
        .map_err(|e| format!("JSON serialization failed: {}", e))?;
    println!("{}", json);
    Ok(())
}

fn output_yaml(
    sorted: &[(String, usize)],
    total: usize,
    git_root: &Path,
    filter_path: &str,
) -> Result<(), String> {
    #[derive(Serialize)]
    struct YamlOutput {
        git_root: String,
        path: String,
        counts: Vec<StatusCount>,
        total: usize,
    }

    let counts: Vec<StatusCount> = sorted
        .iter()
        .map(|(status, count)| StatusCount {
            status: status.clone(),
            count: *count,
        })
        .collect();

    let output = YamlOutput {
        git_root: git_root.to_string_lossy().to_string(),
        path: filter_path.to_string(),
        counts,
        total,
    };

    let yaml = serde_yaml::to_string(&output)
        .map_err(|e| format!("YAML serialization failed: {}", e))?;
    print!("{}", yaml);
    Ok(())
}

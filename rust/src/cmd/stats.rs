use std::collections::HashMap;
use std::path::Path;

use clap::Args;
use serde::Serialize;

use crate::output::OutputFormat;
use crate::thread::Thread;
use crate::workspace::{self, FindOptions};

#[derive(Args)]
pub struct StatsArgs {
    /// Path to show stats for (git-root-relative, ./pwd-relative, or absolute)
    #[arg(default_value = "")]
    path: String,

    /// Search subdirectories (unlimited depth, or specify N levels)
    #[arg(short = 'd', long = "down", value_name = "N")]
    down: Option<Option<usize>>,

    /// Alias for --down (backward compatibility)
    #[arg(short = 'r', long, conflicts_with = "down")]
    recursive: bool,

    /// Search parent directories (up to git root, or specify N levels)
    #[arg(short = 'u', long = "up", value_name = "N")]
    up: Option<Option<usize>>,

    /// Cross git boundaries when searching down (enter nested repos)
    #[arg(long)]
    no_git_bound_down: bool,

    /// Cross git boundaries when searching up (continue past git root)
    #[arg(long)]
    no_git_bound_up: bool,

    /// Cross all git boundaries (alias for --no-git-bound-up --no-git-bound-down)
    #[arg(long)]
    no_git_bound: bool,

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

/// Describes the search direction for output display.
#[derive(Clone)]
struct SearchDirection {
    down: Option<Option<usize>>,
    up: Option<Option<usize>>,
    has_down: bool,
    has_up: bool,
}

impl SearchDirection {
    fn description(&self) -> String {
        let mut parts = Vec::new();

        if self.has_down {
            match self.down {
                Some(Some(n)) => parts.push(format!("down {}", n)),
                Some(None) | None => parts.push("recursive".to_string()),
            }
        }

        if self.has_up {
            match self.up {
                Some(Some(n)) => parts.push(format!("up {}", n)),
                Some(None) => parts.push("up".to_string()),
                None => {}
            }
        }

        if parts.is_empty() {
            String::new()
        } else {
            format!(" ({})", parts.join(", "))
        }
    }

    fn is_searching(&self) -> bool {
        self.has_down || self.has_up
    }
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
    let start_path = scope.threads_dir.parent().unwrap_or(git_root);

    // Build FindOptions from flags
    let no_git_bound_down = args.no_git_bound || args.no_git_bound_down;
    let no_git_bound_up = args.no_git_bound || args.no_git_bound_up;

    // Determine search direction: --down/-d takes priority, then -r as alias
    let down_opt = if args.down.is_some() {
        args.down
    } else if args.recursive {
        Some(None) // unlimited depth
    } else {
        None
    };

    let options = FindOptions::new()
        .with_no_git_bound_down(no_git_bound_down)
        .with_no_git_bound_up(no_git_bound_up);

    let options = if let Some(depth) = down_opt {
        options.with_down(depth)
    } else {
        options
    };

    let options = if let Some(depth) = args.up {
        options.with_up(depth)
    } else {
        options
    };

    // Track search direction for output
    let search_dir = SearchDirection {
        down: down_opt,
        up: args.up,
        has_down: down_opt.is_some(),
        has_up: args.up.is_some(),
    };

    // Find threads using options
    let threads = workspace::find_threads_with_options(start_path, git_root, &options)?;

    let mut counts: HashMap<String, usize> = HashMap::new();
    let mut total = 0;

    for path in threads {
        let rel_path = workspace::parse_thread_path(git_root, &path);

        // Path filter: if not searching, only count threads at the specified level
        if !search_dir.is_searching() {
            if rel_path != filter_path {
                continue;
            }
        }
        // Note: find_threads_with_options already handles direction/depth filtering

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
        OutputFormat::Fancy => output_fancy(&sorted, total, &filter_path, &search_dir),
        OutputFormat::Plain => output_plain(&sorted, total, git_root, &filter_path, &search_dir),
        OutputFormat::Json => output_json(&sorted, total, git_root, &filter_path),
        OutputFormat::Yaml => output_yaml(&sorted, total, git_root, &filter_path),
    }
}

fn output_fancy(
    sorted: &[(String, usize)],
    total: usize,
    filter_path: &str,
    search_dir: &SearchDirection,
) -> Result<(), String> {
    let path_desc = if filter_path == "." {
        "repo root".to_string()
    } else {
        filter_path.to_string()
    };

    let search_suffix = search_dir.description();

    println!("Stats for threads in {}{}", path_desc, search_suffix);
    println!();

    if total == 0 {
        println!("No threads found.");
        if !search_dir.is_searching() {
            println!("Hint: use -r to include nested directories, -u to search parents");
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
    search_dir: &SearchDirection,
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

    let search_suffix = search_dir.description();

    println!("Stats for threads in {}{}", path_desc, search_suffix);
    println!();

    if total == 0 {
        println!("No threads found.");
        if !search_dir.is_searching() {
            println!("Hint: use -r to include nested directories, -u to search parents");
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

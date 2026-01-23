use std::collections::HashMap;
use std::path::Path;

use clap::Args;
use colored::Colorize;
use serde::Serialize;
use tabled::settings::Style;
use tabled::{Table, Tabled};

use crate::output::{self, OutputFormat};
use crate::thread::{self, Thread};
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

    /// Include concluded threads (resolved/superseded/deferred/rejected)
    #[arg(short = 'c', long = "include-concluded")]
    include_concluded: bool,

    /// Output format (auto-detects TTY for pretty vs plain)
    #[arg(short = 'f', long, value_enum, default_value = "pretty")]
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

    // Determine search direction: --down/-d takes priority, then -r as alias
    let down_opt = if args.down.is_some() {
        args.down
    } else if args.recursive {
        Some(None) // unlimited depth
    } else {
        None
    };

    let mut options = FindOptions::new();

    if let Some(depth) = down_opt {
        options = options.with_down(depth);
    }

    if let Some(depth) = args.up {
        options = options.with_up(depth);
    }

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
        if !search_dir.is_searching() && rel_path != filter_path {
            continue;
        }
        // Note: find_threads_with_options already handles direction/depth filtering

        let t = match Thread::parse(&path) {
            Ok(t) => t,
            Err(_) => continue,
        };

        let status = t.base_status();

        // Filter out terminal threads unless --include-concluded
        if !args.include_concluded && thread::is_terminal(&status) {
            continue;
        }

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
        OutputFormat::Pretty => output_pretty(
            &sorted,
            total,
            &filter_path,
            &search_dir,
            args.include_concluded,
        ),
        OutputFormat::Plain => output_plain(
            &sorted,
            total,
            git_root,
            &filter_path,
            &search_dir,
            args.include_concluded,
        ),
        OutputFormat::Json => output_json(&sorted, total, git_root, &filter_path),
        OutputFormat::Yaml => output_yaml(&sorted, total, git_root, &filter_path),
    }
}

/// Row data for stats table
#[derive(Tabled)]
struct StatsRow {
    #[tabled(rename = "STATUS")]
    status: String,
    #[tabled(rename = "COUNT")]
    count: String,
}

/// Build filter description for summary line
fn build_filter_desc(include_concluded: bool, search_dir: &SearchDirection) -> String {
    let mut parts = Vec::new();

    if !include_concluded {
        parts.push("open".to_string());
    } else {
        parts.push("all statuses".to_string());
    }

    let dir_desc = search_dir.description();
    if !dir_desc.is_empty() {
        parts.push(dir_desc.trim().to_string());
    }

    parts.join(", ")
}

fn output_pretty(
    sorted: &[(String, usize)],
    total: usize,
    filter_path: &str,
    search_dir: &SearchDirection,
    include_concluded: bool,
) -> Result<(), String> {
    let path_desc = if filter_path == "." {
        "repo root".to_string()
    } else {
        filter_path.to_string()
    };

    let filter_desc = build_filter_desc(include_concluded, search_dir);

    println!(
        "{} {} ({})",
        "Stats for threads in".bold(),
        path_desc,
        filter_desc.dimmed()
    );
    println!();

    if total == 0 {
        println!("{}", "No threads found.".dimmed());
        if !search_dir.is_searching() {
            println!(
                "{}",
                "Hint: use -r to include nested directories, -u to search parents".dimmed()
            );
        }
        return Ok(());
    }

    // Build table rows with styled status
    let mut rows: Vec<StatsRow> = sorted
        .iter()
        .map(|(status, count)| StatsRow {
            status: output::style_status(status).to_string(),
            count: count.to_string(),
        })
        .collect();

    // Add total row
    rows.push(StatsRow {
        status: "Total".bold().to_string(),
        count: total.to_string().bold().to_string(),
    });

    let mut table = Table::new(rows);
    table.with(Style::rounded());

    println!("{}", table);

    Ok(())
}

fn output_plain(
    sorted: &[(String, usize)],
    total: usize,
    git_root: &Path,
    filter_path: &str,
    search_dir: &SearchDirection,
    include_concluded: bool,
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

    let filter_desc = build_filter_desc(include_concluded, search_dir);

    println!("Stats for threads in {} ({})", path_desc, filter_desc);
    println!();

    if total == 0 {
        println!("No threads found.");
        if !search_dir.is_searching() {
            println!("Hint: use -r to include nested directories, -u to search parents");
        }
        return Ok(());
    }

    // Plain pipe-delimited format
    println!("STATUS | COUNT");
    for (status, count) in sorted {
        println!("{} | {}", status, count);
    }
    println!("Total | {}", total);

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

    let yaml =
        serde_yaml::to_string(&output).map_err(|e| format!("YAML serialization failed: {}", e))?;
    print!("{}", yaml);
    Ok(())
}

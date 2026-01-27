use std::collections::HashMap;
use std::path::Path;

use clap::Args;
use colored::Colorize;
use serde::Serialize;
use tabled::settings::Style;
use tabled::{Table, Tabled};

use crate::args::{DirectionArgs, FilterArgs, FormatArgs};
use crate::config::{Config, is_quiet, root_name};
use crate::output::{self, OutputFormat};
use crate::thread::{self, Thread};
use crate::workspace;

#[derive(Args)]
pub struct StatsArgs {
    /// Path to show stats for (git-root-relative, ./pwd-relative, or absolute)
    #[arg(default_value = "")]
    path: String,

    #[command(flatten)]
    direction: DirectionArgs,

    #[command(flatten)]
    filter: FilterArgs,

    #[command(flatten)]
    format: FormatArgs,
}

#[derive(Serialize)]
struct StatusCount {
    status: String,
    count: usize,
}

pub fn run(args: StatsArgs, git_root: &Path, config: &Config) -> Result<(), String> {
    let format = args.format.resolve();

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

    // Convert direction args to find options
    let options = args.direction.to_find_options();

    // Find threads using options
    let threads = workspace::find_threads_with_options(start_path, git_root, &options)?;

    let mut counts: HashMap<String, usize> = HashMap::new();
    let mut total = 0;

    for path in threads {
        let rel_path = workspace::parse_thread_path(git_root, &path);

        // Path filter: if not searching, only count threads at the specified level
        if !args.direction.is_searching() && rel_path != filter_path {
            continue;
        }
        // Note: find_threads_with_options already handles direction/depth filtering

        let t = match Thread::parse(&path) {
            Ok(t) => t,
            Err(_) => continue,
        };

        let status = t.base_status();

        // Filter out closed threads unless --include-closed
        if !args.filter.include_closed() && thread::is_closed(&status) {
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

    let include_closed = args.filter.include_closed();

    match format {
        OutputFormat::Pretty => output_pretty(
            &sorted,
            total,
            &filter_path,
            &args.direction,
            include_closed,
            config,
        ),
        OutputFormat::Plain => output_plain(
            &sorted,
            total,
            git_root,
            &filter_path,
            &args.direction,
            include_closed,
            config,
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
fn build_filter_desc(include_closed: bool, direction: &DirectionArgs) -> String {
    let mut parts = Vec::new();

    if !include_closed {
        parts.push("open".to_string());
    } else {
        parts.push("all statuses".to_string());
    }

    let dir_desc = direction.description();
    if !dir_desc.is_empty() {
        parts.push(dir_desc);
    }

    parts.join(", ")
}

fn output_pretty(
    sorted: &[(String, usize)],
    total: usize,
    filter_path: &str,
    direction: &DirectionArgs,
    include_closed: bool,
    config: &Config,
) -> Result<(), String> {
    let path_desc = if filter_path == "." {
        root_name(config).to_string()
    } else {
        filter_path.to_string()
    };

    let filter_desc = build_filter_desc(include_closed, direction);

    println!(
        "{} {} ({})",
        "Stats for threads in".bold(),
        path_desc,
        filter_desc.dimmed()
    );
    println!();

    if total == 0 {
        println!("{}", "No threads found.".dimmed());
        if !direction.is_searching() && !is_quiet(config) {
            println!(
                "{}",
                "Hint: use --down to include nested directories, --up to search parents".dimmed()
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
    direction: &DirectionArgs,
    include_closed: bool,
    config: &Config,
) -> Result<(), String> {
    let pwd = std::env::current_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| "(unknown)".to_string());

    println!("PWD: {}", pwd);
    println!("Git root: {}", git_root.display());
    println!();

    let path_desc = if filter_path == "." {
        root_name(config).to_string()
    } else {
        filter_path.to_string()
    };

    let filter_desc = build_filter_desc(include_closed, direction);

    println!("Stats for threads in {} ({})", path_desc, filter_desc);
    println!();

    if total == 0 {
        println!("No threads found.");
        if !direction.is_searching() && !is_quiet(config) {
            println!("Hint: use --down to include nested directories, --up to search parents");
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

use std::collections::HashMap;
use std::path::Path;

use clap::Args;

use crate::thread::Thread;
use crate::workspace;

#[derive(Args)]
pub struct StatsArgs {
    /// Path to show stats for
    #[arg(default_value = "")]
    path: String,

    /// Include nested categories/projects
    #[arg(short = 'r', long)]
    recursive: bool,
}

pub fn run(args: StatsArgs, ws: &Path) -> Result<(), String> {
    // Parse path filter
    let mut category_filter: Option<String> = None;
    let mut project_filter: Option<String> = None;

    if !args.path.is_empty() {
        let path_filter = &args.path;
        let full_path = ws.join(path_filter);
        if full_path.is_dir() {
            let parts: Vec<&str> = path_filter.split('/').collect();
            category_filter = Some(parts[0].to_string());
            if parts.len() > 1 {
                project_filter = Some(parts[1].to_string());
            }
        }
    }

    // Find all threads
    let threads = workspace::find_all_threads(ws)?;

    let mut counts: HashMap<String, usize> = HashMap::new();
    let mut total = 0;

    for path in threads {
        let (category, project, _) = workspace::parse_thread_path(ws, &path);

        // Category filter
        if let Some(ref cat) = category_filter {
            if &category != cat {
                continue;
            }
        }

        // Project filter
        if let Some(ref proj) = project_filter {
            if &project != proj {
                continue;
            }
        }

        // Non-recursive: only threads at current hierarchy level
        if !args.recursive {
            if project_filter.is_some() {
                // At project level, count all
            } else if category_filter.is_some() {
                if project != "-" {
                    continue;
                }
            } else {
                if category != "-" {
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

    // Build scope description
    let (level_desc, path_suffix) = if project_filter.is_some() && category_filter.is_some() {
        (
            "project-level",
            format!(
                " ({}/{})",
                category_filter.as_ref().unwrap(),
                project_filter.as_ref().unwrap()
            ),
        )
    } else if category_filter.is_some() {
        (
            "category-level",
            format!(" ({})", category_filter.as_ref().unwrap()),
        )
    } else {
        ("workspace-level", String::new())
    };

    let recursive_suffix = if args.recursive {
        " (including nested)"
    } else {
        ""
    };

    println!(
        "Stats for {} threads{}{}",
        level_desc, path_suffix, recursive_suffix
    );
    println!();

    if total == 0 {
        println!("No threads found.");
        if !args.recursive {
            println!("Hint: use -r to include nested categories/projects");
        }
        return Ok(());
    }

    // Sort by count descending
    let mut sorted: Vec<_> = counts.into_iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(&a.1));

    println!("| Status     | Count |");
    println!("|------------|-------|");
    for (status, count) in &sorted {
        println!("| {:<10} | {:>5} |", status, count);
    }
    println!("|------------|-------|");
    println!("| {:<10} | {:>5} |", "Total", total);

    Ok(())
}

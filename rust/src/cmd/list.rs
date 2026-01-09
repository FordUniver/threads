use std::path::Path;

use clap::Args;
use serde::Serialize;

use crate::thread::{self, Thread};
use crate::workspace;

#[derive(Args)]
pub struct ListArgs {
    /// Path to list threads from
    #[arg(default_value = "")]
    path: String,

    /// Include nested categories/projects
    #[arg(short = 'r', long)]
    recursive: bool,

    /// Include resolved/terminal threads
    #[arg(long)]
    all: bool,

    /// Search name/title/desc (substring)
    #[arg(short = 's', long)]
    search: Option<String>,

    /// Filter by status
    #[arg(long)]
    status: Option<String>,

    /// Filter by category
    #[arg(short = 'c', long)]
    category: Option<String>,

    /// Filter by project
    #[arg(short = 'p', long)]
    project: Option<String>,

    /// Output as JSON
    #[arg(long)]
    json: bool,
}

#[derive(Serialize)]
struct ThreadInfo {
    id: String,
    status: String,
    category: String,
    project: String,
    name: String,
    title: String,
    desc: String,
}

pub fn run(args: ListArgs, ws: &Path) -> Result<(), String> {
    let mut category_filter = args.category.clone();
    let mut project_filter = args.project.clone();
    let mut search_filter = args.search.clone();

    // Parse path filter if provided
    if !args.path.is_empty() {
        let path_arg = &args.path;
        let full_path = ws.join(path_arg);
        if full_path.is_dir() {
            let parts: Vec<&str> = path_arg.split('/').collect();
            category_filter = Some(parts[0].to_string());
            if parts.len() > 1 {
                project_filter = Some(parts[1].to_string());
            }
        } else {
            // Treat as search filter
            search_filter = Some(path_arg.clone());
        }
    }

    let threads = workspace::find_all_threads(ws)?;
    let mut results = Vec::new();

    for path in threads {
        let t = match Thread::parse(&path) {
            Ok(t) => t,
            Err(_) => continue,
        };

        let (category, project, name) = workspace::parse_thread_path(ws, &path);
        let status = t.status().to_string();
        let base_status = thread::base_status(&status);

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
                // At project level, show all threads here
            } else if category_filter.is_some() {
                // At category level: only show category-level threads
                if project != "-" {
                    continue;
                }
            } else {
                // At workspace level: only show workspace-level threads
                if category != "-" {
                    continue;
                }
            }
        }

        // Status filter
        if let Some(ref status_filter) = args.status {
            let filter_statuses: Vec<&str> = status_filter.split(',').collect();
            if !filter_statuses.contains(&base_status.as_str()) {
                continue;
            }
        } else if !args.all && thread::is_terminal(&status) {
            continue;
        }

        // Search filter
        if let Some(ref search) = search_filter {
            let search_lower = search.to_lowercase();
            let name_lower = name.to_lowercase();
            let title_lower = t.name().to_lowercase();
            let desc_lower = t.frontmatter.desc.to_lowercase();

            if !name_lower.contains(&search_lower)
                && !title_lower.contains(&search_lower)
                && !desc_lower.contains(&search_lower)
            {
                continue;
            }
        }

        // Use title if available, else humanize name
        let title = if !t.name().is_empty() {
            t.name().to_string()
        } else {
            name.replace('-', " ")
        };

        results.push(ThreadInfo {
            id: t.id().to_string(),
            status: base_status,
            category,
            project,
            name,
            title,
            desc: t.frontmatter.desc.clone(),
        });
    }

    if args.json {
        output_json(&results)?;
    } else {
        output_table(&results, &category_filter, &project_filter, args.recursive, args.all, args.status.as_deref())?;
    }

    Ok(())
}

fn output_json(results: &[ThreadInfo]) -> Result<(), String> {
    let json = serde_json::to_string_pretty(results)
        .map_err(|e| format!("JSON serialization failed: {}", e))?;
    println!("{}", json);
    Ok(())
}

fn output_table(
    results: &[ThreadInfo],
    category_filter: &Option<String>,
    project_filter: &Option<String>,
    recursive: bool,
    all: bool,
    status_filter: Option<&str>,
) -> Result<(), String> {
    // Build header description
    let (level_desc, path_suffix) = if project_filter.is_some() && category_filter.is_some() {
        (
            "project-level",
            format!(" ({}/{})", category_filter.as_ref().unwrap(), project_filter.as_ref().unwrap()),
        )
    } else if category_filter.is_some() {
        (
            "category-level",
            format!(" ({})", category_filter.as_ref().unwrap()),
        )
    } else {
        ("workspace-level", String::new())
    };

    let status_desc = if let Some(s) = status_filter {
        s.to_string()
    } else if all {
        String::new()
    } else {
        "active".to_string()
    };

    let recursive_suffix = if recursive { " (including nested)" } else { "" };

    if !status_desc.is_empty() {
        println!(
            "Showing {} {} {} threads{}{}",
            results.len(),
            status_desc,
            level_desc,
            path_suffix,
            recursive_suffix
        );
    } else {
        println!(
            "Showing {} {} threads{} (all statuses){}",
            results.len(),
            level_desc,
            path_suffix,
            recursive_suffix
        );
    }
    println!();

    if results.is_empty() {
        if !recursive {
            println!("Hint: use -r to include nested categories/projects");
        }
        return Ok(());
    }

    // Print table header
    println!(
        "{:<6} {:<10} {:<18} {:<22} {}",
        "ID", "STATUS", "CATEGORY", "PROJECT", "NAME"
    );
    println!(
        "{:<6} {:<10} {:<18} {:<22} {}",
        "--", "------", "--------", "-------", "----"
    );

    for t in results {
        let category = truncate(&t.category, 16);
        let project = truncate(&t.project, 20);
        println!(
            "{:<6} {:<10} {:<18} {:<22} {}",
            t.id, t.status, category, project, t.title
        );
    }

    Ok(())
}

fn truncate(s: &str, max_chars: usize) -> String {
    let char_count = s.chars().count();
    if char_count <= max_chars {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max_chars - 1).collect();
        format!("{}…", truncated)
    }
}

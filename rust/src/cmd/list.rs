use std::path::Path;

use clap::Args;
use serde::Serialize;

use crate::output::OutputFormat;
use crate::thread::{self, Thread};
use crate::workspace;

#[derive(Args)]
pub struct ListArgs {
    /// Path to list threads from (git-root-relative, ./pwd-relative, or absolute)
    #[arg(default_value = "")]
    path: String,

    /// Include nested directories (recursive search)
    #[arg(short = 'r', long)]
    recursive: bool,

    /// Include resolved/terminal threads
    #[arg(long)]
    include_closed: bool,

    /// Search name/title/desc (substring)
    #[arg(short = 's', long)]
    search: Option<String>,

    /// Filter by status (comma-separated)
    #[arg(long)]
    status: Option<String>,

    /// Output format (auto-detects TTY for fancy vs plain)
    #[arg(short = 'f', long, value_enum, default_value = "fancy")]
    format: OutputFormat,

    /// Output as JSON (shorthand for --format=json)
    #[arg(long, conflicts_with = "format")]
    json: bool,
}

#[derive(Serialize)]
struct ThreadInfo {
    id: String,
    status: String,
    path: String,
    name: String,
    title: String,
    desc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    path_absolute: Option<String>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    is_pwd: bool,
}

pub fn run(args: ListArgs, git_root: &Path) -> Result<(), String> {
    // Determine output format (handle --json shorthand)
    let format = if args.json {
        OutputFormat::Json
    } else {
        args.format.resolve()
    };

    // Parse path filter if provided
    let path_filter = if args.path.is_empty() {
        None
    } else {
        Some(args.path.as_str())
    };

    // Resolve the scope to understand current context
    let scope = workspace::infer_scope(git_root, path_filter)?;
    let filter_path = scope.path.clone();

    let threads = workspace::find_all_threads(git_root)?;
    let mut results = Vec::new();

    // Get PWD relative path for comparison
    let pwd_rel = workspace::pwd_relative_to_git_root(git_root).unwrap_or_else(|_| ".".to_string());

    // Determine if we need absolute paths (for json/yaml)
    let include_absolute = matches!(format, OutputFormat::Json | OutputFormat::Yaml);

    for thread_path in threads {
        let t = match Thread::parse(&thread_path) {
            Ok(t) => t,
            Err(_) => continue,
        };

        let rel_path = workspace::parse_thread_path(git_root, &thread_path);
        let status = t.status().to_string();
        let base_status = thread::base_status(&status);
        let name = thread::extract_name_from_path(&thread_path);

        // Path filter: if not recursive, only show threads at the specified level
        if !args.recursive {
            // At specific path: only show threads in that exact directory
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

        // Status filter
        if let Some(ref status_filter) = args.status {
            let filter_statuses: Vec<&str> = status_filter.split(',').collect();
            if !filter_statuses.contains(&base_status.as_str()) {
                continue;
            }
        } else if !args.include_closed && thread::is_terminal(&status) {
            continue;
        }

        // Search filter
        if let Some(ref search) = args.search {
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

        let is_pwd = rel_path == pwd_rel;

        results.push(ThreadInfo {
            id: t.id().to_string(),
            status: base_status,
            path: rel_path,
            name,
            title,
            desc: t.frontmatter.desc.clone(),
            path_absolute: if include_absolute {
                Some(thread_path.to_string_lossy().to_string())
            } else {
                None
            },
            is_pwd,
        });
    }

    match format {
        OutputFormat::Fancy => output_fancy(
            &results,
            git_root,
            &filter_path,
            &pwd_rel,
            args.recursive,
            args.include_closed,
            args.status.as_deref(),
        ),
        OutputFormat::Plain => output_plain(
            &results,
            git_root,
            &filter_path,
            &pwd_rel,
            args.recursive,
            args.include_closed,
            args.status.as_deref(),
        ),
        OutputFormat::Json => output_json(&results, git_root, &pwd_rel),
        OutputFormat::Yaml => output_yaml(&results, git_root, &pwd_rel),
    }
}

fn output_fancy(
    results: &[ThreadInfo],
    git_root: &Path,
    filter_path: &str,
    pwd_rel: &str,
    recursive: bool,
    include_closed: bool,
    status_filter: Option<&str>,
) -> Result<(), String> {
    // Fancy header: repo-name (rel/path/to/pwd)
    let repo_name = git_root
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "repo".to_string());

    let path_desc = if filter_path == "." {
        String::new()
    } else {
        format!(" ({})", filter_path)
    };

    let pwd_marker = if filter_path == pwd_rel { " ← PWD" } else { "" };

    println!("{}{}{}", repo_name, path_desc, pwd_marker);
    println!();

    let status_desc = if let Some(s) = status_filter {
        format!("{} ", s)
    } else if include_closed {
        String::new()
    } else {
        "active ".to_string()
    };

    let recursive_suffix = if recursive { " (recursive)" } else { "" };

    println!(
        "Showing {} {}threads{}",
        results.len(),
        status_desc,
        recursive_suffix
    );
    println!();

    if results.is_empty() {
        if !recursive {
            println!("Hint: use -r to include nested directories");
        }
        return Ok(());
    }

    // Print table header
    println!(
        "{:<6} {:<10} {:<24} {}",
        "ID", "STATUS", "PATH", "NAME"
    );
    println!(
        "{:<6} {:<10} {:<24} {}",
        "--", "------", "----", "----"
    );

    for t in results {
        let path_display = truncate(&t.path, 22);
        let pwd_marker = if t.is_pwd { " ←" } else { "" };
        println!(
            "{:<6} {:<10} {:<24} {}{}",
            t.id, t.status, path_display, t.title, pwd_marker
        );
    }

    Ok(())
}

fn output_plain(
    results: &[ThreadInfo],
    git_root: &Path,
    filter_path: &str,
    pwd_rel: &str,
    recursive: bool,
    include_closed: bool,
    status_filter: Option<&str>,
) -> Result<(), String> {
    // Plain header: explicit context
    let pwd = std::env::current_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| "(unknown)".to_string());

    println!("PWD: {}", pwd);
    println!("Git root: {}", git_root.display());
    println!("PWD (git-relative): {}", pwd_rel);
    println!();

    let path_desc = if filter_path == "." {
        "repo root".to_string()
    } else {
        filter_path.to_string()
    };

    let status_desc = if let Some(s) = status_filter {
        s.to_string()
    } else if include_closed {
        String::new()
    } else {
        "active".to_string()
    };

    let recursive_suffix = if recursive { " (recursive)" } else { "" };

    let pwd_suffix = if filter_path == pwd_rel {
        " ← PWD"
    } else {
        ""
    };

    if !status_desc.is_empty() {
        println!(
            "Showing {} {} threads in {}{}{}",
            results.len(),
            status_desc,
            path_desc,
            recursive_suffix,
            pwd_suffix
        );
    } else {
        println!(
            "Showing {} threads in {} (all statuses){}{}",
            results.len(),
            path_desc,
            recursive_suffix,
            pwd_suffix
        );
    }
    println!();

    if results.is_empty() {
        if !recursive {
            println!("Hint: use -r to include nested directories");
        }
        return Ok(());
    }

    // Print table header
    println!(
        "{:<6} {:<10} {:<24} {}",
        "ID", "STATUS", "PATH", "NAME"
    );
    println!(
        "{:<6} {:<10} {:<24} {}",
        "--", "------", "----", "----"
    );

    for t in results {
        let path_display = truncate(&t.path, 22);
        let pwd_marker = if t.is_pwd { " ← PWD" } else { "" };
        println!(
            "{:<6} {:<10} {:<24} {}{}",
            t.id, t.status, path_display, t.title, pwd_marker
        );
    }

    Ok(())
}

fn output_json(
    results: &[ThreadInfo],
    git_root: &Path,
    pwd_rel: &str,
) -> Result<(), String> {
    let pwd = std::env::current_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| String::new());

    #[derive(Serialize)]
    struct JsonOutput<'a> {
        pwd: String,
        git_root: String,
        pwd_relative: &'a str,
        threads: &'a [ThreadInfo],
    }

    let output = JsonOutput {
        pwd,
        git_root: git_root.to_string_lossy().to_string(),
        pwd_relative: pwd_rel,
        threads: results,
    };

    let json =
        serde_json::to_string_pretty(&output).map_err(|e| format!("JSON serialization failed: {}", e))?;
    println!("{}", json);
    Ok(())
}

fn output_yaml(
    results: &[ThreadInfo],
    git_root: &Path,
    pwd_rel: &str,
) -> Result<(), String> {
    let pwd = std::env::current_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| String::new());

    #[derive(Serialize)]
    struct YamlOutput<'a> {
        pwd: String,
        git_root: String,
        pwd_relative: &'a str,
        threads: &'a [ThreadInfo],
    }

    let output = YamlOutput {
        pwd,
        git_root: git_root.to_string_lossy().to_string(),
        pwd_relative: pwd_rel,
        threads: results,
    };

    let yaml =
        serde_yaml::to_string(&output).map_err(|e| format!("YAML serialization failed: {}", e))?;
    print!("{}", yaml);
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

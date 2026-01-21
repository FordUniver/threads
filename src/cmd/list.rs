use std::path::Path;

use clap::Args;
use serde::Serialize;

use crate::output::OutputFormat;
use crate::thread::{self, Thread};
use crate::workspace::{self, FindOptions};

#[derive(Args)]
pub struct ListArgs {
    /// Path to list threads from (git-root-relative, ./pwd-relative, or absolute)
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

        // Path filter: if not searching, only show threads at the specified level
        if !search_dir.is_searching() && rel_path != filter_path {
            continue;
        }
        // Note: find_threads_with_options already handles direction/depth filtering

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
            &search_dir,
            args.include_closed,
            args.status.as_deref(),
        ),
        OutputFormat::Plain => output_plain(
            &results,
            git_root,
            &filter_path,
            &pwd_rel,
            &search_dir,
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
    search_dir: &SearchDirection,
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

    let pwd_marker = if filter_path == pwd_rel {
        " ← PWD"
    } else {
        ""
    };

    println!("{}{}{}", repo_name, path_desc, pwd_marker);
    println!();

    let status_desc = if let Some(s) = status_filter {
        format!("{} ", s)
    } else if include_closed {
        String::new()
    } else {
        "active ".to_string()
    };

    let search_suffix = search_dir.description();

    println!(
        "Showing {} {}threads{}",
        results.len(),
        status_desc,
        search_suffix
    );
    println!();

    if results.is_empty() {
        if !search_dir.is_searching() {
            println!("Hint: use -r to include nested directories, -u to search parents");
        }
        return Ok(());
    }

    // Print table header
    println!("{:<6} {:<10} {:<24} NAME", "ID", "STATUS", "PATH");
    println!("{:<6} {:<10} {:<24} ----", "--", "------", "----");

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
    search_dir: &SearchDirection,
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

    let search_suffix = search_dir.description();

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
            search_suffix,
            pwd_suffix
        );
    } else {
        println!(
            "Showing {} threads in {} (all statuses){}{}",
            results.len(),
            path_desc,
            search_suffix,
            pwd_suffix
        );
    }
    println!();

    if results.is_empty() {
        if !search_dir.is_searching() {
            println!("Hint: use -r to include nested directories, -u to search parents");
        }
        return Ok(());
    }

    // Print table header
    println!("{:<6} {:<10} {:<24} NAME", "ID", "STATUS", "PATH");
    println!("{:<6} {:<10} {:<24} ----", "--", "------", "----");

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

fn output_json(results: &[ThreadInfo], git_root: &Path, pwd_rel: &str) -> Result<(), String> {
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

    let json = serde_json::to_string_pretty(&output)
        .map_err(|e| format!("JSON serialization failed: {}", e))?;
    println!("{}", json);
    Ok(())
}

fn output_yaml(results: &[ThreadInfo], git_root: &Path, pwd_rel: &str) -> Result<(), String> {
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

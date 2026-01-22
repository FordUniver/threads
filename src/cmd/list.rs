use std::fs;
use std::path::Path;

use chrono::{DateTime, Local, Utc};
use clap::Args;
use colored::Colorize;
use serde::Serialize;
use tabled::settings::Style;
use tabled::{Table, Tabled};

use crate::cache::TimestampCache;
use crate::git;
use crate::output::{self, OutputFormat};
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

    /// Output format (auto-detects TTY for pretty vs plain)
    #[arg(short = 'f', long, value_enum, default_value = "pretty")]
    format: OutputFormat,

    /// Output as JSON (shorthand for --format=json)
    #[arg(long, conflicts_with = "format")]
    json: bool,
}

#[derive(Serialize, Clone)]
struct ThreadInfo {
    id: String,
    status: String,
    path: String,
    name: String,
    title: String,
    desc: String,
    #[serde(skip)]
    created_dt: Option<DateTime<Local>>,
    #[serde(skip)]
    updated_dt: Option<DateTime<Local>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    path_absolute: Option<String>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    is_pwd: bool,
}

impl ThreadInfo {
    /// Format date for plain mode (YYYY-MM-DD)
    fn created_plain(&self) -> String {
        self.created_dt
            .map(|dt| dt.format("%Y-%m-%d").to_string())
            .unwrap_or_else(|| "?".to_string())
    }

    fn updated_plain(&self) -> String {
        self.updated_dt
            .map(|dt| dt.format("%Y-%m-%d").to_string())
            .unwrap_or_else(|| "?".to_string())
    }

    /// Format date for pretty mode (short: "3h", "2d", "1w")
    fn created_short(&self) -> String {
        self.created_dt
            .map(output::format_relative_short)
            .unwrap_or_else(|| "?".to_string())
    }

    fn updated_short(&self) -> String {
        self.updated_dt
            .map(output::format_relative_short)
            .unwrap_or_else(|| "?".to_string())
    }

    /// Format date for JSON/YAML (ISO 8601)
    fn created_iso(&self) -> String {
        self.created_dt
            .map(|dt| dt.with_timezone(&Utc).to_rfc3339())
            .unwrap_or_default()
    }

    fn updated_iso(&self) -> String {
        self.updated_dt
            .map(|dt| dt.with_timezone(&Utc).to_rfc3339())
            .unwrap_or_default()
    }

    /// Get timestamp for sorting (most recent first)
    fn updated_ts(&self) -> i64 {
        self.updated_dt
            .map(|dt| dt.timestamp())
            .unwrap_or(0)
    }
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
    // Open repository for git-based timestamps
    let repo = workspace::open()?;

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

    // Load and update timestamp cache
    let mut cache = TimestampCache::load(git_root);
    cache.update(&repo, &threads, git_root);

    // Save cache (ignore errors - cache is optional)
    let _ = cache.save(git_root);

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

        // Get timestamps from cache, with fallback for uncommitted files
        let thread_rel_path = thread_path.strip_prefix(git_root).unwrap_or(&thread_path);
        let thread_rel_str = thread_rel_path.to_string_lossy();
        let (created_dt, updated_dt) = get_timestamps(&repo, &cache, &thread_path, &thread_rel_str);

        results.push(ThreadInfo {
            id: t.id().to_string(),
            status: base_status,
            path: rel_path,
            name,
            title,
            desc: t.frontmatter.desc.clone(),
            created_dt,
            updated_dt,
            path_absolute: if include_absolute {
                Some(thread_path.to_string_lossy().to_string())
            } else {
                None
            },
            is_pwd,
        });
    }

    // Sort by updated timestamp, most recent first
    results.sort_by_key(|t| std::cmp::Reverse(t.updated_ts()));

    match format {
        OutputFormat::Pretty => output_pretty(
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

/// Row data for tabled output
#[derive(Tabled)]
struct TableRow {
    #[tabled(rename = "ID")]
    id: String,
    #[tabled(rename = "STATUS")]
    status: String,
    #[tabled(rename = "NEW")]
    created: String,
    #[tabled(rename = "MOD")]
    modified: String,
    #[tabled(rename = "PATH")]
    path: String,
    #[tabled(rename = "TITLE")]
    title: String,
}

/// Build filter description for summary line
fn build_filter_desc(
    include_closed: bool,
    status_filter: Option<&str>,
    search: Option<&str>,
    search_dir: &SearchDirection,
) -> String {
    let mut parts = Vec::new();

    // Status filter
    if let Some(s) = status_filter {
        parts.push(format!("status={}", s));
    } else if !include_closed {
        parts.push("open".to_string()); // "open" = non-terminal, not "active"
    } else {
        parts.push("all statuses".to_string());
    }

    // Search filter
    if let Some(s) = search {
        parts.push(format!("search=\"{}\"", s));
    }

    // Direction
    let dir_desc = search_dir.description();
    if !dir_desc.is_empty() {
        parts.push(dir_desc.trim().to_string());
    }

    parts.join(", ")
}

fn output_pretty(
    results: &[ThreadInfo],
    git_root: &Path,
    filter_path: &str,
    pwd_rel: &str,
    search_dir: &SearchDirection,
    include_closed: bool,
    status_filter: Option<&str>,
) -> Result<(), String> {
    // Header: repo-name (path) with PWD marker
    let repo_name = git_root
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "repo".to_string());

    let path_desc = if filter_path == "." {
        String::new()
    } else {
        format!(" ({})", filter_path)
    };

    // PWD marker uses bold (not cyan - cyan is for status)
    let pwd_marker = if filter_path == pwd_rel {
        " ← PWD".bold().to_string()
    } else {
        String::new()
    };

    println!("{}{}{}", repo_name.bold(), path_desc.dimmed(), pwd_marker);

    // Filter disclosure - always show what filters are active
    let filter_desc = build_filter_desc(include_closed, status_filter, None, search_dir);
    println!(
        "{} threads ({})",
        results.len().to_string().bold(),
        filter_desc.dimmed()
    );
    println!();

    if results.is_empty() {
        if !search_dir.is_searching() {
            println!(
                "{}",
                "Hint: use -r to include nested directories, -u to search parents".dimmed()
            );
        }
        return Ok(());
    }

    // Build table rows
    let term_width = output::terminal_width();
    let title_max = term_width.saturating_sub(58).max(20); // Leave room for other columns (added NEW column)

    let rows: Vec<TableRow> = results
        .iter()
        .map(|t| {
            let short_path = output::shortest_path(&t.path, pwd_rel);
            let path_display = output::truncate_front(&short_path, 20);
            // PWD paths are bold, others dimmed
            let path_styled = output::style_path(&path_display, t.is_pwd);

            TableRow {
                id: output::style_id(&t.id).to_string(),
                status: output::style_status(&t.status).to_string(),
                created: t.created_short(),
                modified: t.updated_short(),
                path: path_styled,
                title: output::truncate_back(&t.title, title_max),
            }
        })
        .collect();

    let mut table = Table::new(rows);
    table.with(Style::rounded());

    println!("{}", table);

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

    let pwd_suffix = if filter_path == pwd_rel {
        " ← PWD"
    } else {
        ""
    };

    // Full filter disclosure
    let filter_desc = build_filter_desc(include_closed, status_filter, None, search_dir);
    println!(
        "Showing {} threads in {}{} ({})",
        results.len(),
        path_desc,
        pwd_suffix,
        filter_desc
    );
    println!();

    if results.is_empty() {
        if !search_dir.is_searching() {
            println!("Hint: use -r to include nested directories, -u to search parents");
        }
        return Ok(());
    }

    // Pipe-delimited format, no truncation, full paths
    println!("ID | STATUS | CREATED | UPDATED | PATH | TITLE");

    for t in results {
        println!(
            "{} | {} | {} | {} | {} | {}",
            t.id, t.status, t.created_plain(), t.updated_plain(), t.path, t.title
        );
    }

    Ok(())
}

/// Serializable thread info with ISO 8601 dates for JSON/YAML
#[derive(Serialize)]
struct ThreadInfoJson {
    id: String,
    status: String,
    path: String,
    name: String,
    title: String,
    desc: String,
    created: String,
    updated: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    path_absolute: Option<String>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    is_pwd: bool,
}

impl From<&ThreadInfo> for ThreadInfoJson {
    fn from(t: &ThreadInfo) -> Self {
        Self {
            id: t.id.clone(),
            status: t.status.clone(),
            path: t.path.clone(),
            name: t.name.clone(),
            title: t.title.clone(),
            desc: t.desc.clone(),
            created: t.created_iso(),
            updated: t.updated_iso(),
            path_absolute: t.path_absolute.clone(),
            is_pwd: t.is_pwd,
        }
    }
}

fn output_json(results: &[ThreadInfo], git_root: &Path, pwd_rel: &str) -> Result<(), String> {
    let pwd = std::env::current_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| String::new());

    let threads: Vec<ThreadInfoJson> = results.iter().map(ThreadInfoJson::from).collect();

    #[derive(Serialize)]
    struct JsonOutput {
        pwd: String,
        git_root: String,
        pwd_relative: String,
        threads: Vec<ThreadInfoJson>,
    }

    let output = JsonOutput {
        pwd,
        git_root: git_root.to_string_lossy().to_string(),
        pwd_relative: pwd_rel.to_string(),
        threads,
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

    let threads: Vec<ThreadInfoJson> = results.iter().map(ThreadInfoJson::from).collect();

    #[derive(Serialize)]
    struct YamlOutput {
        pwd: String,
        git_root: String,
        pwd_relative: String,
        threads: Vec<ThreadInfoJson>,
    }

    let output = YamlOutput {
        pwd,
        git_root: git_root.to_string_lossy().to_string(),
        pwd_relative: pwd_rel.to_string(),
        threads,
    };

    let yaml =
        serde_yaml::to_string(&output).map_err(|e| format!("YAML serialization failed: {}", e))?;
    print!("{}", yaml);
    Ok(())
}

/// Get timestamps from cache, handling uncommitted modifications.
fn get_timestamps(
    repo: &git2::Repository,
    cache: &TimestampCache,
    abs_path: &Path,
    rel_path: &str,
) -> (Option<DateTime<Local>>, Option<DateTime<Local>>) {
    // Check if file has uncommitted changes
    let has_uncommitted_changes = git::has_changes(repo, Path::new(rel_path));

    if let Some(cached) = cache.get(rel_path) {
        // File is in cache (has been committed at some point)
        let created_dt = DateTime::from_timestamp(cached.created, 0)
            .map(|dt| dt.with_timezone(&Local));

        let modified_dt = if has_uncommitted_changes {
            // File has uncommitted changes - use filesystem mtime
            fs::metadata(abs_path)
                .ok()
                .and_then(|m| m.modified().ok())
                .map(|t| t.into())
        } else {
            // File is clean - use git commit date
            DateTime::from_timestamp(cached.modified, 0)
                .map(|dt| dt.with_timezone(&Local))
        };

        (created_dt, modified_dt)
    } else {
        // File not in cache (never committed) - use filesystem times
        let metadata = fs::metadata(abs_path).ok();

        let created_dt: Option<DateTime<Local>> = metadata
            .as_ref()
            .and_then(|m| m.created().ok())
            .map(|t| t.into());

        let modified_dt: Option<DateTime<Local>> = metadata
            .and_then(|m| m.modified().ok())
            .map(|t| t.into());

        (created_dt.or(modified_dt), modified_dt)
    }
}

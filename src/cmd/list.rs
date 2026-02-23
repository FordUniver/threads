use std::fs;
use std::path::Path;

use chrono::{DateTime, Local, NaiveDate, Utc};
use clap::Args;
use colored::Colorize;
use serde::Serialize;
use tabled::settings::Style;
use tabled::{Table, Tabled};

use crate::args::{DirectionArgs, FilterArgs, FormatArgs};
use crate::cache::TimestampCache;
use crate::config::{Config, is_quiet, root_name};
use crate::git;
use crate::output::{self, OutputFormat};
use crate::thread::{self, Thread};
use crate::workspace;

#[derive(Args)]
pub struct ListArgs {
    /// Path to list threads from (git-root-relative, ./pwd-relative, or absolute)
    #[arg(default_value = "")]
    path: String,

    #[command(flatten)]
    direction: DirectionArgs,

    #[command(flatten)]
    filter: FilterArgs,

    /// Search name/title/desc (substring)
    #[arg(short = 's', long)]
    search: Option<String>,

    /// Filter by status (comma-separated)
    #[arg(long)]
    status: Option<String>,

    #[command(flatten)]
    format: FormatArgs,
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
    /// Git file status (M/S/A/?/D or empty for clean)
    #[serde(skip_serializing_if = "Option::is_none")]
    git_status: Option<String>,
    /// Nearest upcoming deadline date (YYYY-MM-DD), or None
    #[serde(skip_serializing_if = "Option::is_none")]
    due: Option<String>,
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
        self.updated_dt.map(|dt| dt.timestamp()).unwrap_or(0)
    }
}

pub fn run(args: ListArgs, git_root: &Path, config: &Config) -> Result<(), String> {
    // Open repository for git-based timestamps
    let repo = workspace::open()?;

    let format = args.format.resolve();

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

    // Convert direction args to find options
    let options = args.direction.to_find_options();

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
        if !args.direction.is_searching() && rel_path != filter_path {
            continue;
        }
        // Note: find_threads_with_options already handles direction/depth filtering

        // Status filter
        let include_closed = args.filter.include_closed();
        if let Some(ref status_filter) = args.status {
            let filter_statuses: Vec<&str> = status_filter.split(',').collect();
            if !filter_statuses.contains(&base_status.as_str()) {
                continue;
            }
        } else if !include_closed && thread::is_closed(&status) {
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

        // Get git file status
        let file_status = git::file_status(&repo, thread_rel_path);
        let git_status_str = format_git_status(&file_status);

        // Nearest upcoming deadline
        let today_str = Local::now().date_naive().format("%Y-%m-%d").to_string();
        let due = {
            let deadlines = t.get_deadlines();
            deadlines
                .into_iter()
                .map(|d| d.date)
                .filter(|date| date.as_str() >= today_str.as_str())
                .min()
        };

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
            git_status: if git_status_str.is_empty() {
                None
            } else {
                Some(git_status_str.to_string())
            },
            due,
        });
    }

    // Sort by updated timestamp, most recent first
    results.sort_by_key(|t| std::cmp::Reverse(t.updated_ts()));

    let include_closed = args.filter.include_closed();

    match format {
        OutputFormat::Pretty => output_pretty(
            &results,
            git_root,
            &filter_path,
            &pwd_rel,
            &args.direction,
            include_closed,
            args.status.as_deref(),
            config,
        ),
        OutputFormat::Plain => output_plain(
            &results,
            git_root,
            &filter_path,
            &pwd_rel,
            &args.direction,
            include_closed,
            args.status.as_deref(),
            config,
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
    #[tabled(rename = "GIT")]
    git_status: String,
    #[tabled(rename = "DUE")]
    due: String,
    #[tabled(rename = "TITLE")]
    title: String,
}

/// Format git file status as short code for list display
fn format_git_status(status: &git::FileStatus) -> &'static str {
    match status {
        git::FileStatus::Clean => "",
        git::FileStatus::Modified => "M",
        git::FileStatus::Staged => "S",
        git::FileStatus::StagedAndModified => "SM",
        git::FileStatus::StagedNew => "A",
        git::FileStatus::Untracked => "?",
        git::FileStatus::Deleted => "D",
        git::FileStatus::Changed => "M",
        git::FileStatus::Unknown => "",
    }
}

/// Style the DUE date for table display.
fn style_due_date(due: Option<&str>, today: NaiveDate) -> String {
    let date_str = match due {
        Some(d) => d,
        None => return String::new(),
    };
    match NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
        Ok(d) => {
            let days = (d - today).num_days();
            if days == 0 {
                date_str.red().bold().to_string()
            } else if days <= 7 {
                date_str.yellow().to_string()
            } else {
                date_str.to_string()
            }
        }
        Err(_) => date_str.to_string(),
    }
}

/// Build filter description for summary line
fn build_filter_desc(
    include_closed: bool,
    status_filter: Option<&str>,
    search: Option<&str>,
    direction: &DirectionArgs,
) -> String {
    let mut parts = Vec::new();

    // Status filter
    if let Some(s) = status_filter {
        parts.push(format!("status={}", s));
    } else if !include_closed {
        parts.push("open".to_string()); // "open" = non-closed
    } else {
        parts.push("all statuses".to_string());
    }

    // Search filter
    if let Some(s) = search {
        parts.push(format!("search=\"{}\"", s));
    }

    // Direction
    let dir_desc = direction.description();
    if !dir_desc.is_empty() {
        parts.push(dir_desc);
    }

    parts.join(", ")
}

#[allow(clippy::too_many_arguments)]
fn output_pretty(
    results: &[ThreadInfo],
    git_root: &Path,
    filter_path: &str,
    pwd_rel: &str,
    direction: &DirectionArgs,
    include_closed: bool,
    status_filter: Option<&str>,
    config: &Config,
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
    let filter_desc = build_filter_desc(include_closed, status_filter, None, direction);
    println!(
        "{} threads ({})",
        results.len().to_string().bold(),
        filter_desc.dimmed()
    );
    println!();

    if results.is_empty() {
        if !direction.is_searching() && !is_quiet(config) {
            println!(
                "{}",
                "Hint: use --down to include nested directories, --up to search parents".dimmed()
            );
        }
        return Ok(());
    }

    // Build table rows
    let term_width = output::terminal_width();
    let title_max = term_width.saturating_sub(70).max(20); // Leave room for other columns (added NEW, DUE columns)
    let today = Local::now().date_naive();

    let rows: Vec<TableRow> = results
        .iter()
        .map(|t| {
            let short_path = output::shortest_path(&t.path, pwd_rel);
            let path_display = output::truncate_front(&short_path, 20);
            // PWD paths are bold, others dimmed
            let path_styled = output::style_path(&path_display, t.is_pwd);

            let due_styled = style_due_date(t.due.as_deref(), today);

            TableRow {
                id: output::style_id(&t.id).to_string(),
                status: output::style_status(&t.status).to_string(),
                created: t.created_short(),
                modified: t.updated_short(),
                path: path_styled,
                git_status: t.git_status.clone().unwrap_or_default(),
                due: due_styled,
                title: output::truncate_back(&t.title, title_max),
            }
        })
        .collect();

    let mut table = Table::new(rows);
    table.with(Style::rounded());

    println!("{}", table);

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn output_plain(
    results: &[ThreadInfo],
    git_root: &Path,
    filter_path: &str,
    pwd_rel: &str,
    direction: &DirectionArgs,
    include_closed: bool,
    status_filter: Option<&str>,
    config: &Config,
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
        root_name(config).to_string()
    } else {
        filter_path.to_string()
    };

    let pwd_suffix = if filter_path == pwd_rel {
        " ← PWD"
    } else {
        ""
    };

    // Full filter disclosure
    let filter_desc = build_filter_desc(include_closed, status_filter, None, direction);
    println!(
        "Showing {} threads in {}{} ({})",
        results.len(),
        path_desc,
        pwd_suffix,
        filter_desc
    );
    println!();

    if results.is_empty() {
        if !direction.is_searching() && !is_quiet(config) {
            println!("Hint: use --down to include nested directories, --up to search parents");
        }
        return Ok(());
    }

    // Pipe-delimited format, no truncation, full paths
    println!("ID | STATUS | CREATED | UPDATED | PATH | GIT | DUE | TITLE");

    for t in results {
        println!(
            "{} | {} | {} | {} | {} | {} | {} | {}",
            t.id,
            t.status,
            t.created_plain(),
            t.updated_plain(),
            t.path,
            t.git_status.as_deref().unwrap_or(""),
            t.due.as_deref().unwrap_or(""),
            t.title
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
    #[serde(skip_serializing_if = "Option::is_none")]
    git_status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    due: Option<String>,
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
            git_status: t.git_status.clone(),
            due: t.due.clone(),
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
        let created_dt =
            DateTime::from_timestamp(cached.created, 0).map(|dt| dt.with_timezone(&Local));

        let modified_dt = if has_uncommitted_changes {
            // File has uncommitted changes - use filesystem mtime
            fs::metadata(abs_path)
                .ok()
                .and_then(|m| m.modified().ok())
                .map(|t| t.into())
        } else {
            // File is clean - use git commit date
            DateTime::from_timestamp(cached.modified, 0).map(|dt| dt.with_timezone(&Local))
        };

        (created_dt, modified_dt)
    } else {
        // File not in cache (never committed) - use filesystem times
        let metadata = fs::metadata(abs_path).ok();

        let created_dt: Option<DateTime<Local>> = metadata
            .as_ref()
            .and_then(|m| m.created().ok())
            .map(|t| t.into());

        let modified_dt: Option<DateTime<Local>> =
            metadata.and_then(|m| m.modified().ok()).map(|t| t.into());

        (created_dt.or(modified_dt), modified_dt)
    }
}

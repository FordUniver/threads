use std::path::Path;

use clap::Args;
use colored::Colorize;
use serde::Serialize;
use tabled::settings::Style;
use tabled::{Table, Tabled};

use crate::args::{DirectionArgs, FilterArgs, FormatArgs};
use crate::config::{Config, is_quiet, root_name};
use crate::fuzzy;
use crate::output::{self, OutputFormat};
use crate::thread::{self, Thread};
use crate::workspace;

#[derive(Args)]
pub struct SearchArgs {
    /// [path] query - Path is optional, query is required
    /// Path resolution:
    ///   (none)  → PWD (current directory)
    ///   .       → PWD (explicit)
    ///   ./X/Y   → PWD-relative
    ///   /X/Y    → Absolute
    ///   X/Y     → Git-root-relative
    #[arg(required = true, num_args = 1..=2)]
    args: Vec<String>,

    #[command(flatten)]
    direction: DirectionArgs,

    #[command(flatten)]
    filter: FilterArgs,

    /// Filter by status (comma-separated)
    #[arg(long)]
    status: Option<String>,

    #[command(flatten)]
    format: FormatArgs,
}

#[derive(Clone, Copy, Debug)]
enum MatchKind {
    Title,
    Desc,
    Path,
    Body,
}

impl MatchKind {
    fn as_str(self) -> &'static str {
        match self {
            MatchKind::Title => "title",
            MatchKind::Desc => "desc",
            MatchKind::Path => "path",
            MatchKind::Body => "body",
        }
    }
}

#[derive(Clone)]
struct SearchMatch {
    score: i64,
    kind: MatchKind,
    snippet: String,
}

#[derive(Clone, Serialize)]
struct SearchResult {
    score: i64,
    id: String,
    status: String,
    path: String,
    title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    path_absolute: Option<String>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    is_pwd: bool,
    matched_in: String,
    snippet: String,
}

pub fn run(args: SearchArgs, git_root: &Path, config: &Config) -> Result<(), String> {
    let format = args.format.resolve();

    // Parse positional args: either [query] or [path, query]
    let (path_arg, query) = if args.args.len() == 2 {
        (Some(args.args[0].as_str()), args.args[1].clone())
    } else if args.args.len() == 1 {
        (None, args.args[0].clone())
    } else {
        return Err("query is required".to_string());
    };

    let query = query.trim().to_string();
    if query.is_empty() {
        return Err("query is required".to_string());
    }

    let tokens: Vec<String> = query
        .split_whitespace()
        .filter(|t| !t.is_empty())
        .map(|t| t.to_string())
        .collect();

    if tokens.is_empty() {
        return Err("query is required".to_string());
    }

    // Resolve scope
    let scope = workspace::infer_scope(git_root, path_arg)?;
    let filter_path = scope.path.clone();
    let start_path = scope.threads_dir.parent().unwrap_or(git_root);

    // Convert direction args to find options
    let options = args.direction.to_find_options();

    // Find threads using options
    let threads = workspace::find_threads_with_options(start_path, git_root, &options)?;

    // PWD relative path for display
    let pwd_rel = workspace::pwd_relative_to_git_root(git_root).unwrap_or_else(|_| ".".to_string());

    // Determine if we need absolute paths (for json/yaml)
    let include_absolute = matches!(format, OutputFormat::Json | OutputFormat::Yaml);

    let mut results = Vec::new();
    let mut skipped_closed_metadata_matches = 0usize;

    for thread_path in threads {
        let rel_path = workspace::parse_thread_path(git_root, &thread_path);

        // Path filter: if not searching, only include threads at the specified level
        if !args.direction.is_searching() && rel_path != filter_path {
            continue;
        }

        let t = match Thread::parse(&thread_path) {
            Ok(t) => t,
            Err(_) => continue,
        };

        let status = t.status().to_string();
        let base_status = thread::base_status(&status);

        // Resolve title early (used by filters and hints).
        let title = if !t.name().is_empty() {
            t.name().to_string()
        } else {
            let name = thread::extract_name_from_path(&thread_path);
            name.replace('-', " ")
        };

        // Status filter
        let include_closed = args.filter.include_closed();
        if let Some(ref status_filter) = args.status {
            let filter_statuses: Vec<&str> = status_filter.split(',').collect();
            if !filter_statuses.contains(&base_status.as_str()) {
                continue;
            }
        } else if !include_closed && thread::is_closed(&status) {
            if matches_metadata(&tokens, &title, &t.frontmatter.desc, &rel_path) {
                skipped_closed_metadata_matches += 1;
            }
            continue;
        }

        let is_pwd = rel_path == pwd_rel;

        let Some(best) = best_match(&tokens, &title, &t.frontmatter.desc, &rel_path, t.body())
        else {
            continue;
        };

        results.push(SearchResult {
            score: best.score,
            id: t.id().to_string(),
            status: base_status,
            path: rel_path,
            title,
            path_absolute: if include_absolute {
                Some(thread_path.to_string_lossy().to_string())
            } else {
                None
            },
            is_pwd,
            matched_in: best.kind.as_str().to_string(),
            snippet: best.snippet,
        });
    }

    // Sort by score descending, then by title for stable order
    results.sort_by(|a, b| {
        b.score
            .cmp(&a.score)
            .then_with(|| a.title.cmp(&b.title))
            .then_with(|| a.id.cmp(&b.id))
    });

    let include_closed = args.filter.include_closed();

    match format {
        OutputFormat::Pretty => output_pretty(
            &results,
            git_root,
            &filter_path,
            &pwd_rel,
            &query,
            &args.direction,
            include_closed,
            args.status.as_deref(),
            skipped_closed_metadata_matches,
            config,
        ),
        OutputFormat::Plain => output_plain(
            &results,
            git_root,
            &filter_path,
            &pwd_rel,
            &query,
            &args.direction,
            include_closed,
            args.status.as_deref(),
            skipped_closed_metadata_matches,
            config,
        ),
        OutputFormat::Json => output_json(&results, git_root, &pwd_rel, &query),
        OutputFormat::Yaml => output_yaml(&results, git_root, &pwd_rel, &query),
    }
}

fn matches_metadata(tokens: &[String], title: &str, desc: &str, rel_path: &str) -> bool {
    for tok in tokens {
        let found = fuzzy::score(tok, title).is_some()
            || fuzzy::score(tok, desc).is_some()
            || fuzzy::score(tok, rel_path).is_some();
        if !found {
            return false;
        }
    }
    true
}

fn best_match(
    tokens: &[String],
    title: &str,
    desc: &str,
    rel_path: &str,
    body: &str,
) -> Option<SearchMatch> {
    // Build candidate lines (trimmed, non-empty).
    let mut lines: Vec<(MatchKind, &str)> = Vec::new();
    let title = title.trim();
    if !title.is_empty() {
        lines.push((MatchKind::Title, title));
    }
    let desc = desc.trim();
    if !desc.is_empty() {
        lines.push((MatchKind::Desc, desc));
    }
    let rel_path = rel_path.trim();
    if !rel_path.is_empty() {
        lines.push((MatchKind::Path, rel_path));
    }
    for line in body.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        lines.push((MatchKind::Body, line));
    }

    if lines.is_empty() {
        return None;
    }

    // Thread score: sum of best per-token scores across all lines.
    let mut total = 0i64;
    for tok in tokens {
        let mut best_tok: Option<i64> = None;
        for (_, line) in &lines {
            if let Some(s) = fuzzy::score(tok, line) {
                best_tok = Some(best_tok.map_or(s, |cur| cur.max(s)));
            }
        }
        total += best_tok?;
    }

    // Snippet: pick the single line that matches the most tokens, then the highest summed score.
    let mut best_line: Option<(usize, i64, MatchKind, &str)> = None;
    for (kind, line) in &lines {
        let mut matched = 0usize;
        let mut sum = 0i64;
        for tok in tokens {
            if let Some(s) = fuzzy::score(tok, line) {
                matched += 1;
                sum += s;
            }
        }
        if matched == 0 {
            continue;
        }

        best_line = match best_line {
            None => Some((matched, sum, *kind, *line)),
            Some((best_matched, best_sum, best_kind, best_line)) => {
                if matched > best_matched
                    || (matched == best_matched && sum > best_sum)
                    || (matched == best_matched
                        && sum == best_sum
                        && kind.as_str() < best_kind.as_str())
                    || (matched == best_matched
                        && sum == best_sum
                        && kind.as_str() == best_kind.as_str()
                        && line.len() < best_line.len())
                {
                    Some((matched, sum, *kind, *line))
                } else {
                    Some((best_matched, best_sum, best_kind, best_line))
                }
            }
        };
    }

    let (matched, _sum, kind, line) = best_line?;
    if matched == tokens.len() {
        // Small co-location bonus if all tokens hit on a single line.
        total += 25;
    }

    Some(SearchMatch {
        score: total,
        kind,
        snippet: format!("{}: {}", kind.as_str(), line),
    })
}

/// Row data for pretty output table.
#[derive(Tabled)]
struct TableRow {
    #[tabled(rename = "SCORE")]
    score: String,
    #[tabled(rename = "ID")]
    id: String,
    #[tabled(rename = "STATUS")]
    status: String,
    #[tabled(rename = "PATH")]
    path: String,
    #[tabled(rename = "TITLE")]
    title: String,
}

fn build_filter_desc(
    include_closed: bool,
    status_filter: Option<&str>,
    query: &str,
    direction: &DirectionArgs,
) -> String {
    let mut parts = Vec::new();

    if let Some(s) = status_filter {
        parts.push(format!("status={}", s));
    } else if !include_closed {
        parts.push("open".to_string());
    } else {
        parts.push("all statuses".to_string());
    }

    parts.push(format!("query=\"{}\"", query));

    let dir_desc = direction.description();
    if !dir_desc.is_empty() {
        parts.push(dir_desc);
    }

    parts.join(", ")
}

#[allow(clippy::too_many_arguments)]
fn output_pretty(
    results: &[SearchResult],
    git_root: &Path,
    filter_path: &str,
    pwd_rel: &str,
    query: &str,
    direction: &DirectionArgs,
    include_closed: bool,
    status_filter: Option<&str>,
    skipped_closed_metadata_matches: usize,
    config: &Config,
) -> Result<(), String> {
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
        " ← PWD".bold().to_string()
    } else {
        String::new()
    };

    println!("{}{}{}", repo_name.bold(), path_desc.dimmed(), pwd_marker);

    let filter_desc = build_filter_desc(include_closed, status_filter, query, direction);
    println!(
        "{} matches ({})",
        results.len().to_string().bold(),
        filter_desc.dimmed()
    );
    if !include_closed && status_filter.is_none() && skipped_closed_metadata_matches > 0 {
        println!(
            "{}",
            format!(
                "Note: {} additional matches in closed thread metadata (use --include-closed).",
                skipped_closed_metadata_matches
            )
            .dimmed()
        );
    }
    println!();

    if results.is_empty() {
        println!("{}", "No matches.".dimmed());
        if !direction.is_searching() && !is_quiet(config) {
            println!(
                "{}",
                "Hint: use --down to include nested directories, --up to search parents".dimmed()
            );
        }
        return Ok(());
    }

    let term_width = output::terminal_width();
    let title_max = 28usize;
    let _unused = term_width; // keep width calc pattern consistent with list/stats

    let rows: Vec<TableRow> = results
        .iter()
        .map(|r| {
            let short_path = output::shortest_path(&r.path, pwd_rel);
            let path_display = output::truncate_front(&short_path, 20);
            let path_styled = output::style_path(&path_display, r.is_pwd);

            TableRow {
                score: r.score.to_string().dimmed().to_string(),
                id: output::style_id(&r.id).to_string(),
                status: output::style_status(&r.status).to_string(),
                path: path_styled,
                title: output::truncate_back(&r.title, title_max),
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
    results: &[SearchResult],
    git_root: &Path,
    filter_path: &str,
    pwd_rel: &str,
    query: &str,
    direction: &DirectionArgs,
    include_closed: bool,
    status_filter: Option<&str>,
    skipped_closed_metadata_matches: usize,
    config: &Config,
) -> Result<(), String> {
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

    let filter_desc = build_filter_desc(include_closed, status_filter, query, direction);
    println!(
        "Showing {} matches in {}{} ({})",
        results.len(),
        path_desc,
        pwd_suffix,
        filter_desc
    );
    if !include_closed && status_filter.is_none() && skipped_closed_metadata_matches > 0 {
        println!(
            "Note: {} additional matches in closed thread metadata (use --include-closed).",
            skipped_closed_metadata_matches
        );
    }
    println!();

    if results.is_empty() {
        if !direction.is_searching() && !is_quiet(config) {
            println!("Hint: use --down to include nested directories, --up to search parents");
        }
        return Ok(());
    }

    println!("SCORE | ID | STATUS | PATH | TITLE");
    for r in results {
        println!("{} | {} | {} | {} | {}", r.score, r.id, r.status, r.path, r.title);
    }

    Ok(())
}

#[derive(Serialize)]
struct SearchResultJson<'a> {
    score: i64,
    id: &'a str,
    status: &'a str,
    path: &'a str,
    title: &'a str,
    matched_in: &'a str,
    snippet: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    path_absolute: Option<&'a str>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    is_pwd: bool,
}

fn output_json(
    results: &[SearchResult],
    git_root: &Path,
    pwd_rel: &str,
    query: &str,
) -> Result<(), String> {
    let pwd = std::env::current_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| String::new());

    let matches: Vec<SearchResultJson<'_>> = results
        .iter()
        .map(|r| SearchResultJson {
            score: r.score,
            id: &r.id,
            status: &r.status,
            path: &r.path,
            title: &r.title,
            matched_in: &r.matched_in,
            snippet: &r.snippet,
            path_absolute: r.path_absolute.as_deref(),
            is_pwd: r.is_pwd,
        })
        .collect();

    #[derive(Serialize)]
    struct JsonOutput<'a> {
        pwd: String,
        git_root: String,
        pwd_relative: String,
        query: &'a str,
        matches: Vec<SearchResultJson<'a>>,
    }

    let output = JsonOutput {
        pwd,
        git_root: git_root.to_string_lossy().to_string(),
        pwd_relative: pwd_rel.to_string(),
        query,
        matches,
    };

    let json = serde_json::to_string_pretty(&output)
        .map_err(|e| format!("JSON serialization failed: {}", e))?;
    println!("{}", json);
    Ok(())
}

fn output_yaml(
    results: &[SearchResult],
    git_root: &Path,
    pwd_rel: &str,
    query: &str,
) -> Result<(), String> {
    let pwd = std::env::current_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| String::new());

    let matches: Vec<SearchResultJson<'_>> = results
        .iter()
        .map(|r| SearchResultJson {
            score: r.score,
            id: &r.id,
            status: &r.status,
            path: &r.path,
            title: &r.title,
            matched_in: &r.matched_in,
            snippet: &r.snippet,
            path_absolute: r.path_absolute.as_deref(),
            is_pwd: r.is_pwd,
        })
        .collect();

    #[derive(Serialize)]
    struct YamlOutput<'a> {
        pwd: String,
        git_root: String,
        pwd_relative: String,
        query: &'a str,
        matches: Vec<SearchResultJson<'a>>,
    }

    let output = YamlOutput {
        pwd,
        git_root: git_root.to_string_lossy().to_string(),
        pwd_relative: pwd_rel.to_string(),
        query,
        matches,
    };

    let yaml =
        serde_yaml::to_string(&output).map_err(|e| format!("YAML serialization failed: {}", e))?;
    print!("{}", yaml);
    Ok(())
}

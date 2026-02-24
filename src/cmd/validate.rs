use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process;
use std::sync::LazyLock;

use clap::{Args, Subcommand};
use colored::Colorize;
use regex::Regex;
use serde::Serialize;

use crate::args::{DirectionArgs, FilterArgs, FormatArgs};
use crate::cmd::migrate::migrate_file_for_validate;
use crate::config::Config;
use crate::output::OutputFormat;
use crate::thread::{self, Frontmatter, extract_id_from_path};
use crate::workspace;

// ============================================================================
// Regexes for validation
// ============================================================================

/// Matches a valid 6-character hex ID
static VALID_ID_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[0-9a-f]{6}$").unwrap());

/// Matches section headers (## Name)
static SECTION_HEADER_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?m)^## (.+)$").unwrap());

/// Legacy section names that must not appear in fully migrated threads.
/// Finding any of these triggers W010.
static LEGACY_SECTIONS: &[&str] = &["Body", "Notes", "Todo", "Log"];

/// Matches log date headers (### YYYY-MM-DD) - legacy format to be removed
static LOG_DATE_HEADER_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^### (\d{4}-\d{2}-\d{2})$").unwrap());

/// Matches current log format: - [YYYY-MM-DD HH:MM:SS] text
static BRACKET_LOG_FORMAT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^- \[(\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2})\]").unwrap());

/// Matches legacy bold log format: - **YYYY-MM-DD HH:MM:SS** text
static BOLD_LOG_FORMAT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^- \*\*(\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2})\*\*").unwrap());

/// Matches legacy time-only format: - **HH:MM** text (under date header)
static TIME_ONLY_FORMAT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^- \*\*(\d{2}:\d{2})\*\*").unwrap());

/// Matches todo checkbox line
static TODO_CHECKBOX_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^- \[([ xX])\]").unwrap());

/// Matches malformed checkbox (common mistakes)
static MALFORMED_CHECKBOX_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^- \[([^\] ]|.{2,})\]").unwrap());

/// Issue code descriptions
fn issue_description(code: &str) -> &'static str {
    match code {
        "E000" => "Cannot read file",
        "E001" => "Missing frontmatter",
        "E002" => "Invalid YAML syntax",
        "E003" => "Missing required field",
        "E004" => "Invalid ID format",
        "E005" => "ID mismatch with filename",
        "E006" => "Invalid status value",
        "E007" => "Duplicate ID across threads",
        "W004" => "Old log format",
        "W005" => "Invalid timestamp",
        "W006" => "Malformed checkbox",
        "W007" => "Log entry missing or legacy timestamp",
        "W008" => "Legacy date header",
        "W009" => "Filename missing ID prefix",
        "W010" => "Legacy markdown section found",
        _ => "Unknown issue",
    }
}

// ============================================================================
// CLI Arguments
// ============================================================================

#[derive(Args)]
pub struct ValidateArgs {
    #[command(subcommand)]
    action: Option<ValidateAction>,

    /// Path to validate (git-root-relative, ./pwd-relative, or absolute)
    #[arg(default_value = "", global = true)]
    path: String,

    /// Validate all threads in workspace
    #[arg(short = 'a', long, global = true)]
    all: bool,

    #[command(flatten)]
    direction: DirectionArgs,

    #[command(flatten)]
    filter: FilterArgs,

    #[command(flatten)]
    format: FormatArgs,
}

#[derive(Subcommand)]
enum ValidateAction {
    /// Run validation checks (default)
    Check {
        /// Show each issue with file:line
        #[arg(short = 'v', long)]
        verbose: bool,
    },

    /// Show issue statistics by type
    Stats,

    /// Auto-fix issues where possible
    Fix {
        /// Fix E002: Quote frontmatter values that break YAML parsing
        #[arg(long)]
        e002: bool,

        /// Fix W007: Add timestamps to log entries (from git blame)
        #[arg(long)]
        w007: bool,

        /// Fix W010: Strip legacy markdown sections (migrate to current format)
        #[arg(long)]
        w010: bool,

        /// Show what would be fixed without making changes
        #[arg(long)]
        dry_run: bool,
    },
}

// ============================================================================
// Data Structures
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Severity::Error => write!(f, "error"),
            Severity::Warning => write!(f, "warning"),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Issue {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<usize>,
    pub severity: Severity,
    pub code: String,
    pub message: String,
}

impl Issue {
    fn error(code: &str, message: impl Into<String>) -> Self {
        Self {
            line: None,
            severity: Severity::Error,
            code: code.to_string(),
            message: message.into(),
        }
    }

    fn error_at(code: &str, line: usize, message: impl Into<String>) -> Self {
        Self {
            line: Some(line),
            severity: Severity::Error,
            code: code.to_string(),
            message: message.into(),
        }
    }

    #[allow(dead_code)]
    fn warning(code: &str, message: impl Into<String>) -> Self {
        Self {
            line: None,
            severity: Severity::Warning,
            code: code.to_string(),
            message: message.into(),
        }
    }

    fn warning_at(code: &str, line: usize, message: impl Into<String>) -> Self {
        Self {
            line: Some(line),
            severity: Severity::Warning,
            code: code.to_string(),
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct FileResult {
    pub path: String,
    pub issues: Vec<Issue>,
}

impl FileResult {
    fn is_valid(&self) -> bool {
        self.issues.is_empty()
    }

    fn error_count(&self) -> usize {
        self.issues
            .iter()
            .filter(|i| i.severity == Severity::Error)
            .count()
    }

    fn warning_count(&self) -> usize {
        self.issues
            .iter()
            .filter(|i| i.severity == Severity::Warning)
            .count()
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ValidationSummary {
    pub total: usize,
    pub valid: usize,
    pub errors: usize,
    pub warnings: usize,
    pub files: Vec<FileResult>,
}

// ============================================================================
// Main Entry Point
// ============================================================================

pub fn run(args: ValidateArgs, ws: &Path, config: &Config) -> Result<(), String> {
    let format = args.format.resolve();

    // Collect thread files to validate
    let files = collect_files(&args, ws)?;

    if files.is_empty() {
        match format {
            OutputFormat::Pretty | OutputFormat::Plain => {
                println!("No threads found to validate");
            }
            OutputFormat::Json | OutputFormat::Yaml => {
                output_check_structured(
                    &ValidationSummary {
                        total: 0,
                        valid: 0,
                        errors: 0,
                        warnings: 0,
                        files: vec![],
                    },
                    format,
                )?;
            }
        }
        return Ok(());
    }

    let include_closed = args.filter.include_closed();

    // Validate all files
    let summary = validate_all(&files, ws, config, include_closed);

    // Dispatch to subcommand
    match args.action {
        None | Some(ValidateAction::Check { verbose: false }) => run_check(&summary, format, false),
        Some(ValidateAction::Check { verbose: true }) => run_check(&summary, format, true),
        Some(ValidateAction::Stats) => run_stats(&summary, format),
        Some(ValidateAction::Fix {
            e002,
            w007,
            w010,
            dry_run,
        }) => run_fix(
            &files,
            ws,
            e002,
            w007,
            w010,
            dry_run,
            format,
            include_closed,
        ),
    }
}

fn collect_files(args: &ValidateArgs, ws: &Path) -> Result<Vec<PathBuf>, String> {
    if args.all {
        workspace::find_all_threads(ws)
    } else {
        let path_filter = if args.path.is_empty() {
            None
        } else {
            Some(args.path.as_str())
        };

        let scope = workspace::infer_scope(ws, path_filter)?;
        let start_path = scope.threads_dir.parent().unwrap_or(ws);

        let options = args.direction.to_find_options();
        workspace::find_threads_with_options(start_path, ws, &options)
    }
}

// ============================================================================
// Check Subcommand
// ============================================================================

fn run_check(
    summary: &ValidationSummary,
    format: OutputFormat,
    verbose: bool,
) -> Result<(), String> {
    match format {
        OutputFormat::Pretty => output_check_pretty(summary, verbose),
        OutputFormat::Plain => output_check_plain(summary, verbose),
        OutputFormat::Json | OutputFormat::Yaml => output_check_structured(summary, format)?,
    }

    if summary.errors > 0 {
        process::exit(1);
    }

    Ok(())
}

fn output_check_pretty(summary: &ValidationSummary, verbose: bool) {
    // Summary line
    if summary.errors == 0 && summary.warnings == 0 {
        println!(
            "Validated {} threads: {}",
            summary.total.to_string().bold(),
            "all valid ✓".green()
        );
    } else {
        let mut parts = vec![format!("{} valid", summary.valid)];
        if summary.errors > 0 {
            parts.push(format!("{} errors", summary.errors).red().to_string());
        }
        if summary.warnings > 0 {
            parts.push(
                format!("{} warnings", summary.warnings)
                    .yellow()
                    .to_string(),
            );
        }
        println!(
            "Validated {} threads: {}",
            summary.total.to_string().bold(),
            parts.join(", ")
        );
    }

    // Show issues
    let files_with_issues: Vec<_> = summary.files.iter().filter(|f| !f.is_valid()).collect();

    if files_with_issues.is_empty() && !verbose {
        return;
    }

    println!();

    for file in &summary.files {
        if file.issues.is_empty() && !verbose {
            continue;
        }

        if file.issues.is_empty() {
            println!("  {} {}", "✓".green(), file.path.dimmed());
        } else {
            println!("  {}", file.path);
            for issue in &file.issues {
                let severity_marker = match issue.severity {
                    Severity::Error => "E".red(),
                    Severity::Warning => "W".yellow(),
                };
                let location = issue.line.map(|l| format!(":{}", l)).unwrap_or_default();
                println!(
                    "    {} {} {}{}",
                    severity_marker,
                    issue.code.dimmed(),
                    issue.message,
                    location.dimmed()
                );
            }
        }
    }

    // Final summary
    if summary.errors > 0 || summary.warnings > 0 {
        println!();
        let mut final_parts = vec![];
        if summary.errors > 0 {
            final_parts.push(format!("{} error(s)", summary.errors));
        }
        if summary.warnings > 0 {
            final_parts.push(format!("{} warning(s)", summary.warnings));
        }
        println!("{}", final_parts.join(", "));
    }
}

fn output_check_plain(summary: &ValidationSummary, verbose: bool) {
    println!(
        "Validated {} threads: {} valid, {} errors, {} warnings",
        summary.total, summary.valid, summary.errors, summary.warnings
    );

    let files_with_issues: Vec<_> = summary.files.iter().filter(|f| !f.is_valid()).collect();

    if files_with_issues.is_empty() && !verbose {
        return;
    }

    println!();

    for file in &summary.files {
        if file.issues.is_empty() && !verbose {
            continue;
        }

        if file.issues.is_empty() {
            println!("OK: {}", file.path);
        } else {
            for issue in &file.issues {
                let location = issue.line.map(|l| format!(":{}", l)).unwrap_or_default();
                println!(
                    "{}: {}{}: [{}] {}",
                    issue.severity.to_string().to_uppercase(),
                    file.path,
                    location,
                    issue.code,
                    issue.message
                );
            }
        }
    }
}

fn output_check_structured(
    summary: &ValidationSummary,
    format: OutputFormat,
) -> Result<(), String> {
    match format {
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&summary)
                .map_err(|e| format!("JSON serialization failed: {}", e))?;
            println!("{}", json);
        }
        OutputFormat::Yaml => {
            let yaml = serde_yaml::to_string(&summary)
                .map_err(|e| format!("YAML serialization failed: {}", e))?;
            print!("{}", yaml);
        }
        _ => {}
    }
    Ok(())
}

// ============================================================================
// Stats Subcommand
// ============================================================================

#[derive(Debug, Clone, Serialize)]
struct IssueStat {
    code: String,
    severity: Severity,
    count: usize,
    description: String,
}

fn run_stats(summary: &ValidationSummary, format: OutputFormat) -> Result<(), String> {
    // Count issues by code
    let mut counts: HashMap<String, (Severity, usize)> = HashMap::new();

    for file in &summary.files {
        for issue in &file.issues {
            let entry = counts
                .entry(issue.code.clone())
                .or_insert((issue.severity, 0));
            entry.1 += 1;
        }
    }

    // Convert to sorted vec
    let mut stats: Vec<IssueStat> = counts
        .into_iter()
        .map(|(code, (severity, count))| IssueStat {
            description: issue_description(&code).to_string(),
            code,
            severity,
            count,
        })
        .collect();

    // Sort: errors first, then by count descending
    stats.sort_by(|a, b| match (&a.severity, &b.severity) {
        (Severity::Error, Severity::Warning) => std::cmp::Ordering::Less,
        (Severity::Warning, Severity::Error) => std::cmp::Ordering::Greater,
        _ => b.count.cmp(&a.count),
    });

    match format {
        OutputFormat::Pretty => output_stats_pretty(summary, &stats),
        OutputFormat::Plain => output_stats_plain(summary, &stats),
        OutputFormat::Json => output_stats_json(summary, &stats)?,
        OutputFormat::Yaml => output_stats_yaml(summary, &stats)?,
    }

    Ok(())
}

fn output_stats_pretty(summary: &ValidationSummary, stats: &[IssueStat]) {
    println!(
        "Validated {} threads: {} valid, {} errors, {} warnings",
        summary.total.to_string().bold(),
        summary.valid,
        if summary.errors > 0 {
            summary.errors.to_string().red().to_string()
        } else {
            "0".to_string()
        },
        if summary.warnings > 0 {
            summary.warnings.to_string().yellow().to_string()
        } else {
            "0".to_string()
        }
    );
    println!();

    if stats.is_empty() {
        println!("No issues found");
        return;
    }

    // Table header
    println!(
        "  {} {:>5}  {}",
        "CODE".dimmed(),
        "COUNT".dimmed(),
        "DESCRIPTION".dimmed()
    );

    for stat in stats {
        let severity_color = match stat.severity {
            Severity::Error => stat.code.red(),
            Severity::Warning => stat.code.yellow(),
        };
        println!(
            "  {} {:>5}  {}",
            severity_color, stat.count, stat.description
        );
    }
}

fn output_stats_plain(summary: &ValidationSummary, stats: &[IssueStat]) {
    println!(
        "Validated {} threads: {} valid, {} errors, {} warnings",
        summary.total, summary.valid, summary.errors, summary.warnings
    );
    println!();

    if stats.is_empty() {
        println!("No issues found");
        return;
    }

    println!("CODE | SEVERITY | COUNT | DESCRIPTION");

    for stat in stats {
        println!(
            "{} | {} | {} | {}",
            stat.code, stat.severity, stat.count, stat.description
        );
    }
}

fn output_stats_json(summary: &ValidationSummary, stats: &[IssueStat]) -> Result<(), String> {
    #[derive(Serialize)]
    struct Output {
        total: usize,
        valid: usize,
        errors: usize,
        warnings: usize,
        by_code: Vec<IssueStat>,
    }

    let output = Output {
        total: summary.total,
        valid: summary.valid,
        errors: summary.errors,
        warnings: summary.warnings,
        by_code: stats.to_vec(),
    };

    let json = serde_json::to_string_pretty(&output)
        .map_err(|e| format!("JSON serialization failed: {}", e))?;
    println!("{}", json);
    Ok(())
}

fn output_stats_yaml(summary: &ValidationSummary, stats: &[IssueStat]) -> Result<(), String> {
    #[derive(Serialize)]
    struct Output {
        total: usize,
        valid: usize,
        errors: usize,
        warnings: usize,
        by_code: Vec<IssueStat>,
    }

    let output = Output {
        total: summary.total,
        valid: summary.valid,
        errors: summary.errors,
        warnings: summary.warnings,
        by_code: stats.to_vec(),
    };

    let yaml =
        serde_yaml::to_string(&output).map_err(|e| format!("YAML serialization failed: {}", e))?;
    print!("{}", yaml);
    Ok(())
}

// ============================================================================
// Validation Logic
// ============================================================================

fn validate_all(
    files: &[PathBuf],
    ws: &Path,
    config: &Config,
    include_closed: bool,
) -> ValidationSummary {
    let mut results: Vec<FileResult> = Vec::new();
    let mut ids_seen: HashMap<String, PathBuf> = HashMap::new();

    for path in files {
        let rel_path = path
            .strip_prefix(ws)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| path.to_string_lossy().to_string());

        let mut issues = Vec::new();

        // Read file content
        let content = match fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) => {
                issues.push(Issue::error("E000", format!("cannot read file: {}", e)));
                results.push(FileResult {
                    path: rel_path,
                    issues,
                });
                continue;
            }
        };

        // Validate frontmatter
        let fm_result = validate_frontmatter(&content, path, config);
        issues.extend(fm_result.issues);

        // Skip closed threads unless include_closed is set
        if !include_closed
            && let Some(ref status) = fm_result.status
            && thread::is_closed(status)
        {
            continue;
        }

        // Check for duplicate IDs (E007)
        if let Some(ref id) = fm_result.id {
            if let Some(other_path) = ids_seen.get(id) {
                let other_rel = other_path
                    .strip_prefix(ws)
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_else(|_| other_path.to_string_lossy().to_string());
                issues.push(Issue::error(
                    "E007",
                    format!("duplicate ID '{}' (also in {})", id, other_rel),
                ));
            } else {
                ids_seen.insert(id.clone(), path.clone());
            }
        }

        // Validate sections
        issues.extend(validate_sections(&content, config));

        // Validate log entries
        issues.extend(validate_log(&content));

        // Validate todo items
        issues.extend(validate_todos(&content));

        results.push(FileResult {
            path: rel_path,
            issues,
        });
    }

    // Compute summary
    let valid = results.iter().filter(|r| r.is_valid()).count();
    let errors: usize = results.iter().map(|r| r.error_count()).sum();
    let warnings: usize = results.iter().map(|r| r.warning_count()).sum();

    ValidationSummary {
        total: results.len(),
        valid,
        errors,
        warnings,
        files: results,
    }
}

struct FrontmatterResult {
    id: Option<String>,
    status: Option<String>,
    issues: Vec<Issue>,
}

fn validate_frontmatter(content: &str, path: &Path, config: &Config) -> FrontmatterResult {
    let mut issues = Vec::new();

    // E001: Check for frontmatter delimiters
    if !content.starts_with("---\n") {
        issues.push(Issue::error_at("E001", 1, "missing frontmatter delimiter"));
        return FrontmatterResult {
            id: None,
            status: None,
            issues,
        };
    }

    // Find closing delimiter
    let rest = &content[4..];
    let end = match rest.find("\n---") {
        Some(e) => e,
        None => {
            issues.push(Issue::error(
                "E001",
                "unclosed frontmatter (missing closing ---)",
            ));
            return FrontmatterResult {
                id: None,
                status: None,
                issues,
            };
        }
    };

    let yaml_content = &rest[..end];

    // E002: Parse YAML
    let fm: Frontmatter = match serde_yaml::from_str(yaml_content) {
        Ok(fm) => fm,
        Err(e) => {
            let line = extract_yaml_error_line(&e);
            if let Some(l) = line {
                issues.push(Issue::error_at(
                    "E002",
                    l + 1,
                    format!("invalid YAML: {}", e),
                ));
            } else {
                issues.push(Issue::error("E002", format!("invalid YAML: {}", e)));
            }
            return FrontmatterResult {
                id: None,
                status: None,
                issues,
            };
        }
    };

    // E003: Check required fields
    if fm.id.is_empty() {
        issues.push(Issue::error("E003", "missing required field: id"));
    }
    if fm.name.is_empty() {
        issues.push(Issue::error("E003", "missing required field: name"));
    }
    if fm.status.is_empty() {
        issues.push(Issue::error("E003", "missing required field: status"));
    }

    // E004: Validate ID format
    if !fm.id.is_empty() && !VALID_ID_RE.is_match(&fm.id) {
        issues.push(Issue::error(
            "E004",
            format!("invalid ID format '{}' (expected 6 hex chars)", fm.id),
        ));
    }

    // E005: Check ID matches filename
    if !fm.id.is_empty()
        && let Some(filename_id) = extract_id_from_path(path)
        && fm.id != filename_id
    {
        issues.push(Issue::error(
            "E005",
            format!(
                "ID mismatch: frontmatter has '{}', filename has '{}'",
                fm.id, filename_id
            ),
        ));
    }

    // W009: Frontmatter has ID but filename has no ID prefix
    if !fm.id.is_empty() && extract_id_from_path(path).is_none() {
        issues.push(Issue::warning(
            "W009",
            format!("frontmatter id '{}' not reflected in filename", fm.id),
        ));
    }

    // E006: Validate status using config status lists
    if !fm.status.is_empty()
        && !thread::is_valid_status_with_config(
            &fm.status,
            &config.status.open,
            &config.status.closed,
        )
    {
        let base = thread::base_status(&fm.status);
        issues.push(Issue::error("E006", format!("invalid status '{}'", base)));
    }

    let extracted_id = if fm.id.is_empty() {
        extract_id_from_path(path)
    } else {
        Some(fm.id)
    };

    let extracted_status = if fm.status.is_empty() {
        None
    } else {
        Some(fm.status)
    };

    FrontmatterResult {
        id: extracted_id,
        status: extracted_status,
        issues,
    }
}

fn extract_yaml_error_line(e: &serde_yaml::Error) -> Option<usize> {
    e.location().map(|loc| loc.line())
}

fn validate_sections(content: &str, _config: &Config) -> Vec<Issue> {
    let mut issues = Vec::new();

    for (line_num, line) in content.lines().enumerate() {
        if let Some(caps) = SECTION_HEADER_RE.captures(line) {
            let section = caps.get(1).unwrap().as_str();
            let line_display = line_num + 1;

            // W010: Any legacy section name found means the file needs migration.
            // Non-legacy ## headers in body content are fine and are ignored.
            if LEGACY_SECTIONS.contains(&section) {
                issues.push(Issue::warning_at(
                    "W010",
                    line_display,
                    format!(
                        "legacy section '## {}' found — run 'threads migrate'",
                        section
                    ),
                ));
            }
        }
    }

    issues
}

fn validate_log(content: &str) -> Vec<Issue> {
    let mut issues = Vec::new();
    let mut in_log_section = false;
    let mut has_date_header = false;

    for (line_num, line) in content.lines().enumerate() {
        let line_display = line_num + 1;

        if line.starts_with("## ") {
            in_log_section = line.starts_with("## Log");
            has_date_header = false;
            continue;
        }

        if !in_log_section {
            continue;
        }

        // W008: Legacy date headers should be removed (dates go in entries)
        if LOG_DATE_HEADER_RE.is_match(line) {
            has_date_header = true;
            issues.push(Issue::warning_at(
                "W008",
                line_display,
                "legacy date header - run 'validate fix --w007' to migrate",
            ));
            continue;
        }

        // Check log entries (lines starting with "- ")
        if line.starts_with("- ") {
            let entry_content = line.strip_prefix("- ").unwrap_or(line);

            // Skip continuation lines (bold labels, table rows, etc.)
            if is_non_log_list_item(entry_content) {
                continue;
            }

            // Current format: - [YYYY-MM-DD HH:MM:SS] text
            if line.starts_with("- [") {
                if BRACKET_LOG_FORMAT_RE.is_match(line) {
                    // Valid current format, check timestamp validity
                    if let Some(caps) = BRACKET_LOG_FORMAT_RE.captures(line) {
                        let ts = &caps[1];
                        if !is_valid_timestamp(ts) {
                            issues.push(Issue::warning_at(
                                "W005",
                                line_display,
                                format!("invalid timestamp '{}'", ts),
                            ));
                        }
                    }
                } else {
                    // Has brackets but not a valid timestamp - might be malformed
                    issues.push(Issue::warning_at(
                        "W007",
                        line_display,
                        "log entry missing timestamp",
                    ));
                }
            } else if line.starts_with("- **") {
                // Legacy bold formats
                if BOLD_LOG_FORMAT_RE.is_match(line) {
                    issues.push(Issue::warning_at(
                        "W007",
                        line_display,
                        "legacy bold timestamp - run 'validate fix --w007' to migrate",
                    ));
                } else if TIME_ONLY_FORMAT_RE.is_match(line) {
                    if has_date_header {
                        issues.push(Issue::warning_at(
                            "W007",
                            line_display,
                            "legacy time-only format - run 'validate fix --w007' to migrate",
                        ));
                    } else {
                        issues.push(Issue::warning_at(
                            "W004",
                            line_display,
                            "time-only format without date header",
                        ));
                    }
                }
                // Note: Bold text that isn't a timestamp is handled by is_non_log_list_item above
            } else {
                // Plain list item without any timestamp
                issues.push(Issue::warning_at(
                    "W007",
                    line_display,
                    "log entry missing timestamp",
                ));
            }
        }
    }

    issues
}

fn validate_todos(content: &str) -> Vec<Issue> {
    let mut issues = Vec::new();
    let mut in_todo_section = false;

    for (line_num, line) in content.lines().enumerate() {
        let line_display = line_num + 1;

        if line.starts_with("## ") {
            in_todo_section = line.starts_with("## Todo");
            continue;
        }

        if !in_todo_section {
            continue;
        }

        if line.trim().is_empty() {
            continue;
        }

        if line.starts_with("- [") {
            if TODO_CHECKBOX_RE.is_match(line) {
                continue;
            }

            if MALFORMED_CHECKBOX_RE.is_match(line) {
                issues.push(Issue::warning_at(
                    "W006",
                    line_display,
                    "malformed checkbox (use '- [ ]' or '- [x]')",
                ));
            }
        }
    }

    issues
}

fn is_valid_timestamp(ts: &str) -> bool {
    chrono::NaiveDateTime::parse_from_str(ts, "%Y-%m-%d %H:%M:%S").is_ok()
}

// ============================================================================
// Fix Subcommand
// ============================================================================

fn run_fix(
    files: &[PathBuf],
    ws: &Path,
    fix_e002: bool,
    fix_w007: bool,
    fix_w010: bool,
    dry_run: bool,
    format: OutputFormat,
    include_closed: bool,
) -> Result<(), String> {
    if !fix_e002 && !fix_w007 && !fix_w010 {
        return Err("specify at least one fix: --e002, --w007, --w010".to_string());
    }

    let mut frontmatter_fixed = 0;
    let mut log_entries_fixed = 0;
    let mut headers_removed = 0;
    let mut legacy_migrated = 0;
    let mut files_modified = 0;
    let mut fix_entries: Vec<FixEntry> = Vec::new();

    for path in files {
        let rel_path = path
            .strip_prefix(ws)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| path.to_string_lossy().to_string());

        let content = match fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        // Filter by status unless include_closed
        if !include_closed
            && let Some(status) = extract_status_from_content(&content)
            && thread::is_closed(&status)
        {
            continue;
        }

        let mut current_content = content.clone();
        let mut file_changed = false;
        let mut file_fm_fixed = 0;
        let mut file_log_fixed = 0;
        let mut file_headers_removed = 0;
        let mut file_legacy_migrated = false;

        // E002: Fix frontmatter quoting
        if fix_e002 {
            let (new_content, fixed) = fix_frontmatter_quoting(
                &current_content,
                &rel_path,
                dry_run,
                format,
                &mut fix_entries,
            );
            if fixed > 0 {
                file_fm_fixed = fixed;
                current_content = new_content;
                file_changed = true;
            }
        }

        // W007: Fix log timestamps
        if fix_w007 {
            let (new_lines, fixes, removed) = fix_log_section(
                &current_content,
                path,
                ws,
                dry_run,
                &rel_path,
                format,
                &mut fix_entries,
            );
            if fixes > 0 || removed > 0 {
                file_log_fixed = fixes;
                file_headers_removed = removed;
                current_content = new_lines.join("\n") + "\n";
                file_changed = true;
            }
        }

        // W010: migrate legacy sections.
        // migrate_file_for_validate handles its own file write; we only track the count here.
        if fix_w010 {
            match migrate_file_for_validate(path, ws, dry_run) {
                Ok(true) => {
                    file_legacy_migrated = true;
                }
                Ok(false) => {}
                Err(e) => {
                    eprintln!("W010 fix failed for {}: {}", rel_path, e);
                }
            }
        }

        // E002/W007: write updated content if modified
        if file_changed {
            frontmatter_fixed += file_fm_fixed;
            log_entries_fixed += file_log_fixed;
            headers_removed += file_headers_removed;
            files_modified += 1;

            if !dry_run {
                fs::write(path, &current_content)
                    .map_err(|e| format!("failed to write {}: {}", rel_path, e))?;

                match format {
                    OutputFormat::Pretty | OutputFormat::Plain => {
                        let mut parts = Vec::new();
                        if file_fm_fixed > 0 {
                            parts.push(format!("{} frontmatter fields", file_fm_fixed));
                        }
                        if file_log_fixed > 0 {
                            parts.push(format!("{} log entries", file_log_fixed));
                        }
                        if file_headers_removed > 0 {
                            parts.push(format!("{} headers removed", file_headers_removed));
                        }
                        println!("Fixed {} in {}", parts.join(", "), rel_path);
                    }
                    _ => {}
                }
            }
        }

        if file_legacy_migrated {
            legacy_migrated += 1;
            if !file_changed {
                files_modified += 1;
            }
        }
    }

    // Summary
    match format {
        OutputFormat::Pretty | OutputFormat::Plain => {
            println!();
            let mut parts = Vec::new();
            if frontmatter_fixed > 0 {
                parts.push(format!("{} frontmatter fields", frontmatter_fixed));
            }
            if log_entries_fixed > 0 {
                parts.push(format!("{} log entries", log_entries_fixed));
            }
            if headers_removed > 0 {
                parts.push(format!("{} headers removed", headers_removed));
            }
            if legacy_migrated > 0 {
                parts.push(format!("{} files migrated", legacy_migrated));
            }

            if dry_run {
                if parts.is_empty() {
                    println!("Dry run: nothing to fix");
                } else {
                    println!(
                        "Dry run: would fix {} in {} files",
                        parts.join(", "),
                        files_modified
                    );
                }
            } else if parts.is_empty() {
                println!("Nothing to fix");
            } else {
                println!("Fixed {} in {} files", parts.join(", "), files_modified);
            }
        }
        OutputFormat::Json => {
            let output = serde_json::json!({
                "dry_run": dry_run,
                "frontmatter_fixed": frontmatter_fixed,
                "log_entries_fixed": log_entries_fixed,
                "headers_removed": headers_removed,
                "legacy_migrated": legacy_migrated,
                "files_modified": files_modified,
                "changes": fix_entries,
            });
            println!("{}", serde_json::to_string_pretty(&output).unwrap());
        }
        OutputFormat::Yaml => {
            let output = serde_json::json!({
                "dry_run": dry_run,
                "frontmatter_fixed": frontmatter_fixed,
                "log_entries_fixed": log_entries_fixed,
                "headers_removed": headers_removed,
                "legacy_migrated": legacy_migrated,
                "files_modified": files_modified,
                "changes": fix_entries,
            });
            println!("{}", serde_yaml::to_string(&output).unwrap());
        }
    }

    Ok(())
}

/// Fix frontmatter quoting: quote values that contain YAML-special characters
fn fix_frontmatter_quoting(
    content: &str,
    rel_path: &str,
    dry_run: bool,
    format: OutputFormat,
    fix_entries: &mut Vec<FixEntry>,
) -> (String, usize) {
    // Check for frontmatter delimiters
    if !content.starts_with("---\n") {
        return (content.to_string(), 0);
    }

    let rest = &content[4..];
    let end = match rest.find("\n---") {
        Some(e) => e,
        None => return (content.to_string(), 0),
    };

    let yaml_content = &rest[..end];
    let after_frontmatter = &rest[end + 4..]; // Skip \n---

    // Check if YAML parses successfully - if so, no fix needed
    if serde_yaml::from_str::<Frontmatter>(yaml_content).is_ok() {
        return (content.to_string(), 0);
    }

    // Parse frontmatter line by line and fix quoting
    let mut fixed_lines: Vec<String> = Vec::new();
    let mut fixes = 0;

    for (i, line) in yaml_content.lines().enumerate() {
        let line_num = i + 2; // +1 for 0-index, +1 for opening ---

        if let Some((key, value)) = parse_yaml_line(line) {
            let needs_quoting = yaml_value_needs_quoting(value);
            let is_already_quoted = (value.starts_with('"') && value.ends_with('"'))
                || (value.starts_with('\'') && value.ends_with('\''));

            if needs_quoting && !is_already_quoted {
                // Quote the value
                let quoted = quote_yaml_value(value);
                let fixed_line = format!("{}: {}", key, quoted);

                if dry_run {
                    print_fix(format, rel_path, line_num, line, &fixed_line, fix_entries);
                }

                fixed_lines.push(fixed_line);
                fixes += 1;
            } else {
                fixed_lines.push(line.to_string());
            }
        } else {
            fixed_lines.push(line.to_string());
        }
    }

    if fixes == 0 {
        return (content.to_string(), 0);
    }

    // Reconstruct the file
    let new_content = format!("---\n{}\n---{}", fixed_lines.join("\n"), after_frontmatter);
    (new_content, fixes)
}

/// Parse a simple YAML line into key and value
fn parse_yaml_line(line: &str) -> Option<(&str, &str)> {
    let colon_pos = line.find(':')?;
    let key = line[..colon_pos].trim();
    let value = line[colon_pos + 1..].trim();

    // Skip empty values or nested structures
    if value.is_empty() || key.is_empty() {
        return None;
    }

    Some((key, value))
}

/// Check if a YAML value needs quoting
fn yaml_value_needs_quoting(value: &str) -> bool {
    // Already quoted
    if (value.starts_with('"') && value.ends_with('"'))
        || (value.starts_with('\'') && value.ends_with('\''))
    {
        return false;
    }

    // Contains YAML-special characters that break parsing
    let special_chars = [
        ':', '#', '[', ']', '{', '}', ',', '&', '*', '!', '|', '>', '%', '@', '`',
    ];
    if value.chars().any(|c| special_chars.contains(&c)) {
        return true;
    }

    // Starts with special characters
    let special_starts = [
        '-', '?', ':', '&', '*', '!', '|', '>', '\'', '"', '%', '@', '`',
    ];
    if let Some(first) = value.chars().next()
        && special_starts.contains(&first)
    {
        return true;
    }

    // Contains leading/trailing whitespace that would be trimmed
    if value != value.trim() {
        return true;
    }

    // Looks like a number, boolean, or null but should be a string
    let lower = value.to_lowercase();
    if lower == "true"
        || lower == "false"
        || lower == "null"
        || lower == "yes"
        || lower == "no"
        || lower == "on"
        || lower == "off"
    {
        return true;
    }

    // Looks like a number
    if value.parse::<f64>().is_ok() {
        return true;
    }

    false
}

/// Quote a YAML value using single quotes (escaping single quotes inside)
fn quote_yaml_value(value: &str) -> String {
    // Prefer single quotes unless value contains single quotes
    if value.contains('\'') {
        // Use double quotes, escape internal double quotes
        let escaped = value.replace('\\', "\\\\").replace('"', "\\\"");
        format!("\"{}\"", escaped)
    } else {
        format!("'{}'", value)
    }
}

/// Extract status from content using line-by-line parsing (works even with broken YAML)
fn extract_status_from_content(content: &str) -> Option<String> {
    // Find frontmatter
    if !content.starts_with("---\n") {
        return None;
    }

    let rest = &content[4..];
    let end = rest.find("\n---")?;
    let yaml_content = &rest[..end];

    // Look for status line
    for line in yaml_content.lines() {
        if let Some((key, value)) = parse_yaml_line(line)
            && key == "status"
        {
            // Remove quotes if present
            let status = value
                .trim_start_matches('"')
                .trim_end_matches('"')
                .trim_start_matches('\'')
                .trim_end_matches('\'');
            return Some(status.to_string());
        }
    }

    None
}

/// Fix log section: migrate legacy formats to bracket format, remove date headers
fn fix_log_section(
    content: &str,
    path: &Path,
    ws: &Path,
    dry_run: bool,
    rel_path: &str,
    format: OutputFormat,
    fix_entries: &mut Vec<FixEntry>,
) -> (Vec<String>, usize, usize) {
    let lines: Vec<&str> = content.lines().collect();
    let mut result: Vec<String> = Vec::new();
    let mut fixes = 0;
    let mut headers_removed = 0;
    let mut in_log_section = false;
    let mut current_date: Option<String> = None;

    for (i, line) in lines.iter().enumerate() {
        // Track section changes
        if line.starts_with("## ") {
            in_log_section = line.starts_with("## Log");
            current_date = None;
            result.push(line.to_string());
            continue;
        }

        if !in_log_section {
            result.push(line.to_string());
            continue;
        }

        // Handle date headers - extract date but don't include in output
        if let Some(caps) = LOG_DATE_HEADER_RE.captures(line) {
            current_date = Some(caps[1].to_string());
            headers_removed += 1;
            if dry_run {
                print_fix(format, rel_path, i + 1, line, "(removed)", fix_entries);
            }
            continue; // Skip adding to result
        }

        // Handle log entries
        if line.starts_with("- ") {
            // Already in current format - keep as is
            if BRACKET_LOG_FORMAT_RE.is_match(line) {
                result.push(line.to_string());
                continue;
            }

            // Legacy bold full timestamp: - **YYYY-MM-DD HH:MM:SS** text
            if let Some(caps) = BOLD_LOG_FORMAT_RE.captures(line) {
                let ts = &caps[1];
                let rest = line.strip_prefix("- **").unwrap();
                let rest = &rest[ts.len() + 2..]; // Skip timestamp and closing **
                let new_line = format!("- [{}]{}", ts, rest);
                if dry_run {
                    print_fix(format, rel_path, i + 1, line, &new_line, fix_entries);
                }
                result.push(new_line);
                fixes += 1;
                continue;
            }

            // Legacy time-only format: - **HH:MM** text (under date header)
            if let Some(caps) = TIME_ONLY_FORMAT_RE.captures(line) {
                let time = &caps[1];
                let rest = line.strip_prefix("- **").unwrap();
                let rest = &rest[time.len() + 2..]; // Skip time and closing **

                if let Some(ref date) = current_date {
                    let new_line = format!("- [{} {}:00]{}", date, time, rest);
                    if dry_run {
                        print_fix(format, rel_path, i + 1, line, &new_line, fix_entries);
                    }
                    result.push(new_line);
                    fixes += 1;
                } else {
                    // No date context - fall back to git blame
                    if let Some(ts) = get_blame_timestamp(path, ws, i + 1) {
                        let new_line = format!("- [{}]{}", ts, rest);
                        if dry_run {
                            print_fix(format, rel_path, i + 1, line, &new_line, fix_entries);
                        }
                        result.push(new_line);
                        fixes += 1;
                    } else {
                        result.push(line.to_string()); // Can't fix, keep original
                    }
                }
                continue;
            }

            // Entry without timestamp - add one
            let entry_content = line.strip_prefix("- ").unwrap_or(line);

            // Skip entries that look like code/formatting, not log entries
            if is_non_log_list_item(entry_content) {
                result.push(line.to_string());
                continue;
            }

            if let Some(ref date) = current_date {
                // Use date from header + default time 12:00:00
                let new_line = format!("- [{} 12:00:00] {}", date, entry_content);
                if dry_run {
                    print_fix(format, rel_path, i + 1, line, &new_line, fix_entries);
                }
                result.push(new_line);
                fixes += 1;
            } else {
                // No date context - fall back to git blame
                if let Some(ts) = get_blame_timestamp(path, ws, i + 1) {
                    let new_line = format!("- [{}] {}", ts, entry_content);
                    if dry_run {
                        print_fix(format, rel_path, i + 1, line, &new_line, fix_entries);
                    }
                    result.push(new_line);
                    fixes += 1;
                } else {
                    result.push(line.to_string()); // Can't fix, keep original
                }
            }
            continue;
        }

        // Non-entry lines (empty, continuations, etc.) - keep as is
        result.push(line.to_string());
    }

    (result, fixes, headers_removed)
}

#[derive(serde::Serialize)]
struct FixEntry {
    path: String,
    line: usize,
    old: String,
    new: String,
}

fn print_fix(
    format: OutputFormat,
    rel_path: &str,
    line_num: usize,
    old: &str,
    new: &str,
    fixes: &mut Vec<FixEntry>,
) {
    match format {
        OutputFormat::Pretty | OutputFormat::Plain => {
            println!("{}:{}", rel_path, line_num);
            println!("  - {}", old);
            println!("  + {}", new);
        }
        _ => fixes.push(FixEntry {
            path: rel_path.to_string(),
            line: line_num,
            old: old.to_string(),
            new: new.to_string(),
        }),
    }
}

/// Check if a list item content looks like code/formatting rather than a log entry
fn is_non_log_list_item(content: &str) -> bool {
    let trimmed = content.trim();

    // Code fence
    if trimmed.starts_with("```") {
        return true;
    }

    // Shell command (likely inside code block)
    if trimmed.starts_with("$ ") || (trimmed.starts_with("# ") && !trimmed.starts_with("## ")) {
        return true;
    }

    // Markdown header inside list item
    if trimmed.starts_with("### ") || trimmed.starts_with("#### ") {
        return true;
    }

    // Lines that are clearly continuations (start with common code patterns)
    if trimmed.starts_with("git ") || trimmed.starts_with("cd ") || trimmed.starts_with("./") {
        return true;
    }

    // Table rows (markdown tables)
    if trimmed.starts_with('|') {
        return true;
    }

    // Bold text that is NOT a timestamp (continuation headers like "**Results:**")
    // Timestamps look like **YYYY-MM-DD or **HH:MM** - other bold is content
    if let Some(after_bold) = trimmed.strip_prefix("**") {
        // Check if it's NOT a timestamp pattern
        // Timestamp patterns: YYYY-MM-DD or HH:MM
        let is_date = after_bold.len() >= 10
            && after_bold.chars().take(4).all(|c| c.is_ascii_digit())
            && after_bold.chars().nth(4) == Some('-');
        let is_time = after_bold.len() >= 5
            && after_bold.chars().take(2).all(|c| c.is_ascii_digit())
            && after_bold.chars().nth(2) == Some(':');
        if !is_date && !is_time {
            return true;
        }
    }

    // "See X", "Note:", etc. - common continuation patterns
    if trimmed.starts_with("See ") || trimmed.starts_with("Note:") {
        return true;
    }

    false
}

/// Get timestamp from git blame for a specific line
fn get_blame_timestamp(path: &Path, ws: &Path, line_num: usize) -> Option<String> {
    use std::process::Command;

    let output = Command::new("git")
        .args([
            "-C",
            &ws.to_string_lossy(),
            "blame",
            "-L",
            &format!("{},{}", line_num, line_num),
            "--porcelain",
            &path.to_string_lossy(),
        ])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Parse porcelain format for committer-time
    for line in stdout.lines() {
        if let Some(ts_str) = line.strip_prefix("committer-time ")
            && let Ok(ts) = ts_str.parse::<i64>()
        {
            let dt = chrono::DateTime::from_timestamp(ts, 0)?;
            return Some(dt.format("%Y-%m-%d %H:%M:%S").to_string());
        }
    }

    None
}

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process;
use std::sync::LazyLock;

use clap::{Args, Subcommand};
use colored::Colorize;
use regex::Regex;
use serde::Serialize;

use crate::output::OutputFormat;
use crate::thread::{self, extract_id_from_path, Frontmatter};
use crate::workspace::{self, FindOptions};

// ============================================================================
// Regexes for validation
// ============================================================================

/// Matches a valid 6-character hex ID
static VALID_ID_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[0-9a-f]{6}$").unwrap());

/// Matches section headers (## Name)
static SECTION_HEADER_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?m)^## (.+)$").unwrap());

/// Matches valid section names
static VALID_SECTIONS: &[&str] = &["Body", "Notes", "Todo", "Log"];

/// Canonical section order
static SECTION_ORDER: &[&str] = &["Body", "Notes", "Todo", "Log"];

/// Matches log date headers (### YYYY-MM-DD)
static LOG_DATE_HEADER_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^### (\d{4}-\d{2}-\d{2})$").unwrap());

/// Matches new log format: - **YYYY-MM-DD HH:MM:SS** text
static NEW_LOG_FORMAT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^- \*\*(\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2})\*\*").unwrap());

/// Matches old log format: - **HH:MM** text (without date header context)
static OLD_LOG_FORMAT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^- \*\*(\d{2}:\d{2})\*\*").unwrap());

/// Matches todo checkbox line
static TODO_CHECKBOX_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^- \[([ xX])\]").unwrap());

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
        "W001" => "Unknown section header",
        "W002" => "Duplicate section",
        "W003" => "Sections out of order",
        "W004" => "Old log format",
        "W005" => "Invalid timestamp",
        "W006" => "Malformed checkbox",
        "W007" => "Log entry missing timestamp",
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

    /// Search subdirectories (unlimited depth, or specify N levels)
    #[arg(short = 'd', long = "down", value_name = "N", global = true)]
    down: Option<Option<usize>>,

    /// Search parent directories (up to git root, or specify N levels)
    #[arg(short = 'u', long = "up", value_name = "N", global = true)]
    up: Option<Option<usize>>,

    /// Output format (auto-detects TTY for pretty vs plain)
    #[arg(short = 'f', long, value_enum, default_value = "pretty", global = true)]
    format: OutputFormat,

    /// Output as JSON (shorthand for --format=json)
    #[arg(long, conflicts_with = "format", global = true)]
    json: bool,
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
        /// Fix W007: Add timestamps to log entries (from git blame)
        #[arg(long)]
        w007: bool,

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

pub fn run(args: ValidateArgs, ws: &Path) -> Result<(), String> {
    let format = if args.json {
        OutputFormat::Json
    } else {
        args.format.resolve()
    };

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

    // Validate all files
    let summary = validate_all(&files, ws);

    // Dispatch to subcommand
    match args.action {
        None | Some(ValidateAction::Check { verbose: false }) => {
            run_check(&summary, format, false)
        }
        Some(ValidateAction::Check { verbose: true }) => {
            run_check(&summary, format, true)
        }
        Some(ValidateAction::Stats) => run_stats(&summary, format),
        Some(ValidateAction::Fix { w007, dry_run }) => run_fix(&files, ws, w007, dry_run, format),
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

        let mut options = FindOptions::new();

        if let Some(depth) = args.down {
            options = options.with_down(depth);
        }

        if let Some(depth) = args.up {
            options = options.with_up(depth);
        }

        workspace::find_threads_with_options(start_path, ws, &options)
    }
}

// ============================================================================
// Check Subcommand
// ============================================================================

fn run_check(summary: &ValidationSummary, format: OutputFormat, verbose: bool) -> Result<(), String> {
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
            parts.push(format!("{} warnings", summary.warnings).yellow().to_string());
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
                let location = issue
                    .line
                    .map(|l| format!(":{}", l))
                    .unwrap_or_default();
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
                let location = issue
                    .line
                    .map(|l| format!(":{}", l))
                    .unwrap_or_default();
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

fn output_check_structured(summary: &ValidationSummary, format: OutputFormat) -> Result<(), String> {
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
            let entry = counts.entry(issue.code.clone()).or_insert((issue.severity, 0));
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
    stats.sort_by(|a, b| {
        match (&a.severity, &b.severity) {
            (Severity::Error, Severity::Warning) => std::cmp::Ordering::Less,
            (Severity::Warning, Severity::Error) => std::cmp::Ordering::Greater,
            _ => b.count.cmp(&a.count),
        }
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
            severity_color,
            stat.count,
            stat.description
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

    let yaml = serde_yaml::to_string(&output)
        .map_err(|e| format!("YAML serialization failed: {}", e))?;
    print!("{}", yaml);
    Ok(())
}

// ============================================================================
// Validation Logic
// ============================================================================

fn validate_all(files: &[PathBuf], ws: &Path) -> ValidationSummary {
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
        let fm_result = validate_frontmatter(&content, path);
        issues.extend(fm_result.issues);

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
        issues.extend(validate_sections(&content));

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
    issues: Vec<Issue>,
}

fn validate_frontmatter(content: &str, path: &Path) -> FrontmatterResult {
    let mut issues = Vec::new();
    let extracted_id: Option<String>;

    // E001: Check for frontmatter delimiters
    if !content.starts_with("---\n") {
        issues.push(Issue::error_at("E001", 1, "missing frontmatter delimiter"));
        return FrontmatterResult {
            id: None,
            issues,
        };
    }

    // Find closing delimiter
    let rest = &content[4..];
    let end = match rest.find("\n---") {
        Some(e) => e,
        None => {
            issues.push(Issue::error("E001", "unclosed frontmatter (missing closing ---)"));
            return FrontmatterResult {
                id: None,
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
                issues.push(Issue::error_at("E002", l + 1, format!("invalid YAML: {}", e)));
            } else {
                issues.push(Issue::error("E002", format!("invalid YAML: {}", e)));
            }
            return FrontmatterResult {
                id: None,
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
    if !fm.id.is_empty() {
        if let Some(filename_id) = extract_id_from_path(path) {
            if fm.id != filename_id {
                issues.push(Issue::error(
                    "E005",
                    format!(
                        "ID mismatch: frontmatter has '{}', filename has '{}'",
                        fm.id, filename_id
                    ),
                ));
            }
        }
    }

    // E006: Validate status
    if !fm.status.is_empty() && !thread::is_valid_status(&fm.status) {
        let base = thread::base_status(&fm.status);
        issues.push(Issue::error("E006", format!("invalid status '{}'", base)));
    }

    extracted_id = if fm.id.is_empty() {
        extract_id_from_path(path)
    } else {
        Some(fm.id)
    };

    FrontmatterResult {
        id: extracted_id,
        issues,
    }
}

fn extract_yaml_error_line(e: &serde_yaml::Error) -> Option<usize> {
    e.location().map(|loc| loc.line())
}

fn validate_sections(content: &str) -> Vec<Issue> {
    let mut issues = Vec::new();
    let mut seen_sections: HashMap<String, usize> = HashMap::new();
    let mut section_positions: Vec<(String, usize)> = Vec::new();

    for (line_num, line) in content.lines().enumerate() {
        if let Some(caps) = SECTION_HEADER_RE.captures(line) {
            let section = caps.get(1).unwrap().as_str().to_string();
            let line_display = line_num + 1;

            // W001: Unknown section
            if !VALID_SECTIONS.contains(&section.as_str()) {
                issues.push(Issue::warning_at(
                    "W001",
                    line_display,
                    format!("unknown section '{}'", section),
                ));
            }

            // W002: Duplicate section
            if let Some(&first_line) = seen_sections.get(&section) {
                issues.push(Issue::warning_at(
                    "W002",
                    line_display,
                    format!("duplicate section '{}' (first at line {})", section, first_line),
                ));
            } else {
                seen_sections.insert(section.clone(), line_display);
                section_positions.push((section, line_display));
            }
        }
    }

    // W003: Check section order
    let known_positions: Vec<(usize, usize)> = section_positions
        .iter()
        .filter_map(|(name, line)| {
            SECTION_ORDER
                .iter()
                .position(|&s| s == name)
                .map(|order| (order, *line))
        })
        .collect();

    for i in 1..known_positions.len() {
        if known_positions[i].0 < known_positions[i - 1].0 {
            let current_name = SECTION_ORDER[known_positions[i].0];
            let prev_name = SECTION_ORDER[known_positions[i - 1].0];
            issues.push(Issue::warning_at(
                "W003",
                known_positions[i].1,
                format!("section '{}' should come before '{}'", current_name, prev_name),
            ));
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

        if LOG_DATE_HEADER_RE.is_match(line) {
            has_date_header = true;
            continue;
        }

        // Check log entries (lines starting with "- ")
        if line.starts_with("- ") {
            if line.starts_with("- **") {
                if NEW_LOG_FORMAT_RE.is_match(line) {
                    // Valid new format, check timestamp validity
                    if let Some(caps) = NEW_LOG_FORMAT_RE.captures(line) {
                        let ts = &caps[1];
                        if !is_valid_timestamp(ts) {
                            issues.push(Issue::warning_at(
                                "W005",
                                line_display,
                                format!("invalid timestamp '{}'", ts),
                            ));
                        }
                    }
                } else if OLD_LOG_FORMAT_RE.is_match(line) {
                    // Old format: - **HH:MM** (only valid under date header)
                    if !has_date_header {
                        issues.push(Issue::warning_at(
                            "W004",
                            line_display,
                            "old log format (HH:MM without date header) - use YYYY-MM-DD HH:MM:SS",
                        ));
                    }
                } else {
                    // Has bold but not a recognized timestamp format
                    issues.push(Issue::warning_at(
                        "W007",
                        line_display,
                        "log entry missing timestamp",
                    ));
                }
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
    fix_w007: bool,
    dry_run: bool,
    format: OutputFormat,
) -> Result<(), String> {
    if !fix_w007 {
        return Err("specify at least one fix: --w007".to_string());
    }

    let mut total_fixed = 0;
    let mut files_modified = 0;

    for path in files {
        let rel_path = path
            .strip_prefix(ws)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| path.to_string_lossy().to_string());

        let content = match fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let mut lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();
        let mut modified = false;
        let mut file_fixes = 0;

        if fix_w007 {
            let log_lines = find_log_lines_needing_timestamp(&lines);

            for line_num in log_lines {
                // Get timestamp from git blame (1-indexed for git)
                if let Some(timestamp) = get_blame_timestamp(path, ws, line_num + 1) {
                    let old_line = &lines[line_num];
                    let new_line = add_timestamp_to_log_entry(old_line, &timestamp);

                    if dry_run {
                        match format {
                            OutputFormat::Pretty | OutputFormat::Plain => {
                                println!("{}:{}", rel_path, line_num + 1);
                                println!("  - {}", old_line);
                                println!("  + {}", new_line);
                            }
                            _ => {}
                        }
                    }

                    lines[line_num] = new_line;
                    modified = true;
                    file_fixes += 1;
                }
            }
        }

        if modified {
            total_fixed += file_fixes;
            files_modified += 1;

            if !dry_run {
                // Write back with trailing newline
                let new_content = lines.join("\n") + "\n";
                fs::write(path, new_content)
                    .map_err(|e| format!("failed to write {}: {}", rel_path, e))?;

                match format {
                    OutputFormat::Pretty | OutputFormat::Plain => {
                        println!("Fixed {} entries in {}", file_fixes, rel_path);
                    }
                    _ => {}
                }
            }
        }
    }

    // Summary
    match format {
        OutputFormat::Pretty | OutputFormat::Plain => {
            println!();
            if dry_run {
                println!(
                    "Dry run: would fix {} entries in {} files",
                    total_fixed, files_modified
                );
            } else {
                println!("Fixed {} entries in {} files", total_fixed, files_modified);
            }
        }
        OutputFormat::Json => {
            let output = serde_json::json!({
                "dry_run": dry_run,
                "fixed": total_fixed,
                "files_modified": files_modified,
            });
            println!("{}", serde_json::to_string_pretty(&output).unwrap());
        }
        OutputFormat::Yaml => {
            let output = serde_json::json!({
                "dry_run": dry_run,
                "fixed": total_fixed,
                "files_modified": files_modified,
            });
            println!("{}", serde_yaml::to_string(&output).unwrap());
        }
    }

    Ok(())
}

/// Find line indices in the Log section that need timestamps (W007)
fn find_log_lines_needing_timestamp(lines: &[String]) -> Vec<usize> {
    let mut result = Vec::new();
    let mut in_log_section = false;

    for (i, line) in lines.iter().enumerate() {
        if line.starts_with("## ") {
            in_log_section = line.starts_with("## Log");
            continue;
        }

        if !in_log_section {
            continue;
        }

        // Check for log entries without proper timestamps
        if line.starts_with("- ") {
            if line.starts_with("- **") {
                // Has bold - check if it's a valid timestamp format
                if !NEW_LOG_FORMAT_RE.is_match(line) && !OLD_LOG_FORMAT_RE.is_match(line) {
                    result.push(i);
                }
            } else {
                // Plain list item without any timestamp
                result.push(i);
            }
        }
    }

    result
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
        if let Some(ts_str) = line.strip_prefix("committer-time ") {
            if let Ok(ts) = ts_str.parse::<i64>() {
                let dt = chrono::DateTime::from_timestamp(ts, 0)?;
                return Some(dt.format("%Y-%m-%d %H:%M:%S").to_string());
            }
        }
    }

    None
}

/// Add timestamp to a log entry line
fn add_timestamp_to_log_entry(line: &str, timestamp: &str) -> String {
    // Remove "- " prefix
    let content = line.strip_prefix("- ").unwrap_or(line);

    // Remove any existing bold markers at the start (malformed timestamps)
    let content = if content.starts_with("**") {
        // Find closing ** and strip
        if let Some(end) = content[2..].find("**") {
            let inner = &content[2..2 + end];
            let rest = &content[2 + end + 2..];
            // Keep the inner content but without bold, prepend to rest
            format!("{}{}", inner.trim(), rest)
        } else {
            content.to_string()
        }
    } else {
        content.to_string()
    };

    format!("- **{}** {}", timestamp, content.trim())
}

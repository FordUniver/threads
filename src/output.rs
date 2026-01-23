//! Output formatting utilities with TTY auto-detection and semantic styling.

use std::io::IsTerminal;

use chrono::{DateTime, Local};
use clap::ValueEnum;
use colored::{ColoredString, Colorize};

/// Output format for commands.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, ValueEnum)]
pub enum OutputFormat {
    /// Human-optimized: colors, boxes, relative dates
    #[default]
    Pretty,
    /// LLM-optimized: no colors, pipe-delimited, full paths
    Plain,
    /// Machine-readable JSON with ISO 8601 timestamps
    Json,
    /// Machine-readable YAML with ISO 8601 timestamps
    Yaml,
}

impl OutputFormat {
    /// Resolve the output format, applying environment variables and TTY auto-detection.
    ///
    /// Priority for Pretty format:
    /// 1. NO_COLOR env var (if set and non-empty) → Plain
    /// 2. FORCE_COLOR env var (if set and non-empty) → Pretty (skip TTY check)
    /// 3. TTY detection → Plain if not a TTY, Pretty otherwise
    ///
    /// Json, Yaml, and Plain formats are returned as-is (considered explicit choices).
    pub fn resolve(self) -> Self {
        match self {
            OutputFormat::Pretty => {
                // NO_COLOR takes precedence (https://no-color.org/)
                if std::env::var("NO_COLOR")
                    .map(|v| !v.is_empty())
                    .unwrap_or(false)
                {
                    return OutputFormat::Plain;
                }
                // FORCE_COLOR forces pretty output
                if std::env::var("FORCE_COLOR")
                    .map(|v| !v.is_empty())
                    .unwrap_or(false)
                {
                    return OutputFormat::Pretty;
                }
                // TTY detection as fallback
                if std::io::stdout().is_terminal() {
                    OutputFormat::Pretty
                } else {
                    OutputFormat::Plain
                }
            }
            other => other,
        }
    }
}

// ============================================================================
// Semantic Styling - Centralized color/style decisions
// ============================================================================

/// Status lifecycle colors.
/// - Green: active work
/// - Yellow: blocked/waiting
/// - Blue: planning phase (NOT cyan - cyan reserved for UI markers)
/// - Dimmed: closed states
pub fn style_status(status: &str) -> ColoredString {
    match status {
        "active" => status.green(),
        "blocked" | "paused" => status.yellow(),
        "planning" | "idea" => status.blue(),
        "resolved" | "superseded" | "deferred" | "rejected" => status.dimmed(),
        _ => status.normal(),
    }
}

/// Style status using config colors.
///
/// Color names supported: green, yellow, blue, red, cyan, magenta, white, dim/dimmed
pub fn style_status_with_config(
    status: &str,
    colors: Option<&crate::config::StatusColors>,
) -> ColoredString {
    let color = colors.and_then(|c| match status {
        "active" => c.active.as_deref(),
        "blocked" => c.blocked.as_deref(),
        "paused" => c.paused.as_deref(),
        "idea" => c.idea.as_deref(),
        "planning" => c.planning.as_deref(),
        "resolved" => c.resolved.as_deref(),
        "superseded" => c.superseded.as_deref(),
        "deferred" => c.deferred.as_deref(),
        "rejected" => c.rejected.as_deref(),
        _ => None,
    });

    match color {
        Some("green") => status.green(),
        Some("yellow") => status.yellow(),
        Some("blue") => status.blue(),
        Some("red") => status.red(),
        Some("cyan") => status.cyan(),
        Some("magenta") => status.magenta(),
        Some("white") => status.white(),
        Some("dim") | Some("dimmed") => status.dimmed(),
        Some(_) | None => style_status(status), // fallback to defaults
    }
}

/// Style for IDs and hashes - always dimmed.
pub fn style_id(id: &str) -> ColoredString {
    id.dimmed()
}

/// Style for paths - dimmed by default, bold if it's PWD.
pub fn style_path(path: &str, is_pwd: bool) -> String {
    if is_pwd {
        path.bold().to_string()
    } else {
        path.dimmed().to_string()
    }
}

// ============================================================================
// Terminal utilities
// ============================================================================

/// Get terminal width, defaulting to 80 if unavailable.
pub fn terminal_width() -> usize {
    terminal_size::terminal_size()
        .map(|(w, _)| w.0 as usize)
        .unwrap_or(80)
}

// ============================================================================
// Date formatting
// ============================================================================

/// Format a datetime as short relative time (e.g., "5m", "3h", "2d", "1w", "2mo", "1y").
pub fn format_relative_short(dt: DateTime<Local>) -> String {
    let now = Local::now();
    let duration = now.signed_duration_since(dt);

    let seconds = duration.num_seconds().abs();
    let minutes = duration.num_minutes().abs();
    let hours = duration.num_hours().abs();
    let days = duration.num_days().abs();

    if seconds < 60 {
        "now".to_string()
    } else if minutes < 60 {
        format!("{}m", minutes)
    } else if hours < 24 {
        format!("{}h", hours)
    } else if days < 7 {
        format!("{}d", days)
    } else if days < 30 {
        format!("{}w", days / 7)
    } else if days < 365 {
        format!("{}mo", days / 30)
    } else {
        format!("{}y", days / 365)
    }
}

// ============================================================================
// Path utilities
// ============================================================================

/// Truncate a string from the front, showing "..suffix".
/// Useful for paths where the end is more meaningful.
pub fn truncate_front(s: &str, max_chars: usize) -> String {
    let char_count = s.chars().count();
    if char_count <= max_chars {
        s.to_string()
    } else if max_chars <= 2 {
        "..".to_string()
    } else {
        let skip = char_count - (max_chars - 2);
        let truncated: String = s.chars().skip(skip).collect();
        format!("..{}", truncated)
    }
}

/// Truncate a string from the back, showing "prefix…".
pub fn truncate_back(s: &str, max_chars: usize) -> String {
    let char_count = s.chars().count();
    if char_count <= max_chars {
        s.to_string()
    } else if max_chars <= 1 {
        "…".to_string()
    } else {
        let truncated: String = s.chars().take(max_chars - 1).collect();
        format!("{}…", truncated)
    }
}

/// Compute the shortest path representation.
/// Compares git-root-relative path vs PWD-relative path and returns the shorter one.
pub fn shortest_path(git_rel_path: &str, pwd_rel: &str) -> String {
    let target = if git_rel_path.is_empty() || git_rel_path == "." {
        ""
    } else {
        git_rel_path.trim_end_matches('/')
    };
    let pwd = if pwd_rel.is_empty() || pwd_rel == "." {
        ""
    } else {
        pwd_rel.trim_end_matches('/')
    };

    // Same location
    if target == pwd {
        return ".".to_string();
    }

    // Target is under PWD
    if !pwd.is_empty() && target.starts_with(&format!("{}/", pwd)) {
        let rel = &target[pwd.len() + 1..];
        let pwd_relative = format!("./{}", rel);
        if pwd_relative.len() < git_rel_path.len() {
            return pwd_relative;
        }
        return git_rel_path.to_string();
    }

    // PWD is under target (target is ancestor of PWD)
    if !target.is_empty() && pwd.starts_with(&format!("{}/", target)) {
        let depth = pwd[target.len() + 1..].matches('/').count() + 1;
        let pwd_relative = (0..depth).map(|_| "..").collect::<Vec<_>>().join("/");
        if pwd_relative.len() < git_rel_path.len() {
            return pwd_relative;
        }
        return git_rel_path.to_string();
    }

    // Target is at git root, PWD is somewhere inside
    if target.is_empty() && !pwd.is_empty() {
        let depth = pwd.matches('/').count() + 1;
        let pwd_relative = (0..depth).map(|_| "..").collect::<Vec<_>>().join("/");
        if pwd_relative.len() < git_rel_path.len() {
            return pwd_relative;
        }
        return ".".to_string();
    }

    // PWD is at git root, target is somewhere inside
    if pwd.is_empty() && !target.is_empty() {
        return git_rel_path.to_string();
    }

    // Different branches - compute common ancestor and relative path
    let target_parts: Vec<&str> = target.split('/').collect();
    let pwd_parts: Vec<&str> = pwd.split('/').collect();

    let common_len = target_parts
        .iter()
        .zip(pwd_parts.iter())
        .take_while(|(a, b)| a == b)
        .count();

    let ups = pwd_parts.len() - common_len;
    let downs = &target_parts[common_len..];

    let mut parts: Vec<&str> = (0..ups).map(|_| "..").collect();
    parts.extend(downs);
    let pwd_relative = parts.join("/");

    if pwd_relative.len() < git_rel_path.len() {
        pwd_relative
    } else {
        git_rel_path.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // Mutex to serialize env var tests (they modify global state)
    static ENV_MUTEX: Mutex<()> = Mutex::new(());

    fn with_env<F, R>(vars: &[(&str, Option<&str>)], f: F) -> R
    where
        F: FnOnce() -> R,
    {
        let _guard = ENV_MUTEX.lock().unwrap();

        // Save original values
        let originals: Vec<_> = vars
            .iter()
            .map(|(k, _)| (*k, std::env::var(*k).ok()))
            .collect();

        // Set test values
        for (k, v) in vars {
            match v {
                Some(val) => std::env::set_var(k, val),
                None => std::env::remove_var(k),
            }
        }

        let result = f();

        // Restore original values
        for (k, original) in originals {
            match original {
                Some(val) => std::env::set_var(k, val),
                None => std::env::remove_var(k),
            }
        }

        result
    }

    #[test]
    fn test_no_color_disables_pretty() {
        with_env(&[("NO_COLOR", Some("1")), ("FORCE_COLOR", None)], || {
            assert_eq!(OutputFormat::Pretty.resolve(), OutputFormat::Plain);
        });
    }

    #[test]
    fn test_no_color_empty_is_ignored() {
        with_env(&[("NO_COLOR", Some("")), ("FORCE_COLOR", None)], || {
            // Empty NO_COLOR should be ignored, falls through to TTY detection
            // In test context, stdout is typically not a TTY
            let result = OutputFormat::Pretty.resolve();
            // We can't assert a specific value here since it depends on TTY state
            assert!(result == OutputFormat::Pretty || result == OutputFormat::Plain);
        });
    }

    #[test]
    fn test_force_color_enables_pretty() {
        with_env(&[("NO_COLOR", None), ("FORCE_COLOR", Some("1"))], || {
            // FORCE_COLOR should force Pretty regardless of TTY
            assert_eq!(OutputFormat::Pretty.resolve(), OutputFormat::Pretty);
        });
    }

    #[test]
    fn test_no_color_takes_precedence_over_force_color() {
        with_env(
            &[("NO_COLOR", Some("1")), ("FORCE_COLOR", Some("1"))],
            || {
                // NO_COLOR takes precedence
                assert_eq!(OutputFormat::Pretty.resolve(), OutputFormat::Plain);
            },
        );
    }

    #[test]
    fn test_env_vars_dont_affect_explicit_formats() {
        with_env(&[("NO_COLOR", Some("1")), ("FORCE_COLOR", None)], || {
            // Explicit formats are returned as-is
            assert_eq!(OutputFormat::Json.resolve(), OutputFormat::Json);
            assert_eq!(OutputFormat::Yaml.resolve(), OutputFormat::Yaml);
            assert_eq!(OutputFormat::Plain.resolve(), OutputFormat::Plain);
        });
    }
}

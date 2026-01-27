//! Shared CLI argument structs for consistent flag definitions across commands.
//!
//! These structs centralize common flags like format, direction, and filter options.
//! Use `#[command(flatten)]` to include them in command-specific Args structs.
//!
//! Environment variable overrides:
//! - `THREADS_FORMAT` → default format (pretty, plain, json, yaml)
//! - `THREADS_INCLUDE_CLOSED` → include closed threads by default
//! - `THREADS_DOWN` → default --down depth
//! - `THREADS_UP` → default --up depth

use clap::Args;

use crate::config::{env_bool, env_string, env_usize};
use crate::output::OutputFormat;
use crate::workspace::FindOptions;

// ============================================================================
// FormatArgs - Output format flags
// ============================================================================

/// Common output format flags.
///
/// Provides consistent --format/-f and --json flags across commands.
/// Use `resolve()` to get the effective format with TTY auto-detection.
#[derive(Args, Clone, Debug, Default)]
pub struct FormatArgs {
    /// Output format (auto-detects TTY for pretty vs plain)
    #[arg(short = 'f', long, value_enum, default_value = "pretty", global = true)]
    pub format: OutputFormat,

    /// Output as JSON (shorthand for --format=json)
    #[arg(long, conflicts_with = "format", global = true)]
    pub json: bool,
}

impl FormatArgs {
    /// Resolve the effective output format.
    ///
    /// Priority: --json flag > --format flag > THREADS_FORMAT env > default (pretty)
    /// Then applies NO_COLOR/FORCE_COLOR/TTY detection via OutputFormat::resolve().
    pub fn resolve(&self) -> OutputFormat {
        if self.json {
            return OutputFormat::Json;
        }

        // Check if format was explicitly set (not default)
        // If format is not pretty (the default), user explicitly chose it
        if self.format != OutputFormat::Pretty {
            return self.format.resolve();
        }

        // Check THREADS_FORMAT env var
        if let Some(env_format) = env_string("THREADS_FORMAT") {
            match env_format.to_lowercase().as_str() {
                "pretty" => return OutputFormat::Pretty.resolve(),
                "plain" => return OutputFormat::Plain,
                "json" => return OutputFormat::Json,
                "yaml" => return OutputFormat::Yaml,
                _ => {} // Invalid value, fall through to default
            }
        }

        // Default with TTY/color detection
        self.format.resolve()
    }
}

// ============================================================================
// DirectionArgs - Search direction flags
// ============================================================================

/// Common direction flags for hierarchical thread search.
///
/// Provides --down/-d and --up/-u flags for controlling search scope.
/// Use `to_find_options()` to convert to FindOptions for workspace search.
#[derive(Args, Clone, Debug, Default)]
pub struct DirectionArgs {
    /// Search subdirectories (unlimited depth, or specify N levels)
    #[arg(short = 'd', long = "down", value_name = "N", global = true)]
    pub down: Option<Option<usize>>,

    /// Search parent directories (up to git root, or specify N levels)
    #[arg(short = 'u', long = "up", value_name = "N", global = true)]
    pub up: Option<Option<usize>>,
}

impl DirectionArgs {
    /// Convert to FindOptions for workspace search.
    ///
    /// Priority: CLI flags > THREADS_DOWN/THREADS_UP env > default (local only)
    pub fn to_find_options(&self) -> FindOptions {
        let mut options = FindOptions::new();

        // --down/-d takes priority, then env var
        let down_opt = if self.down.is_some() {
            self.down
        } else {
            // Check THREADS_DOWN env var
            Self::parse_depth_env("THREADS_DOWN")
        };

        if let Some(depth) = down_opt {
            options = options.with_down(depth);
        }

        // --up takes priority, then env var
        let up_opt = if self.up.is_some() {
            self.up
        } else {
            Self::parse_depth_env("THREADS_UP")
        };

        if let Some(depth) = up_opt {
            options = options.with_up(depth);
        }

        options
    }

    /// Parse a depth env var (number for limit, "unlimited"/empty for unlimited)
    fn parse_depth_env(name: &str) -> Option<Option<usize>> {
        let value = env_string(name)?;
        let lower = value.to_lowercase();

        if lower == "unlimited" || lower == "all" {
            Some(None) // unlimited
        } else if let Some(n) = env_usize(name) {
            if n == 0 {
                Some(None) // 0 means unlimited
            } else {
                Some(Some(n))
            }
        } else {
            None // invalid value, ignore
        }
    }

    /// Check if any direction search is active (from flags or env vars).
    pub fn is_searching(&self) -> bool {
        self.down.is_some()
            || self.up.is_some()
            || Self::parse_depth_env("THREADS_DOWN").is_some()
            || Self::parse_depth_env("THREADS_UP").is_some()
    }

    /// Get a description of the active search direction for display.
    ///
    /// Returns strings like "(recursive)", "(down 2)", "(up)", "(down, up 3)".
    /// Returns empty string if no direction flags are active.
    pub fn description(&self) -> String {
        let mut parts = Vec::new();

        // Resolve down option (--down/-d takes priority, then env)
        let down_opt = if self.down.is_some() {
            self.down
        } else {
            Self::parse_depth_env("THREADS_DOWN")
        };

        if let Some(depth) = down_opt {
            match depth {
                Some(n) => parts.push(format!("down {}", n)),
                None => parts.push("down".to_string()),
            }
        }

        // Resolve up option (--up takes priority, then env)
        let up_opt = if self.up.is_some() {
            self.up
        } else {
            Self::parse_depth_env("THREADS_UP")
        };

        if let Some(depth) = up_opt {
            match depth {
                Some(n) => parts.push(format!("up {}", n)),
                None => parts.push("up".to_string()),
            }
        }

        if parts.is_empty() {
            String::new()
        } else {
            format!("({})", parts.join(", "))
        }
    }
}

// ============================================================================
// FilterArgs - Thread status filter flags
// ============================================================================

/// Common filter flags for thread status filtering.
///
/// Provides --include-closed/-c flag for showing closed threads.
#[derive(Args, Clone, Debug, Default)]
pub struct FilterArgs {
    /// Include closed threads (resolved/superseded/deferred/rejected)
    #[arg(short = 'c', long = "include-closed", global = true)]
    pub include_closed: bool,

    /// Hidden alias for backward compatibility
    #[arg(long = "include-concluded", hide = true, global = true)]
    include_concluded: bool,
}

impl FilterArgs {
    /// Check if closed threads should be included.
    ///
    /// Priority: CLI flags > THREADS_INCLUDE_CLOSED env > default (false)
    pub fn include_closed(&self) -> bool {
        // CLI flags take priority
        if self.include_closed || self.include_concluded {
            return true;
        }

        // Check THREADS_INCLUDE_CLOSED env var
        env_bool("THREADS_INCLUDE_CLOSED").unwrap_or(false)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_direction_description() {
        // No flags
        let args = DirectionArgs::default();
        assert_eq!(args.description(), "");
        assert!(!args.is_searching());

        // Down with limit
        let args = DirectionArgs {
            down: Some(Some(2)),
            ..Default::default()
        };
        assert_eq!(args.description(), "(down 2)");
        assert!(args.is_searching());

        // Down unlimited
        let args = DirectionArgs {
            down: Some(None),
            ..Default::default()
        };
        assert_eq!(args.description(), "(down)");
        assert!(args.is_searching());

        // Up only
        let args = DirectionArgs {
            up: Some(None),
            ..Default::default()
        };
        assert_eq!(args.description(), "(up)");
        assert!(args.is_searching());

        // Up with limit
        let args = DirectionArgs {
            up: Some(Some(3)),
            ..Default::default()
        };
        assert_eq!(args.description(), "(up 3)");
        assert!(args.is_searching());

        // Both down and up
        let args = DirectionArgs {
            down: Some(Some(2)),
            up: Some(None),
            ..Default::default()
        };
        assert_eq!(args.description(), "(down 2, up)");
        assert!(args.is_searching());

        // Down with limit still works
        let args = DirectionArgs {
            down: Some(Some(1)),
            ..Default::default()
        };
        assert_eq!(args.description(), "(down 1)");
    }

    #[test]
    fn test_filter_include_closed() {
        // Neither flag
        let args = FilterArgs::default();
        assert!(!args.include_closed());

        // include_closed
        let args = FilterArgs {
            include_closed: true,
            ..Default::default()
        };
        assert!(args.include_closed());

        // include_concluded (backward compat)
        let args = FilterArgs {
            include_concluded: true,
            ..Default::default()
        };
        assert!(args.include_closed());

        // Both (shouldn't happen but handle gracefully)
        let args = FilterArgs {
            include_concluded: true,
            include_closed: true,
        };
        assert!(args.include_closed());
    }

    #[test]
    fn test_direction_to_find_options() {
        // Down with limit
        let args = DirectionArgs {
            down: Some(Some(3)),
            ..Default::default()
        };
        let opts = args.to_find_options();
        assert_eq!(opts.down, Some(Some(3)));

        // Up with limit
        let args = DirectionArgs {
            up: Some(Some(2)),
            ..Default::default()
        };
        let opts = args.to_find_options();
        assert_eq!(opts.up, Some(Some(2)));

        // Down unlimited
        let args = DirectionArgs {
            down: Some(None),
            ..Default::default()
        };
        let opts = args.to_find_options();
        assert_eq!(opts.down, Some(None));
    }
}

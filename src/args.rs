//! Shared CLI argument structs for consistent flag definitions across commands.
//!
//! These structs centralize common flags like format, direction, and filter options.
//! Use `#[command(flatten)]` to include them in command-specific Args structs.

use clap::Args;

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
    /// Handles --json shorthand and applies TTY auto-detection for pretty mode.
    pub fn resolve(&self) -> OutputFormat {
        if self.json {
            OutputFormat::Json
        } else {
            self.format.resolve()
        }
    }
}

// ============================================================================
// DirectionArgs - Search direction flags
// ============================================================================

/// Common direction flags for recursive/hierarchical thread search.
///
/// Provides --down/-d/-r and --up/-u flags for controlling search scope.
/// Use `to_find_options()` to convert to FindOptions for workspace search.
#[derive(Args, Clone, Debug, Default)]
pub struct DirectionArgs {
    /// Search subdirectories (unlimited depth, or specify N levels)
    #[arg(short = 'd', long = "down", value_name = "N", global = true)]
    pub down: Option<Option<usize>>,

    /// Alias for --down (backward compatibility)
    #[arg(short = 'r', long, conflicts_with = "down", global = true)]
    pub recursive: bool,

    /// Search parent directories (up to git root, or specify N levels)
    #[arg(short = 'u', long = "up", value_name = "N", global = true)]
    pub up: Option<Option<usize>>,
}

impl DirectionArgs {
    /// Convert to FindOptions for workspace search.
    pub fn to_find_options(&self) -> FindOptions {
        let mut options = FindOptions::new();

        // --down/-d takes priority, then -r as alias for unlimited down
        let down_opt = if self.down.is_some() {
            self.down
        } else if self.recursive {
            Some(None) // unlimited depth
        } else {
            None
        };

        if let Some(depth) = down_opt {
            options = options.with_down(depth);
        }

        if let Some(depth) = self.up {
            options = options.with_up(depth);
        }

        options
    }

    /// Check if any direction search is active.
    pub fn is_searching(&self) -> bool {
        self.down.is_some() || self.recursive || self.up.is_some()
    }

    /// Get a description of the active search direction for display.
    ///
    /// Returns strings like "(recursive)", "(down 2)", "(up)", "(down, up 3)".
    /// Returns empty string if no direction flags are active.
    pub fn description(&self) -> String {
        let mut parts = Vec::new();

        // Resolve down option (--down/-d takes priority over -r)
        let down_opt = if self.down.is_some() {
            self.down
        } else if self.recursive {
            Some(None)
        } else {
            None
        };

        if let Some(depth) = down_opt {
            match depth {
                Some(n) => parts.push(format!("down {}", n)),
                None => parts.push("recursive".to_string()),
            }
        }

        if let Some(depth) = self.up {
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
    /// Returns true if either --include-closed or --include-concluded is set.
    pub fn include_closed(&self) -> bool {
        self.include_closed || self.include_concluded
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

        // Recursive only
        let args = DirectionArgs {
            recursive: true,
            ..Default::default()
        };
        assert_eq!(args.description(), "(recursive)");
        assert!(args.is_searching());

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
        assert_eq!(args.description(), "(recursive)");
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

        // Down takes priority over recursive
        let args = DirectionArgs {
            down: Some(Some(1)),
            recursive: true, // Should be ignored
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
        // Recursive flag sets unlimited down
        let args = DirectionArgs {
            recursive: true,
            ..Default::default()
        };
        let opts = args.to_find_options();
        assert_eq!(opts.down, Some(None)); // Unlimited
        assert_eq!(opts.up, None);

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

        // Down takes priority over recursive
        let args = DirectionArgs {
            down: Some(Some(1)),
            recursive: true,
            ..Default::default()
        };
        let opts = args.to_find_options();
        assert_eq!(opts.down, Some(Some(1))); // down wins, not unlimited
    }
}

//! Output formatting utilities with TTY auto-detection.

use std::io::IsTerminal;

use clap::ValueEnum;

/// Output format for commands.
#[derive(Clone, Copy, Default, ValueEnum, PartialEq, Eq)]
pub enum OutputFormat {
    /// Compact, colored output for terminals
    #[default]
    Fancy,
    /// Verbose output with full paths (default when not TTY)
    Plain,
    /// JSON output for machine processing
    Json,
    /// YAML output for machine processing
    Yaml,
}

impl OutputFormat {
    /// Resolve the output format, applying TTY auto-detection.
    ///
    /// If format is Fancy but stdout is not a TTY, returns Plain.
    pub fn resolve(self) -> Self {
        match self {
            OutputFormat::Fancy if !std::io::stdout().is_terminal() => OutputFormat::Plain,
            other => other,
        }
    }
}

/// Check if stdout is a terminal.
pub fn is_tty() -> bool {
    std::io::stdout().is_terminal()
}

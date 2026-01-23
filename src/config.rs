//! Configuration system for threads CLI.
//!
//! Configuration is loaded from multiple sources with the following precedence:
//! 1. CLI flags (highest priority)
//! 2. Environment variables (THREADS_*)
//! 3. Project manifest (.threads-config/manifest.yaml)
//! 4. User global (~/.config/threads/config.yaml)
//! 5. Built-in defaults (lowest priority)
//!
//! This module provides:
//! - `Config` struct with all settings
//! - `EnvVar` registry for documentation
//! - Helper functions for env var parsing
//! - Config loading and merging

use std::path::Path;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

// ============================================================================
// Config Structs
// ============================================================================

/// Root configuration for threads CLI.
#[derive(Debug, Default, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct Config {
    /// Status definitions (open vs closed)
    pub status: StatusConfig,
    /// Default values for various operations
    pub defaults: DefaultsConfig,
    /// Display settings
    pub display: DisplayConfig,
    /// Behavior settings
    pub behavior: BehaviorConfig,
    /// Section configuration (rename/disable)
    pub sections: SectionsConfig,
}

/// Status category definitions.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct StatusConfig {
    /// Statuses considered "open" (need attention)
    pub open: Vec<String>,
    /// Statuses considered "closed" (resolved/done)
    pub closed: Vec<String>,
}

impl Default for StatusConfig {
    fn default() -> Self {
        Self {
            open: vec![
                "idea".to_string(),
                "planning".to_string(),
                "active".to_string(),
                "blocked".to_string(),
                "paused".to_string(),
            ],
            closed: vec![
                "resolved".to_string(),
                "superseded".to_string(),
                "deferred".to_string(),
                "rejected".to_string(),
            ],
        }
    }
}

/// Default values for operations.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct DefaultsConfig {
    /// Default status for new threads
    pub new: String,
    /// Default status when closing threads
    pub closed: String,
    /// Default status when reopening threads (fallback after git history)
    pub open: String,
}

impl Default for DefaultsConfig {
    fn default() -> Self {
        Self {
            new: "idea".to_string(),
            closed: "resolved".to_string(),
            open: "active".to_string(),
        }
    }
}

/// Display settings.
#[derive(Debug, Default, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct DisplayConfig {
    /// Custom name for repo root (null = use "repo root")
    pub root_name: Option<String>,
    /// Status colors (null entries use defaults)
    pub status_colors: Option<StatusColors>,
}

/// Custom colors for statuses.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct StatusColors {
    pub active: Option<String>,
    pub blocked: Option<String>,
    pub paused: Option<String>,
    pub idea: Option<String>,
    pub planning: Option<String>,
    pub resolved: Option<String>,
    pub superseded: Option<String>,
    pub deferred: Option<String>,
    pub rejected: Option<String>,
}

impl Default for StatusColors {
    fn default() -> Self {
        Self {
            active: Some("green".to_string()),
            blocked: Some("yellow".to_string()),
            paused: Some("yellow".to_string()),
            idea: Some("blue".to_string()),
            planning: Some("blue".to_string()),
            resolved: Some("dim".to_string()),
            superseded: Some("dim".to_string()),
            deferred: Some("dim".to_string()),
            rejected: Some("dim".to_string()),
        }
    }
}

/// Behavior defaults.
#[derive(Debug, Default, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct BehaviorConfig {
    /// Automatically commit after mutations
    pub auto_commit: bool,
    /// Default --down depth (null = disabled)
    pub default_down: Option<DepthSetting>,
    /// Default --up depth (null = disabled)
    pub default_up: Option<DepthSetting>,
    /// Suppress hints
    pub quiet: bool,
}

/// Depth setting for direction flags.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum DepthSetting {
    /// Specific depth limit
    Limit(usize),
    /// Unlimited depth
    Unlimited,
}

/// Section configuration (rename or disable).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct SectionsConfig {
    /// Body section name (null to disable)
    #[serde(rename = "Body")]
    pub body: Option<String>,
    /// Notes section name (null to disable)
    #[serde(rename = "Notes")]
    pub notes: Option<String>,
    /// Todo section name (null to disable)
    #[serde(rename = "Todo")]
    pub todo: Option<String>,
    /// Log section name (null to disable)
    #[serde(rename = "Log")]
    pub log: Option<String>,
}

impl Default for SectionsConfig {
    fn default() -> Self {
        Self {
            body: Some("Body".to_string()),
            notes: Some("Notes".to_string()),
            todo: Some("Todo".to_string()),
            log: Some("Log".to_string()),
        }
    }
}

// ============================================================================
// Config Source Tracking
// ============================================================================

/// Source of a configuration value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigSource {
    /// Built-in default
    Default,
    /// User global config (~/.config/threads/config.yaml)
    UserGlobal,
    /// Project manifest (.threads-config/manifest.yaml)
    ProjectManifest(String),
    /// Environment variable
    EnvVar(String),
    /// CLI flag
    CliFlag,
}

impl std::fmt::Display for ConfigSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigSource::Default => write!(f, "default"),
            ConfigSource::UserGlobal => write!(f, "~/.config/threads/config.yaml"),
            ConfigSource::ProjectManifest(path) => write!(f, "{}", path),
            ConfigSource::EnvVar(name) => write!(f, "${}", name),
            ConfigSource::CliFlag => write!(f, "CLI flag"),
        }
    }
}

// ============================================================================
// Environment Variable Registry
// ============================================================================

/// Environment variable definition for documentation.
pub struct EnvVar {
    /// Variable name (e.g., "THREADS_FORMAT")
    pub name: &'static str,
    /// Human-readable description
    pub description: &'static str,
    /// Default value or behavior
    pub default: &'static str,
    /// Config path this maps to (e.g., "defaults.new")
    pub config_path: &'static str,
    /// Valid values (if enumerable)
    pub values: Option<&'static str>,
}

/// Registry of all supported environment variables.
pub const ENV_VARS: &[EnvVar] = &[
    EnvVar {
        name: "NO_COLOR",
        description: "Disable colored output (standard)",
        default: "unset",
        config_path: "display.color",
        values: Some("any non-empty value"),
    },
    EnvVar {
        name: "FORCE_COLOR",
        description: "Force colored output even when not a TTY",
        default: "unset",
        config_path: "display.color",
        values: Some("any non-empty value"),
    },
    EnvVar {
        name: "THREADS_FORMAT",
        description: "Default output format",
        default: "pretty (auto-detects TTY)",
        config_path: "display.format",
        values: Some("pretty, plain, json, yaml"),
    },
    EnvVar {
        name: "THREADS_INCLUDE_CLOSED",
        description: "Include closed threads in list/stats by default",
        default: "false",
        config_path: "behavior.include_closed",
        values: Some("1, true, yes"),
    },
    EnvVar {
        name: "THREADS_AUTO_COMMIT",
        description: "Automatically commit after mutations",
        default: "false",
        config_path: "behavior.auto_commit",
        values: Some("1, true, yes"),
    },
    EnvVar {
        name: "THREADS_DEFAULT_STATUS",
        description: "Default status for new threads",
        default: "idea",
        config_path: "defaults.new",
        values: None,
    },
    EnvVar {
        name: "THREADS_DOWN",
        description: "Default --down depth for list/stats",
        default: "unset (local only)",
        config_path: "behavior.default_down",
        values: Some("number or 'unlimited'"),
    },
    EnvVar {
        name: "THREADS_UP",
        description: "Default --up depth for list/stats",
        default: "unset (local only)",
        config_path: "behavior.default_up",
        values: Some("number or 'unlimited'"),
    },
    EnvVar {
        name: "THREADS_QUIET",
        description: "Suppress hint messages",
        default: "false",
        config_path: "behavior.quiet",
        values: Some("1, true, yes"),
    },
    EnvVar {
        name: "THREADS_ROOT",
        description: "Override git root detection",
        default: "auto-detected",
        config_path: "workspace.root",
        values: Some("path"),
    },
];

// ============================================================================
// Environment Variable Helpers
// ============================================================================

/// Parse a boolean environment variable.
///
/// Returns `Some(true)` if the variable is set to a truthy value (1, true, yes),
/// `Some(false)` if set to a falsy value (0, false, no),
/// and `None` if unset or empty.
pub fn env_bool(name: &str) -> Option<bool> {
    std::env::var(name).ok().and_then(|v| {
        if v.is_empty() {
            return None;
        }
        let lower = v.to_lowercase();
        match lower.as_str() {
            "1" | "true" | "yes" => Some(true),
            "0" | "false" | "no" => Some(false),
            _ => None,
        }
    })
}

/// Parse a string environment variable.
///
/// Returns `Some(value)` if set and non-empty, `None` otherwise.
pub fn env_string(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.is_empty())
}

/// Parse a usize environment variable.
///
/// Returns `Some(value)` if set and parseable, `None` otherwise.
pub fn env_usize(name: &str) -> Option<usize> {
    env_string(name).and_then(|v| v.parse().ok())
}

/// Check if a string environment variable is set and non-empty.
pub fn env_is_set(name: &str) -> bool {
    std::env::var(name)
        .map(|v| !v.is_empty())
        .unwrap_or(false)
}

// ============================================================================
// Config Loading
// ============================================================================

/// Load configuration from all sources.
///
/// Does not apply CLI flags (those are handled by args resolution).
/// Does not apply ENV vars (those are checked at point of use).
pub fn load_config(_git_root: &Path) -> Config {
    // TODO: Implement manifest loading in Phase 5
    // For now, return defaults
    Config::default()
}

/// Generate JSON schema for the config.
pub fn json_schema() -> String {
    let schema = schemars::schema_for!(Config);
    serde_json::to_string_pretty(&schema).unwrap_or_else(|_| "{}".to_string())
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // Mutex to serialize env var tests
    static ENV_MUTEX: Mutex<()> = Mutex::new(());

    fn with_env<F, R>(vars: &[(&str, Option<&str>)], f: F) -> R
    where
        F: FnOnce() -> R,
    {
        let _guard = ENV_MUTEX.lock().unwrap();

        let originals: Vec<_> = vars
            .iter()
            .map(|(k, _)| (*k, std::env::var(*k).ok()))
            .collect();

        for (k, v) in vars {
            match v {
                Some(val) => std::env::set_var(k, val),
                None => std::env::remove_var(k),
            }
        }

        let result = f();

        for (k, original) in originals {
            match original {
                Some(val) => std::env::set_var(k, val),
                None => std::env::remove_var(k),
            }
        }

        result
    }

    #[test]
    fn test_env_bool_truthy() {
        with_env(&[("TEST_BOOL", Some("1"))], || {
            assert_eq!(env_bool("TEST_BOOL"), Some(true));
        });
        with_env(&[("TEST_BOOL", Some("true"))], || {
            assert_eq!(env_bool("TEST_BOOL"), Some(true));
        });
        with_env(&[("TEST_BOOL", Some("yes"))], || {
            assert_eq!(env_bool("TEST_BOOL"), Some(true));
        });
        with_env(&[("TEST_BOOL", Some("TRUE"))], || {
            assert_eq!(env_bool("TEST_BOOL"), Some(true));
        });
    }

    #[test]
    fn test_env_bool_falsy() {
        with_env(&[("TEST_BOOL", Some("0"))], || {
            assert_eq!(env_bool("TEST_BOOL"), Some(false));
        });
        with_env(&[("TEST_BOOL", Some("false"))], || {
            assert_eq!(env_bool("TEST_BOOL"), Some(false));
        });
        with_env(&[("TEST_BOOL", Some("no"))], || {
            assert_eq!(env_bool("TEST_BOOL"), Some(false));
        });
    }

    #[test]
    fn test_env_bool_unset() {
        with_env(&[("TEST_BOOL", None)], || {
            assert_eq!(env_bool("TEST_BOOL"), None);
        });
        with_env(&[("TEST_BOOL", Some(""))], || {
            assert_eq!(env_bool("TEST_BOOL"), None);
        });
        with_env(&[("TEST_BOOL", Some("invalid"))], || {
            assert_eq!(env_bool("TEST_BOOL"), None);
        });
    }

    #[test]
    fn test_env_string() {
        with_env(&[("TEST_STR", Some("hello"))], || {
            assert_eq!(env_string("TEST_STR"), Some("hello".to_string()));
        });
        with_env(&[("TEST_STR", Some(""))], || {
            assert_eq!(env_string("TEST_STR"), None);
        });
        with_env(&[("TEST_STR", None)], || {
            assert_eq!(env_string("TEST_STR"), None);
        });
    }

    #[test]
    fn test_env_usize() {
        with_env(&[("TEST_NUM", Some("42"))], || {
            assert_eq!(env_usize("TEST_NUM"), Some(42));
        });
        with_env(&[("TEST_NUM", Some("0"))], || {
            assert_eq!(env_usize("TEST_NUM"), Some(0));
        });
        with_env(&[("TEST_NUM", Some("abc"))], || {
            assert_eq!(env_usize("TEST_NUM"), None);
        });
        with_env(&[("TEST_NUM", None)], || {
            assert_eq!(env_usize("TEST_NUM"), None);
        });
    }

    #[test]
    fn test_config_default() {
        let config = Config::default();
        assert_eq!(config.defaults.new, "idea");
        assert_eq!(config.defaults.closed, "resolved");
        assert_eq!(config.defaults.open, "active");
        assert!(config.status.open.contains(&"active".to_string()));
        assert!(config.status.closed.contains(&"resolved".to_string()));
    }

    #[test]
    fn test_json_schema_generates() {
        let schema = json_schema();
        assert!(schema.contains("Config"));
        assert!(schema.contains("StatusConfig"));
    }

    #[test]
    fn test_config_source_display() {
        assert_eq!(ConfigSource::Default.to_string(), "default");
        assert_eq!(
            ConfigSource::EnvVar("THREADS_FORMAT".to_string()).to_string(),
            "$THREADS_FORMAT"
        );
        assert_eq!(ConfigSource::CliFlag.to_string(), "CLI flag");
    }
}

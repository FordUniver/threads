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

use std::fs;
use std::path::{Path, PathBuf};

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
    std::env::var(name).map(|v| !v.is_empty()).unwrap_or(false)
}

// ============================================================================
// Config Loading
// ============================================================================

/// Manifest file name within .threads-config/
pub const MANIFEST_FILE: &str = "manifest.yaml";

/// Config directory name
pub const CONFIG_DIR: &str = ".threads-config";

/// Load configuration from all sources.
///
/// Resolution order (later overrides earlier):
/// 1. Built-in defaults
/// 2. User global (~/.config/threads/config.yaml)
/// 3. Project manifests (walk from git_root to cwd)
///
/// Does not apply CLI flags (those are handled by args resolution).
/// Does not apply ENV vars (those are checked at point of use).
pub fn load_config(git_root: &Path, cwd: &Path) -> LoadedConfig {
    let mut config = Config::default();
    let mut sources = vec![ConfigSource::Default];

    // 1. User global config
    if let Some(user_config_path) = user_config_path() {
        if let Some(user_config) = load_manifest(&user_config_path) {
            merge(&mut config, &user_config);
            sources.push(ConfigSource::UserGlobal);
        }
    }

    // 2. Walk from git root to cwd, loading manifests at each level
    let manifest_paths = collect_manifest_paths(git_root, cwd);
    for path in manifest_paths {
        if let Some(manifest_config) = load_manifest(&path) {
            let rel_path = path
                .strip_prefix(git_root)
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| path.to_string_lossy().to_string());
            merge(&mut config, &manifest_config);
            sources.push(ConfigSource::ProjectManifest(rel_path));
        }
    }

    LoadedConfig { config, sources }
}

/// Result of loading configuration with source tracking.
#[derive(Debug)]
pub struct LoadedConfig {
    /// The merged configuration
    pub config: Config,
    /// Sources that contributed to this config (in order of application)
    pub sources: Vec<ConfigSource>,
}

/// Get the user config file path (~/.config/threads/config.yaml).
pub fn user_config_path() -> Option<PathBuf> {
    dirs::config_dir().map(|p| p.join("threads").join("config.yaml"))
}

/// Load a manifest file, returning None if it doesn't exist or can't be parsed.
pub fn load_manifest(path: &Path) -> Option<Config> {
    let content = fs::read_to_string(path).ok()?;
    serde_yaml::from_str(&content).ok()
}

/// Collect manifest paths from git_root to cwd (inclusive).
///
/// Returns paths in order from root to cwd (so later ones override earlier).
fn collect_manifest_paths(git_root: &Path, cwd: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();

    // Normalize paths
    let git_root = git_root
        .canonicalize()
        .unwrap_or_else(|_| git_root.to_path_buf());
    let cwd = cwd.canonicalize().unwrap_or_else(|_| cwd.to_path_buf());

    // Check if cwd is under git_root
    if !cwd.starts_with(&git_root) {
        // Just check git_root
        let manifest = git_root.join(CONFIG_DIR).join(MANIFEST_FILE);
        if manifest.exists() {
            paths.push(manifest);
        }
        return paths;
    }

    // Walk from git_root to cwd
    let mut current = git_root.clone();
    let rel_path = cwd.strip_prefix(&git_root).unwrap_or(Path::new(""));

    // Check git_root itself
    let manifest = current.join(CONFIG_DIR).join(MANIFEST_FILE);
    if manifest.exists() {
        paths.push(manifest);
    }

    // Walk through each component of the relative path
    for component in rel_path.components() {
        current = current.join(component);
        let manifest = current.join(CONFIG_DIR).join(MANIFEST_FILE);
        if manifest.exists() {
            paths.push(manifest);
        }
    }

    paths
}

/// Merge overlay config into base config.
///
/// Non-default values in overlay override values in base.
/// For Vec fields, overlay replaces entirely (not appended).
pub fn merge(base: &mut Config, overlay: &Config) {
    // Status: replace if overlay has non-default values
    let default_status = StatusConfig::default();
    if overlay.status.open != default_status.open {
        base.status.open = overlay.status.open.clone();
    }
    if overlay.status.closed != default_status.closed {
        base.status.closed = overlay.status.closed.clone();
    }

    // Defaults: replace if overlay has non-default values
    let default_defaults = DefaultsConfig::default();
    if overlay.defaults.new != default_defaults.new {
        base.defaults.new = overlay.defaults.new.clone();
    }
    if overlay.defaults.closed != default_defaults.closed {
        base.defaults.closed = overlay.defaults.closed.clone();
    }
    if overlay.defaults.open != default_defaults.open {
        base.defaults.open = overlay.defaults.open.clone();
    }

    // Display: merge Option fields
    if overlay.display.root_name.is_some() {
        base.display.root_name = overlay.display.root_name.clone();
    }
    if let Some(ref overlay_colors) = overlay.display.status_colors {
        let base_colors = base
            .display
            .status_colors
            .get_or_insert_with(StatusColors::default);
        merge_status_colors(base_colors, overlay_colors);
    }

    // Behavior: merge non-default values
    let default_behavior = BehaviorConfig::default();
    if overlay.behavior.auto_commit != default_behavior.auto_commit {
        base.behavior.auto_commit = overlay.behavior.auto_commit;
    }
    if overlay.behavior.default_down.is_some() {
        base.behavior.default_down = overlay.behavior.default_down.clone();
    }
    if overlay.behavior.default_up.is_some() {
        base.behavior.default_up = overlay.behavior.default_up.clone();
    }
    if overlay.behavior.quiet != default_behavior.quiet {
        base.behavior.quiet = overlay.behavior.quiet;
    }

    // Sections: merge Option fields (None means disabled, Some means renamed)
    let default_sections = SectionsConfig::default();
    if overlay.sections.body != default_sections.body {
        base.sections.body = overlay.sections.body.clone();
    }
    if overlay.sections.notes != default_sections.notes {
        base.sections.notes = overlay.sections.notes.clone();
    }
    if overlay.sections.todo != default_sections.todo {
        base.sections.todo = overlay.sections.todo.clone();
    }
    if overlay.sections.log != default_sections.log {
        base.sections.log = overlay.sections.log.clone();
    }
}

/// Merge status colors (overlay wins for non-None values).
fn merge_status_colors(base: &mut StatusColors, overlay: &StatusColors) {
    if overlay.active.is_some() {
        base.active = overlay.active.clone();
    }
    if overlay.blocked.is_some() {
        base.blocked = overlay.blocked.clone();
    }
    if overlay.paused.is_some() {
        base.paused = overlay.paused.clone();
    }
    if overlay.idea.is_some() {
        base.idea = overlay.idea.clone();
    }
    if overlay.planning.is_some() {
        base.planning = overlay.planning.clone();
    }
    if overlay.resolved.is_some() {
        base.resolved = overlay.resolved.clone();
    }
    if overlay.superseded.is_some() {
        base.superseded = overlay.superseded.clone();
    }
    if overlay.deferred.is_some() {
        base.deferred = overlay.deferred.clone();
    }
    if overlay.rejected.is_some() {
        base.rejected = overlay.rejected.clone();
    }
}

/// Generate JSON schema for the config.
pub fn json_schema() -> String {
    let schema = schemars::schema_for!(Config);
    serde_json::to_string_pretty(&schema).unwrap_or_else(|_| "{}".to_string())
}

/// Get the list of valid section names from config.
///
/// Returns section names that are enabled (Some value, not None).
pub fn valid_section_names(sections: &SectionsConfig) -> Vec<&str> {
    let mut names = Vec::new();
    if let Some(ref name) = sections.body {
        names.push(name.as_str());
    }
    if let Some(ref name) = sections.notes {
        names.push(name.as_str());
    }
    if let Some(ref name) = sections.todo {
        names.push(name.as_str());
    }
    if let Some(ref name) = sections.log {
        names.push(name.as_str());
    }
    names
}

/// Resolve a canonical section name to the configured name.
///
/// For example, if sections.todo is "Tasks", resolving "Todo" returns "Tasks".
/// Returns None if the section is disabled (set to null in config).
pub fn resolve_section_name<'a>(sections: &'a SectionsConfig, canonical: &str) -> Option<&'a str> {
    match canonical {
        "Body" => sections.body.as_deref(),
        "Notes" => sections.notes.as_deref(),
        "Todo" => sections.todo.as_deref(),
        "Log" => sections.log.as_deref(),
        _ => None,
    }
}

/// Check if quiet mode is enabled (suppress hints).
///
/// Checks both config.behavior.quiet and THREADS_QUIET env var.
pub fn is_quiet(config: &Config) -> bool {
    config.behavior.quiet || env_bool("THREADS_QUIET").unwrap_or(false)
}

/// Get the display name for the repo root.
///
/// Returns config.display.root_name if set, otherwise "repo root".
pub fn root_name(config: &Config) -> &str {
    config.display.root_name.as_deref().unwrap_or("repo root")
}

/// Generate a template manifest with comments.
pub fn template_manifest() -> String {
    r#"# threads configuration manifest
# Place in .threads-config/manifest.yaml

# Status definitions (uncomment to customize)
# status:
#   open: [idea, planning, active, blocked, paused]
#   closed: [resolved, superseded, deferred, rejected]

# Default values
# defaults:
#   new: idea           # threads new
#   closed: resolved    # threads close
#   open: active        # threads reopen

# Display settings
# display:
#   root_name: null     # Custom name for repo root (null = "repo root")
#   status_colors:
#     active: green
#     blocked: yellow
#     paused: yellow
#     idea: blue
#     planning: blue
#     resolved: dim
#     superseded: dim
#     deferred: dim
#     rejected: dim

# Behavior settings
# behavior:
#   auto_commit: false
#   default_down: null  # null = disabled, number = depth, "unlimited" = no limit
#   default_up: null
#   quiet: false

# Section names (null to disable, string to rename)
# sections:
#   Body: Body
#   Notes: Notes
#   Todo: Todo
#   Log: Log
"#
    .to_string()
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

    #[test]
    fn test_merge_defaults_preserved() {
        let mut base = Config::default();
        let overlay = Config::default();
        merge(&mut base, &overlay);

        // Should still have defaults
        assert_eq!(base.defaults.new, "idea");
        assert_eq!(base.defaults.closed, "resolved");
    }

    #[test]
    fn test_merge_overlay_wins() {
        let mut base = Config::default();
        let mut overlay = Config::default();
        overlay.defaults.new = "planning".to_string();

        merge(&mut base, &overlay);

        assert_eq!(base.defaults.new, "planning");
        // Other defaults unchanged
        assert_eq!(base.defaults.closed, "resolved");
    }

    #[test]
    fn test_merge_status_lists() {
        let mut base = Config::default();
        let mut overlay = Config::default();
        overlay.status.open = vec!["custom".to_string(), "statuses".to_string()];

        merge(&mut base, &overlay);

        assert_eq!(overlay.status.open, vec!["custom", "statuses"]);
        // Closed unchanged
        assert!(base.status.closed.contains(&"resolved".to_string()));
    }

    #[test]
    fn test_template_manifest() {
        let template = template_manifest();
        assert!(template.contains("# threads configuration manifest"));
        assert!(template.contains("status:"));
        assert!(template.contains("defaults:"));
    }
}

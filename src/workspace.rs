use std::collections::HashSet;
use std::env;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use clap_complete::engine::CompletionCandidate;
use git2::Repository;
use regex::Regex;

use crate::thread;

// Cached regexes for workspace operations
static ID_ONLY_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[0-9a-f]{6}$").unwrap());

/// Options for finding threads with direction controls.
#[derive(Debug, Clone, Default)]
pub struct FindOptions {
    /// Search subdirectories. None = no recursion, Some(None) = unlimited, Some(Some(n)) = n levels
    pub down: Option<Option<usize>>,
    /// Search parent directories. None = no up search, Some(None) = to git root, Some(Some(n)) = n levels
    pub up: Option<Option<usize>>,
}

impl FindOptions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_down(mut self, depth: Option<usize>) -> Self {
        self.down = Some(depth);
        self
    }

    pub fn with_up(mut self, depth: Option<usize>) -> Self {
        self.up = Some(depth);
        self
    }
}

static SLUGIFY_NON_ALNUM_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[^a-z0-9]+").unwrap());

static SLUGIFY_MULTI_DASH_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"-+").unwrap());

/// Open the git repository from current directory.
/// Returns an error if not in a git repository.
pub fn open() -> Result<Repository, String> {
    Repository::discover(".").map_err(|e| {
        if e.code() == git2::ErrorCode::NotFound {
            "Not in a git repository. threads requires a git repo to define scope.".to_string()
        } else {
            format!("Failed to open git repository: {}", e.message())
        }
    })
}

/// Get the git root (working directory) from an opened repository.
pub fn git_root(repo: &Repository) -> PathBuf {
    repo.workdir()
        .expect("Repository should have a working directory")
        .to_path_buf()
}

/// Find the git repository root from current directory.
/// Returns an error if not in a git repository.
pub fn find() -> Result<PathBuf, String> {
    find_git_root()
}

/// Find the git repository root using git2.
pub fn find_git_root() -> Result<PathBuf, String> {
    let repo = open()?;
    Ok(git_root(&repo))
}

/// Check if a directory is a git root (contains .git).
pub fn is_git_root(path: &Path) -> bool {
    path.join(".git").exists()
}

/// Find all thread file paths within the git root.
/// Scans recursively, respecting git boundaries (stops at nested git repos).
pub fn find_all_threads(git_root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut threads = Vec::new();
    find_threads_recursive(git_root, git_root, &mut threads)?;
    threads.sort();
    Ok(threads)
}

/// Recursively find .threads directories and collect thread files.
/// Stops at nested git repositories (directories containing .git).
fn find_threads_recursive(
    dir: &Path,
    git_root: &Path,
    threads: &mut Vec<PathBuf>,
) -> Result<(), String> {
    // Check for .threads directory here
    let threads_dir = dir.join(".threads");
    if threads_dir.is_dir() {
        if let Ok(entries) = fs::read_dir(&threads_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().is_some_and(|e| e == "md") {
                    // Skip archive subdirectory
                    if !path.to_string_lossy().contains("/archive/") {
                        threads.push(path);
                    }
                }
            }
        }
    }

    // Recurse into subdirectories
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            let name = entry.file_name();
            let name_str = name.to_string_lossy();

            // Skip hidden directories (except we already handled .threads)
            if name_str.starts_with('.') {
                continue;
            }

            // Stop at nested git repos (unless it's the root itself)
            if path != git_root && is_git_root(&path) {
                continue;
            }

            find_threads_recursive(&path, git_root, threads)?;
        }
    }

    Ok(())
}

/// Find threads with options for direction controls.
/// This is the primary search function supporting --up and --down flags.
/// Traversal always stops at git boundaries (nested repos when going down, git root when going up).
pub fn find_threads_with_options(
    start_path: &Path,
    git_root: &Path,
    options: &FindOptions,
) -> Result<Vec<PathBuf>, String> {
    let mut threads = Vec::new();
    let start_canonical = start_path
        .canonicalize()
        .unwrap_or_else(|_| start_path.to_path_buf());

    // Always collect threads at start_path
    collect_threads_at_path(&start_canonical, &mut threads);

    // Search down (subdirectories) - stops at nested git repos
    if let Some(max_depth) = options.down {
        find_threads_down(&start_canonical, git_root, &mut threads, 0, max_depth)?;
    }

    // Search up (parent directories) - stops at git root
    if let Some(max_depth) = options.up {
        find_threads_up(&start_canonical, git_root, &mut threads, 0, max_depth)?;
    }

    threads.sort();
    threads.dedup();
    Ok(threads)
}

/// Collect threads from .threads directory at the given path.
fn collect_threads_at_path(dir: &Path, threads: &mut Vec<PathBuf>) {
    let threads_dir = dir.join(".threads");
    if threads_dir.is_dir() {
        if let Ok(entries) = fs::read_dir(&threads_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().is_some_and(|e| e == "md") {
                    // Skip archive subdirectory
                    if !path.to_string_lossy().contains("/archive/") {
                        threads.push(path);
                    }
                }
            }
        }
    }
}

/// Recursively find threads going down into subdirectories.
/// Always stops at nested git repositories.
fn find_threads_down(
    dir: &Path,
    git_root: &Path,
    threads: &mut Vec<PathBuf>,
    current_depth: usize,
    max_depth: Option<usize>,
) -> Result<(), String> {
    // Check depth limit (None or Some(0) means unlimited, matching Go's convention)
    if let Some(max) = max_depth {
        if max > 0 && current_depth >= max {
            return Ok(());
        }
    }

    // Recurse into subdirectories
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            let name = entry.file_name();
            let name_str = name.to_string_lossy();

            // Skip hidden directories
            if name_str.starts_with('.') {
                continue;
            }

            // Stop at nested git repos
            if path != git_root && is_git_root(&path) {
                continue;
            }

            // Collect threads at this level
            collect_threads_at_path(&path, threads);

            // Continue recursing
            find_threads_down(&path, git_root, threads, current_depth + 1, max_depth)?;
        }
    }

    Ok(())
}

/// Find threads going up into parent directories.
/// Always stops at the git root boundary.
fn find_threads_up(
    dir: &Path,
    git_root: &Path,
    threads: &mut Vec<PathBuf>,
    current_depth: usize,
    max_depth: Option<usize>,
) -> Result<(), String> {
    // Check depth limit (None or Some(0) means unlimited, matching Go's convention)
    if let Some(max) = max_depth {
        if max > 0 && current_depth >= max {
            return Ok(());
        }
    }

    let Some(parent) = dir.parent() else {
        return Ok(());
    };

    let parent_canonical = parent
        .canonicalize()
        .unwrap_or_else(|_| parent.to_path_buf());
    let git_root_canonical = git_root
        .canonicalize()
        .unwrap_or_else(|_| git_root.to_path_buf());

    // Stop at git root
    if !parent_canonical.starts_with(&git_root_canonical) {
        return Ok(());
    }

    // Collect threads at parent
    collect_threads_at_path(&parent_canonical, threads);

    // Continue up
    find_threads_up(
        &parent_canonical,
        git_root,
        threads,
        current_depth + 1,
        max_depth,
    )
}

/// Scope represents thread placement information.
/// Path is relative to git root.
#[derive(Debug, Clone)]
pub struct Scope {
    /// Path to the .threads directory (absolute)
    pub threads_dir: PathBuf,
    /// Path relative to git root (e.g., "src/models", "." for root)
    pub path: String,
    /// Human-readable description
    pub level_desc: String,
}

/// Infer the threads directory and scope from a path specification.
///
/// Path resolution rules:
/// - None or empty: PWD
/// - ".": PWD (explicit)
/// - "./X/Y": PWD-relative
/// - "/X/Y": Absolute
/// - "X/Y" (no leading ./ or /): Git-root-relative
pub fn infer_scope(git_root: &Path, path_arg: Option<&str>) -> Result<Scope, String> {
    let pwd = env::current_dir().map_err(|e| format!("Cannot get current directory: {}", e))?;

    let (target_path, _resolution_desc) = match path_arg {
        None | Some("") => {
            // No path argument: use PWD
            (pwd.clone(), "pwd")
        }
        Some(".") => {
            // Explicit ".": use PWD
            (pwd.clone(), "pwd (explicit)")
        }
        Some(p) if p.starts_with("./") => {
            // PWD-relative path: ./X/Y
            let rel = p.strip_prefix("./").unwrap();
            (pwd.join(rel), "pwd-relative")
        }
        Some(p) if p.starts_with('/') => {
            // Absolute path
            (PathBuf::from(p), "absolute")
        }
        Some(p) => {
            // Git-root-relative path: X/Y
            (git_root.join(p), "git-root-relative")
        }
    };

    // Canonicalize for consistent comparison
    let target_canonical = target_path
        .canonicalize()
        .unwrap_or_else(|_| target_path.clone());

    let git_root_canonical = git_root
        .canonicalize()
        .unwrap_or_else(|_| git_root.to_path_buf());

    // Verify target is within the git repo
    if !target_canonical.starts_with(&git_root_canonical) {
        return Err(format!(
            "Path must be within git repository: {} (git root: {})",
            target_path.display(),
            git_root.display()
        ));
    }

    // Check if target is inside a nested git repo
    if target_canonical != git_root_canonical {
        // Walk up from target to git_root, checking for nested .git
        let mut check_path = target_canonical.clone();
        while check_path != git_root_canonical {
            if is_git_root(&check_path) {
                return Err(format!(
                    "Path is inside a nested git repository at: {}",
                    check_path.display()
                ));
            }
            if let Some(parent) = check_path.parent() {
                check_path = parent.to_path_buf();
            } else {
                break;
            }
        }
    }

    // Compute path relative to git root
    let rel_path = target_canonical
        .strip_prefix(&git_root_canonical)
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| ".".to_string());

    let rel_path = if rel_path.is_empty() {
        ".".to_string()
    } else {
        rel_path
    };

    // Build description
    let level_desc = if rel_path == "." {
        "repo root".to_string()
    } else {
        rel_path.clone()
    };

    // Build threads directory path
    let threads_dir = target_canonical.join(".threads");

    Ok(Scope {
        threads_dir,
        path: rel_path,
        level_desc,
    })
}

/// Parse thread path to extract the git-relative path component.
/// Returns the path relative to git root (e.g., "src/models").
pub fn parse_thread_path(git_root: &Path, thread_path: &Path) -> String {
    let git_root_canonical = git_root
        .canonicalize()
        .unwrap_or_else(|_| git_root.to_path_buf());
    let path_canonical = thread_path
        .canonicalize()
        .unwrap_or_else(|_| thread_path.to_path_buf());

    // Get path relative to git root
    let rel = if path_canonical.starts_with(&git_root_canonical) {
        path_canonical
            .strip_prefix(&git_root_canonical)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| thread_path.to_string_lossy().to_string())
    } else {
        thread_path.to_string_lossy().to_string()
    };

    // Extract the directory containing .threads
    // Pattern: <path>/.threads/file.md -> return <path>
    if let Some(idx) = rel.rfind("/.threads/") {
        let path = &rel[..idx];
        if path.is_empty() {
            ".".to_string()
        } else {
            path.to_string()
        }
    } else {
        // Includes .threads/ at root or any other case
        ".".to_string()
    }
}

/// Get path relative to git root for display purposes.
pub fn path_relative_to_git_root(git_root: &Path, path: &Path) -> String {
    let git_root_canonical = git_root
        .canonicalize()
        .unwrap_or_else(|_| git_root.to_path_buf());
    let path_canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());

    if path_canonical.starts_with(&git_root_canonical) {
        let rel = path_canonical
            .strip_prefix(&git_root_canonical)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();
        if rel.is_empty() {
            ".".to_string()
        } else {
            rel
        }
    } else {
        path.to_string_lossy().to_string()
    }
}

/// Get the git-relative path for the current working directory.
pub fn pwd_relative_to_git_root(git_root: &Path) -> Result<String, String> {
    let pwd = env::current_dir().map_err(|e| format!("Cannot get current directory: {}", e))?;
    Ok(path_relative_to_git_root(git_root, &pwd))
}

/// Generate a unique 6-character hex ID.
pub fn generate_id(git_root: &Path) -> Result<String, String> {
    let threads = find_all_threads(git_root)?;
    let mut existing = HashSet::new();

    for t in threads {
        if let Some(id) = thread::extract_id_from_path(&t) {
            existing.insert(id);
        }
    }

    for _ in 0..10 {
        let mut bytes = [0u8; 3];
        getrandom::getrandom(&mut bytes).map_err(|e| format!("random generation failed: {}", e))?;
        let id = hex::encode(bytes);
        if !existing.contains(&id) {
            return Ok(id);
        }
    }

    Err("could not generate unique ID after 10 attempts".to_string())
}

/// Convert a title to kebab-case filename.
pub fn slugify(title: &str) -> String {
    let s = title.to_lowercase();
    let s = SLUGIFY_NON_ALNUM_RE.replace_all(&s, "-");
    let s = SLUGIFY_MULTI_DASH_RE.replace_all(&s, "-");
    s.trim_matches('-').to_string()
}

/// Find a thread by ID or name (with fuzzy matching).
pub fn find_by_ref(git_root: &Path, ref_str: &str) -> Result<PathBuf, String> {
    let threads = find_all_threads(git_root)?;

    // Fast path: exact ID match
    if ID_ONLY_RE.is_match(ref_str) {
        for t in &threads {
            if thread::extract_id_from_path(t).as_deref() == Some(ref_str) {
                return Ok(t.clone());
            }
        }
    }

    // Slow path: name matching
    let ref_lower = ref_str.to_lowercase();
    let mut substring_matches = Vec::new();

    for t in &threads {
        let name = thread::extract_name_from_path(t);

        // Exact name match
        if name == ref_str {
            return Ok(t.clone());
        }

        // Substring match (case-insensitive)
        if name.to_lowercase().contains(&ref_lower) {
            substring_matches.push(t.clone());
        }
    }

    if substring_matches.len() == 1 {
        return Ok(substring_matches.into_iter().next().unwrap());
    }

    if substring_matches.len() > 1 {
        let ids: Vec<String> = substring_matches
            .iter()
            .map(|m| {
                let id = thread::extract_id_from_path(m).unwrap_or_else(|| "???".to_string());
                let name = thread::extract_name_from_path(m);
                format!("{} ({})", id, name)
            })
            .collect();
        return Err(format!(
            "ambiguous reference '{}' matches {} threads: {}",
            ref_str,
            substring_matches.len(),
            ids.join(", ")
        ));
    }

    Err(format!("thread not found: {}", ref_str))
}

// Helper for hex encoding
mod hex {
    use std::fmt::Write;

    pub fn encode(bytes: [u8; 3]) -> String {
        bytes.iter().fold(String::with_capacity(6), |mut acc, b| {
            let _ = write!(acc, "{:02x}", b);
            acc
        })
    }
}

/// Completer for thread IDs - returns all thread IDs with names as descriptions.
pub fn complete_thread_ids(_current: &OsStr) -> Vec<CompletionCandidate> {
    let git_root = match find() {
        Ok(root) => root,
        Err(_) => return vec![],
    };

    let threads = match find_all_threads(&git_root) {
        Ok(t) => t,
        Err(_) => return vec![],
    };

    threads
        .iter()
        .filter_map(|path| {
            let id = thread::extract_id_from_path(path)?;
            let name = thread::extract_name_from_path(path);
            Some(CompletionCandidate::new(id).help(Some(name.into())))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slugify() {
        let cases = vec![
            ("Hello World", "hello-world"),
            ("My Feature Request", "my-feature-request"),
            ("Fix: bug in parser", "fix-bug-in-parser"),
            ("Remove   extra   spaces", "remove-extra-spaces"),
            ("Trailing hyphens---", "trailing-hyphens"),
            ("---Leading hyphens", "leading-hyphens"),
            ("Special!@#$%chars", "special-chars"),
            ("MixedCASE", "mixedcase"),
            ("already-kebab-case", "already-kebab-case"),
            ("123 numbers first", "123-numbers-first"),
        ];

        for (title, want) in cases {
            let got = slugify(title);
            assert_eq!(
                got, want,
                "slugify({:?}) = {:?}, want {:?}",
                title, got, want
            );
        }
    }
}

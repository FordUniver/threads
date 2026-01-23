//! Thread file timestamp cache.
//!
//! Caches git commit dates (created/modified) for thread files to avoid
//! expensive history walks on every `threads list` invocation.
//!
//! Cache lives in `.threads-config/cache.json` at git root.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use git2::Repository;
use serde::{Deserialize, Serialize};

/// Cached timestamp info for a single file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileTimestamps {
    /// Timestamp of first commit touching this file (seconds since epoch)
    pub created: i64,
    /// Commit hash of first commit
    pub created_commit: String,
    /// Timestamp of last commit touching this file
    pub modified: i64,
    /// Commit hash of last commit
    pub modified_commit: String,
}

/// The full cache structure.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TimestampCache {
    /// HEAD commit hash when cache was last updated
    pub head: String,
    /// Map of relative file path -> timestamps
    pub files: HashMap<String, FileTimestamps>,
}

impl TimestampCache {
    /// Load cache from disk, or return empty cache if not found/invalid.
    pub fn load(git_root: &Path) -> Self {
        let cache_path = Self::cache_path(git_root);
        match fs::read_to_string(&cache_path) {
            Ok(contents) => serde_json::from_str(&contents).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    /// Save cache to disk.
    pub fn save(&self, git_root: &Path) -> Result<(), String> {
        let cache_path = Self::cache_path(git_root);

        // Ensure .threads-config directory exists
        if let Some(parent) = cache_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create .threads-config: {}", e))?;
        }

        let contents = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Failed to serialize cache: {}", e))?;

        fs::write(&cache_path, contents).map_err(|e| format!("Failed to write cache: {}", e))?;

        Ok(())
    }

    /// Get the cache file path.
    fn cache_path(git_root: &Path) -> PathBuf {
        git_root.join(".threads-config").join("cache.json")
    }

    /// Get timestamps for a file, if cached.
    pub fn get(&self, rel_path: &str) -> Option<&FileTimestamps> {
        self.files.get(rel_path)
    }

    /// Check if cache is valid for current HEAD.
    pub fn is_current(&self, repo: &Repository) -> bool {
        match Self::current_head(repo) {
            Some(head) => self.head == head,
            None => false,
        }
    }

    /// Check if cached HEAD is an ancestor of current HEAD (incremental update possible).
    pub fn is_ancestor_of_head(&self, repo: &Repository) -> bool {
        if self.head.is_empty() {
            return false;
        }

        let current = match Self::current_head(repo) {
            Some(h) => h,
            None => return false,
        };

        if self.head == current {
            return true; // Same commit
        }

        // Check if cached head is ancestor of current head
        let cached_oid = match git2::Oid::from_str(&self.head) {
            Ok(o) => o,
            Err(_) => return false,
        };

        let current_oid = match git2::Oid::from_str(&current) {
            Ok(o) => o,
            Err(_) => return false,
        };

        repo.graph_descendant_of(current_oid, cached_oid)
            .unwrap_or(false)
    }

    /// Get current HEAD commit hash.
    pub fn current_head(repo: &Repository) -> Option<String> {
        repo.head()
            .ok()
            .and_then(|r| r.peel_to_commit().ok())
            .map(|c| c.id().to_string())
    }

    /// Update cache with timestamps for given files.
    /// This does an incremental update if possible, full rebuild otherwise.
    pub fn update(&mut self, repo: &Repository, thread_files: &[PathBuf], git_root: &Path) {
        let current_head = match Self::current_head(repo) {
            Some(h) => h,
            None => return,
        };

        if self.is_current(repo) {
            // Cache is current, just ensure all requested files are present
            self.ensure_files(repo, thread_files, git_root);
        } else if self.is_ancestor_of_head(repo) {
            // Incremental update: walk from cached head to current head
            self.incremental_update(repo, &current_head, thread_files, git_root);
        } else {
            // Full rebuild needed (branch switch, rebase, etc.)
            self.full_rebuild(repo, &current_head, thread_files, git_root);
        }

        self.head = current_head;
    }

    /// Ensure all requested files have cache entries (for files not yet in cache).
    fn ensure_files(&mut self, repo: &Repository, thread_files: &[PathBuf], git_root: &Path) {
        let missing: Vec<_> = thread_files
            .iter()
            .filter_map(|p| {
                let rel = p.strip_prefix(git_root).ok()?;
                let rel_str = rel.to_string_lossy();
                if self.files.contains_key(rel_str.as_ref()) {
                    None
                } else {
                    Some(rel.to_path_buf())
                }
            })
            .collect();

        if !missing.is_empty() {
            self.populate_files(repo, &missing, git_root);
        }
    }

    /// Incremental update: walk commits from cached HEAD to current HEAD.
    fn incremental_update(
        &mut self,
        repo: &Repository,
        current_head: &str,
        thread_files: &[PathBuf],
        git_root: &Path,
    ) {
        let cached_oid = match git2::Oid::from_str(&self.head) {
            Ok(o) => o,
            Err(_) => {
                self.full_rebuild(repo, current_head, thread_files, git_root);
                return;
            }
        };

        let current_oid = match git2::Oid::from_str(current_head) {
            Ok(o) => o,
            Err(_) => return,
        };

        let mut revwalk = match repo.revwalk() {
            Ok(r) => r,
            Err(_) => return,
        };

        if revwalk.push(current_oid).is_err() {
            return;
        }
        if revwalk.hide(cached_oid).is_err() {
            return;
        }

        // Walk new commits and update modified times for touched files
        for oid in revwalk.flatten() {
            let commit = match repo.find_commit(oid) {
                Ok(c) => c,
                Err(_) => continue,
            };

            let tree = match commit.tree() {
                Ok(t) => t,
                Err(_) => continue,
            };

            let parent_tree = commit.parent(0).ok().and_then(|p| p.tree().ok());

            let diff = match repo.diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), None) {
                Ok(d) => d,
                Err(_) => continue,
            };

            let commit_time = commit.time().seconds();
            let commit_hash = oid.to_string();

            for delta in diff.deltas() {
                for path in [delta.new_file().path(), delta.old_file().path()]
                    .into_iter()
                    .flatten()
                {
                    let path_str = path.to_string_lossy();
                    if let Some(entry) = self.files.get_mut(path_str.as_ref()) {
                        // Update modified time (this commit is newer than cached)
                        entry.modified = commit_time;
                        entry.modified_commit = commit_hash.clone();
                    }
                }
            }
        }

        // Ensure all requested files are in cache
        self.ensure_files(repo, thread_files, git_root);
    }

    /// Full rebuild: walk entire history for given files.
    fn full_rebuild(
        &mut self,
        repo: &Repository,
        current_head: &str,
        thread_files: &[PathBuf],
        git_root: &Path,
    ) {
        self.files.clear();
        self.head = current_head.to_string();

        let rel_paths: Vec<_> = thread_files
            .iter()
            .filter_map(|p| p.strip_prefix(git_root).ok().map(|r| r.to_path_buf()))
            .collect();

        self.populate_files(repo, &rel_paths, git_root);
    }

    /// Populate cache entries for specific files using git CLI with --follow.
    /// This tracks renames to find the true initial commit.
    fn populate_files(&mut self, _repo: &Repository, rel_paths: &[PathBuf], git_root: &Path) {
        use std::process::Command;

        if rel_paths.is_empty() {
            return;
        }

        for rel_path in rel_paths {
            let path_str = rel_path.to_string_lossy().to_string();

            // Use git log --follow to get all commits (tracks renames)
            // Format: timestamp:hash per line, newest first
            let output = Command::new("git")
                .args([
                    "-C",
                    &git_root.to_string_lossy(),
                    "log",
                    "--follow",
                    "--format=%ct:%H",
                    "--",
                    &path_str,
                ])
                .output();

            let output = match output {
                Ok(o) if o.status.success() => o,
                _ => continue,
            };

            let stdout = String::from_utf8_lossy(&output.stdout);
            let lines: Vec<&str> = stdout.lines().collect();

            if lines.is_empty() {
                continue; // File has no git history (uncommitted)
            }

            // First line = most recent commit (modified)
            // Last line = initial commit (created)
            let parse_line = |line: &str| -> Option<(i64, String)> {
                let parts: Vec<&str> = line.splitn(2, ':').collect();
                if parts.len() == 2 {
                    let ts = parts[0].parse::<i64>().ok()?;
                    Some((ts, parts[1].to_string()))
                } else {
                    None
                }
            };

            let modified = parse_line(lines[0]);
            let created = parse_line(lines[lines.len() - 1]);

            if let (Some((mod_ts, mod_hash)), Some((cre_ts, cre_hash))) = (modified, created) {
                self.files.insert(
                    path_str,
                    FileTimestamps {
                        created: cre_ts,
                        created_commit: cre_hash,
                        modified: mod_ts,
                        modified_commit: mod_hash,
                    },
                );
            }
        }
    }
}

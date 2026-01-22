use std::path::Path;

use git2::{Repository, Status, StatusOptions};

/// Check if a file has uncommitted changes (staged, unstaged, or untracked)
pub fn has_changes(repo: &Repository, rel_path: &Path) -> bool {
    // Use status_file for direct file status check
    match repo.status_file(rel_path) {
        Ok(status) => {
            // Any status except CURRENT (tracked and unchanged) means changes
            !status.is_empty() && status != Status::CURRENT
        }
        Err(e) => {
            // NotFound means the file doesn't exist or is truly untracked
            // Check if the file exists on disk - if it does, it's an untracked file
            if e.code() == git2::ErrorCode::NotFound {
                let workdir = repo.workdir().unwrap_or(Path::new("."));
                let full_path = workdir.join(rel_path);
                full_path.exists()
            } else {
                // On other errors, assume changes exist to be safe
                true
            }
        }
    }
}

/// Check if a file is tracked by git
pub fn is_tracked(repo: &Repository, rel_path: &Path) -> bool {
    match repo.index() {
        Ok(index) => index.get_path(rel_path, 0).is_some(),
        Err(_) => false,
    }
}

/// Check if a file exists in HEAD
pub fn exists_in_head(repo: &Repository, rel_path: &Path) -> bool {
    let head = match repo.head() {
        Ok(h) => h,
        Err(_) => return false,
    };

    let tree = match head.peel_to_tree() {
        Ok(t) => t,
        Err(_) => return false,
    };

    tree.get_path(rel_path).is_ok()
}

/// Stage files (skips non-existent for deletions, handles both add and remove)
pub fn add(repo: &Repository, files: &[&Path]) -> Result<(), String> {
    let mut index = repo
        .index()
        .map_err(|e| format!("Failed to get index: {}", e.message()))?;

    let workdir = repo
        .workdir()
        .ok_or("Repository has no working directory")?;

    for file in files {
        let full_path = workdir.join(file);
        if full_path.exists() {
            index
                .add_path(file)
                .map_err(|e| format!("Failed to stage {}: {}", file.display(), e.message()))?;
        } else {
            // File doesn't exist - might be a deletion, try to remove from index
            let _ = index.remove_path(file);
        }
    }

    index
        .write()
        .map_err(|e| format!("Failed to write index: {}", e.message()))?;

    Ok(())
}

/// Create a commit containing only the specified files.
/// This is equivalent to `git commit -- <files>`: it commits only the listed files
/// while leaving other staged changes in the index untouched.
pub fn commit(repo: &Repository, files: &[&Path], message: &str) -> Result<(), String> {
    let workdir = repo
        .workdir()
        .ok_or("Repository has no working directory")?;

    // Get signature
    let sig = repo
        .signature()
        .map_err(|e| format!("Failed to get signature: {}", e.message()))?;

    // Get the HEAD tree (if any) as base for our new tree
    let head_tree = match repo.head() {
        Ok(head) => Some(
            head.peel_to_tree()
                .map_err(|e| format!("Failed to get HEAD tree: {}", e.message()))?,
        ),
        Err(_) => None, // Initial commit - no base tree
    };

    // Save the current index state
    let original_index = repo
        .index()
        .map_err(|e| format!("Failed to get index: {}", e.message()))?;
    let original_entries: Vec<_> = original_index.iter().collect();

    // Get the repo's index and modify it temporarily
    let mut index = repo
        .index()
        .map_err(|e| format!("Failed to get index: {}", e.message()))?;

    // Clear the index and start from HEAD tree
    index.clear().map_err(|e| format!("Failed to clear index: {}", e.message()))?;
    if let Some(ref tree) = head_tree {
        index.read_tree(tree)
            .map_err(|e| format!("Failed to read HEAD tree: {}", e.message()))?;
    }

    // Add only our specified files to the index
    for file in files {
        let full_path = workdir.join(file);
        if full_path.exists() {
            index
                .add_path(file)
                .map_err(|e| format!("Failed to add {}: {}", file.display(), e.message()))?;
        } else {
            // File was deleted - remove from index
            let _ = index.remove_path(file);
        }
    }

    // Write the index as a tree
    let tree_id = index
        .write_tree()
        .map_err(|e| format!("Failed to write tree: {}", e.message()))?;

    let tree = repo
        .find_tree(tree_id)
        .map_err(|e| format!("Failed to find tree: {}", e.message()))?;

    // Get parent commit (if any)
    let parent_commit = match repo.head() {
        Ok(head) => Some(
            head.peel_to_commit()
                .map_err(|e| format!("Failed to get HEAD commit: {}", e.message()))?,
        ),
        Err(_) => None, // Initial commit
    };

    let parents: Vec<&git2::Commit> = parent_commit.iter().collect();

    // Create the commit
    repo.commit(Some("HEAD"), &sig, &sig, message, &tree, &parents)
        .map_err(|e| format!("Failed to create commit: {}", e.message()))?;

    // Restore the original index entries that weren't part of this commit
    // First, read the new HEAD tree into index
    let new_head_tree = repo
        .head()
        .and_then(|h| h.peel_to_tree())
        .map_err(|e| format!("Failed to get new HEAD tree: {}", e.message()))?;

    index.read_tree(&new_head_tree)
        .map_err(|e| format!("Failed to read new HEAD tree: {}", e.message()))?;

    // Re-add the original staged entries that weren't in our commit
    let committed_paths: std::collections::HashSet<_> = files.iter().collect();
    for entry in original_entries {
        let entry_path = std::path::Path::new(std::str::from_utf8(&entry.path).unwrap_or(""));
        if !committed_paths.contains(&entry_path) {
            // This entry wasn't part of our commit - restore it
            index.add(&entry).map_err(|e| format!("Failed to restore staged entry: {}", e.message()))?;
        }
    }

    index.write().map_err(|e| format!("Failed to write index: {}", e.message()))?;

    Ok(())
}

/// Auto-commit: stage and commit (push is opt-in via separate command)
pub fn auto_commit(repo: &Repository, file: &Path, message: &str) -> Result<(), String> {
    let workdir = repo
        .workdir()
        .ok_or("Repository has no working directory")?;

    let rel_path = file
        .strip_prefix(workdir)
        .unwrap_or(file);

    commit(repo, &[rel_path], message)
}

/// Generate a commit message for thread changes
pub fn generate_commit_message(repo: &Repository, files: &[&Path]) -> String {
    let mut added = Vec::new();
    let mut modified = Vec::new();
    let mut deleted = Vec::new();

    let workdir = repo.workdir().unwrap_or(Path::new("."));

    for file in files {
        let full_path = workdir.join(file);

        let name = file
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_default();
        let name = name.trim_end_matches(".md").to_string();

        if exists_in_head(repo, file) {
            if full_path.exists() {
                modified.push(name);
            } else {
                deleted.push(name);
            }
        } else {
            added.push(name);
        }
    }

    let total = added.len() + modified.len() + deleted.len();

    if total == 1 {
        if added.len() == 1 {
            return format!("threads: add {}", extract_id(&added[0]));
        }
        if modified.len() == 1 {
            return format!("threads: update {}", extract_id(&modified[0]));
        }
        return format!("threads: remove {}", extract_id(&deleted[0]));
    }

    if total <= 3 {
        let mut ids = Vec::new();
        for name in added.iter().chain(modified.iter()).chain(deleted.iter()) {
            ids.push(extract_id(name));
        }
        let action = if added.len() == total {
            "add"
        } else if deleted.len() == total {
            "remove"
        } else {
            "update"
        };
        return format!("threads: {} {}", action, ids.join(" "));
    }

    let action = if added.len() == total {
        "add"
    } else if deleted.len() == total {
        "remove"
    } else {
        "update"
    };
    format!("threads: {} {} threads", action, total)
}

fn extract_id(name: &str) -> String {
    if name.len() >= 6 && is_hex(&name[..6]) {
        name[..6].to_string()
    } else {
        name.to_string()
    }
}

fn is_hex(s: &str) -> bool {
    s.chars()
        .all(|c| c.is_ascii_hexdigit() && !c.is_uppercase())
}

/// Find deleted thread files from git status
pub fn find_deleted_thread_files(repo: &Repository) -> Vec<std::path::PathBuf> {
    let workdir = match repo.workdir() {
        Some(w) => w,
        None => return Vec::new(),
    };

    let mut opts = StatusOptions::new();
    opts.include_untracked(false);

    let statuses = match repo.statuses(Some(&mut opts)) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };

    let mut deleted = Vec::new();

    for entry in statuses.iter() {
        let status = entry.status();
        if status.contains(Status::WT_DELETED) || status.contains(Status::INDEX_DELETED) {
            if let Some(path) = entry.path() {
                if path.contains(".threads/") && path.ends_with(".md") {
                    deleted.push(workdir.join(path));
                }
            }
        }
    }

    deleted
}

/// Get the status of a specific file
pub fn file_status(repo: &Repository, rel_path: &Path) -> FileStatus {
    let mut opts = StatusOptions::new();
    opts.pathspec(rel_path);
    opts.include_untracked(true);

    let statuses = match repo.statuses(Some(&mut opts)) {
        Ok(s) => s,
        Err(_) => return FileStatus::Unknown,
    };

    if statuses.is_empty() {
        return FileStatus::Clean;
    }

    for entry in statuses.iter() {
        let status = entry.status();

        if status.contains(Status::WT_NEW) {
            return FileStatus::Untracked;
        }
        if status.contains(Status::INDEX_NEW) {
            return FileStatus::StagedNew;
        }
        if status.contains(Status::INDEX_DELETED) || status.contains(Status::WT_DELETED) {
            return FileStatus::Deleted;
        }
        if status.contains(Status::INDEX_MODIFIED) && status.contains(Status::WT_MODIFIED) {
            return FileStatus::StagedAndModified;
        }
        if status.contains(Status::INDEX_MODIFIED) {
            return FileStatus::Staged;
        }
        if status.contains(Status::WT_MODIFIED) {
            return FileStatus::Modified;
        }
    }

    FileStatus::Changed
}

/// File status enum for cleaner status reporting
#[derive(Debug, Clone, PartialEq)]
pub enum FileStatus {
    Clean,
    Untracked,
    StagedNew,
    Staged,
    Modified,
    StagedAndModified,
    Deleted,
    Changed,
    Unknown,
}

impl std::fmt::Display for FileStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            FileStatus::Clean => "clean",
            FileStatus::Untracked => "untracked",
            FileStatus::StagedNew => "staged (new)",
            FileStatus::Staged => "staged",
            FileStatus::Modified => "modified",
            FileStatus::StagedAndModified => "staged + modified",
            FileStatus::Deleted => "deleted",
            FileStatus::Changed => "changed",
            FileStatus::Unknown => "unknown",
        };
        write!(f, "{}", s)
    }
}

/// Get the timestamp of the last commit that touched a file.
/// Returns None if the file has never been committed.
pub fn last_commit_date(repo: &Repository, rel_path: &Path) -> Option<i64> {
    let mut revwalk = repo.revwalk().ok()?;
    revwalk.push_head().ok()?;
    revwalk.set_sorting(git2::Sort::TIME).ok()?;

    for oid in revwalk.flatten() {
        let commit = repo.find_commit(oid).ok()?;
        let tree = commit.tree().ok()?;

        // For the first commit (no parent), check if file exists in tree
        let parent_tree = commit.parent(0).ok().and_then(|p| p.tree().ok());

        // Check if this commit touched our file by comparing trees
        let diff = repo
            .diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), None)
            .ok()?;

        for delta in diff.deltas() {
            let dominated_paths: [Option<&Path>; 2] = [delta.new_file().path(), delta.old_file().path()];
            if dominated_paths.iter().flatten().any(|p| *p == rel_path) {
                return Some(commit.time().seconds());
            }
        }
    }

    None
}

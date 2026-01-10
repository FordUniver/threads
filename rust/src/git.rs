use std::path::Path;
use std::process::Command;

/// Check if a file has uncommitted changes (staged, unstaged, or untracked)
pub fn has_changes(ws: &Path, rel_path: &str) -> bool {
    // Check unstaged changes
    let status = Command::new("git")
        .args(["-C", &ws.to_string_lossy(), "diff", "--quiet", "--", rel_path])
        .status();

    if status.map(|s| !s.success()).unwrap_or(true) {
        return true;
    }

    // Check staged changes
    let status = Command::new("git")
        .args(["-C", &ws.to_string_lossy(), "diff", "--cached", "--quiet", "--", rel_path])
        .status();

    if status.map(|s| !s.success()).unwrap_or(true) {
        return true;
    }

    // Check if untracked
    !is_tracked(ws, rel_path)
}

/// Check if a file is tracked by git
pub fn is_tracked(ws: &Path, rel_path: &str) -> bool {
    Command::new("git")
        .args(["-C", &ws.to_string_lossy(), "ls-files", "--error-unmatch", rel_path])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Check if a file exists in HEAD
pub fn exists_in_head(ws: &Path, rel_path: &str) -> bool {
    let ref_str = format!("HEAD:{}", rel_path);
    Command::new("git")
        .args(["-C", &ws.to_string_lossy(), "cat-file", "-e", &ref_str])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Stage files
pub fn add(ws: &Path, files: &[&str]) -> Result<(), String> {
    let ws_str = ws.to_string_lossy();
    let mut args = vec!["-C", &ws_str, "add"];
    args.extend(files);

    let output = Command::new("git")
        .args(&args)
        .output()
        .map_err(|e| format!("git add failed: {}", e))?;

    if !output.status.success() {
        return Err(format!(
            "git add failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(())
}

/// Create a commit
pub fn commit(ws: &Path, files: &[String], message: &str) -> Result<(), String> {
    // Stage files
    let file_refs: Vec<&str> = files.iter().map(|s| s.as_str()).collect();
    add(ws, &file_refs)?;

    // Commit
    let ws_str = ws.to_string_lossy();
    let mut args = vec!["-C".to_string(), ws_str.to_string(), "commit".to_string(), "-m".to_string(), message.to_string()];
    for f in files {
        args.push(f.clone());
    }

    let output = Command::new("git")
        .args(&args)
        .output()
        .map_err(|e| format!("git commit failed: {}", e))?;

    if !output.status.success() {
        return Err(format!(
            "git commit failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(())
}

/// Auto-commit: stage and commit (push is opt-in via separate command)
pub fn auto_commit(ws: &Path, file: &Path, message: &str) -> Result<(), String> {
    let rel_path = file
        .strip_prefix(ws)
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| file.to_string_lossy().to_string());

    commit(ws, &[rel_path], message)?;

    Ok(())
}

/// Generate a commit message for thread changes
pub fn generate_commit_message(ws: &Path, files: &[String]) -> String {
    let mut added = Vec::new();
    let mut modified = Vec::new();
    let mut deleted = Vec::new();

    for file in files {
        let path = Path::new(file);
        let rel_path = path
            .strip_prefix(ws)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| file.clone());

        let name = path
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_default();
        let name = name.trim_end_matches(".md").to_string();

        if exists_in_head(ws, &rel_path) {
            if path.exists() {
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
    s.chars().all(|c| c.is_ascii_hexdigit() && !c.is_uppercase())
}

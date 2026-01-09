use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;

use crate::thread;

// Cached regexes for workspace operations
static ID_ONLY_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[0-9a-f]{6}$").unwrap()
});

static SLUGIFY_NON_ALNUM_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"[^a-z0-9]+").unwrap()
});

static SLUGIFY_MULTI_DASH_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"-+").unwrap()
});

/// Find the workspace root from $WORKSPACE
pub fn find() -> Result<PathBuf, String> {
    let ws = env::var("WORKSPACE")
        .map_err(|_| "WORKSPACE environment variable not set".to_string())?;
    if ws.is_empty() {
        return Err("WORKSPACE environment variable not set".to_string());
    }
    let path = PathBuf::from(&ws);
    if !path.is_dir() {
        return Err(format!("WORKSPACE directory does not exist: {}", ws));
    }
    Ok(path)
}

/// Find all thread file paths in the workspace
pub fn find_all_threads(ws: &Path) -> Result<Vec<PathBuf>, String> {
    let mut threads = Vec::new();

    // Workspace level
    if let Ok(entries) = fs::read_dir(ws.join(".threads")) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map_or(false, |e| e == "md") {
                if !path.to_string_lossy().contains("/archive/") {
                    threads.push(path);
                }
            }
        }
    }

    // Category and project level
    if let Ok(categories) = fs::read_dir(ws) {
        for cat_entry in categories.flatten() {
            let cat_path = cat_entry.path();
            if !cat_path.is_dir() {
                continue;
            }
            let cat_name = cat_entry.file_name();
            if cat_name.to_string_lossy().starts_with('.') {
                continue;
            }

            // Category level threads
            let cat_threads = cat_path.join(".threads");
            if let Ok(entries) = fs::read_dir(&cat_threads) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().map_or(false, |e| e == "md") {
                        if !path.to_string_lossy().contains("/archive/") {
                            threads.push(path);
                        }
                    }
                }
            }

            // Project level
            if let Ok(projects) = fs::read_dir(&cat_path) {
                for proj_entry in projects.flatten() {
                    let proj_path = proj_entry.path();
                    if !proj_path.is_dir() {
                        continue;
                    }
                    let proj_name = proj_entry.file_name();
                    if proj_name.to_string_lossy().starts_with('.') {
                        continue;
                    }

                    // Project level threads
                    let proj_threads = proj_path.join(".threads");
                    if let Ok(entries) = fs::read_dir(&proj_threads) {
                        for entry in entries.flatten() {
                            let path = entry.path();
                            if path.extension().map_or(false, |e| e == "md") {
                                if !path.to_string_lossy().contains("/archive/") {
                                    threads.push(path);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    threads.sort();
    Ok(threads)
}

/// Scope represents thread placement information
pub struct Scope {
    pub threads_dir: PathBuf,
    #[allow(dead_code)]
    pub category: String,
    #[allow(dead_code)]
    pub project: String,
    pub level_desc: String,
}

/// Infer the threads directory and level from a path
pub fn infer_scope(ws: &Path, path: &str) -> Result<Scope, String> {
    // Handle explicit "." for workspace level
    if path == "." {
        return Ok(Scope {
            threads_dir: ws.join(".threads"),
            category: "-".to_string(),
            project: "-".to_string(),
            level_desc: "workspace-level thread".to_string(),
        });
    }

    // Resolve to absolute path
    let abs_path = if Path::new(path).is_absolute() {
        PathBuf::from(path)
    } else {
        // Try as relative to workspace first
        let ws_rel = ws.join(path);
        if ws_rel.is_dir() {
            ws_rel
        } else {
            // Try as relative to cwd
            let cwd_rel = env::current_dir()
                .map(|cwd| cwd.join(path))
                .unwrap_or_else(|_| PathBuf::from(path));
            if cwd_rel.is_dir() {
                cwd_rel
            } else {
                return Err(format!("path not found: {}", path));
            }
        }
    };

    // Must be within workspace
    let ws_str = ws.to_string_lossy();
    let abs_str = abs_path.to_string_lossy();
    if !abs_str.starts_with(ws_str.as_ref()) {
        return Ok(Scope {
            threads_dir: ws.join(".threads"),
            category: "-".to_string(),
            project: "-".to_string(),
            level_desc: "workspace-level thread".to_string(),
        });
    }

    let rel = abs_path
        .strip_prefix(ws)
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|_| PathBuf::new());

    if rel.as_os_str().is_empty() {
        return Ok(Scope {
            threads_dir: ws.join(".threads"),
            category: "-".to_string(),
            project: "-".to_string(),
            level_desc: "workspace-level thread".to_string(),
        });
    }

    let parts: Vec<_> = rel.components().collect();
    let category = parts
        .first()
        .map(|c| c.as_os_str().to_string_lossy().to_string())
        .unwrap_or_else(|| "-".to_string());

    let project = if parts.len() >= 2 {
        parts[1].as_os_str().to_string_lossy().to_string()
    } else {
        "-".to_string()
    };

    if project == "-" {
        Ok(Scope {
            threads_dir: ws.join(&category).join(".threads"),
            category: category.clone(),
            project: "-".to_string(),
            level_desc: format!("category-level thread ({})", category),
        })
    } else {
        Ok(Scope {
            threads_dir: ws.join(&category).join(&project).join(".threads"),
            category: category.clone(),
            project: project.clone(),
            level_desc: format!("project-level thread ({}/{})", category, project),
        })
    }
}

/// Parse thread path to extract category, project, and name
pub fn parse_thread_path(ws: &Path, path: &Path) -> (String, String, String) {
    let ws_str = ws.to_string_lossy();
    let path_str = path.to_string_lossy();

    let rel = if path_str.starts_with(ws_str.as_ref()) {
        path_str[ws_str.len()..].trim_start_matches('/').to_string()
    } else {
        path_str.to_string()
    };

    let filename = path
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_default();
    let filename = filename.trim_end_matches(".md");

    // Extract name, stripping ID prefix if present
    let name = thread::extract_name_from_path(path);
    let name = if name.is_empty() {
        filename.to_string()
    } else {
        name
    };

    // Check if workspace-level
    if rel.starts_with(".threads/") {
        return ("-".to_string(), "-".to_string(), name);
    }

    // Extract category and project
    let parts: Vec<&str> = rel.split('/').collect();
    if parts.len() >= 2 {
        let category = parts[0].to_string();
        if parts[1] == ".threads" {
            (category, "-".to_string(), name)
        } else if parts.len() >= 3 {
            let project = parts[1].to_string();
            (category, project, name)
        } else {
            (category, "-".to_string(), name)
        }
    } else {
        ("-".to_string(), "-".to_string(), name)
    }
}

/// Generate a unique 6-character hex ID
pub fn generate_id(ws: &Path) -> Result<String, String> {
    let threads = find_all_threads(ws)?;
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

/// Convert a title to kebab-case filename
pub fn slugify(title: &str) -> String {
    let s = title.to_lowercase();
    let s = SLUGIFY_NON_ALNUM_RE.replace_all(&s, "-");
    let s = SLUGIFY_MULTI_DASH_RE.replace_all(&s, "-");
    s.trim_matches('-').to_string()
}

/// Find a thread by ID or name (with fuzzy matching)
pub fn find_by_ref(ws: &Path, ref_str: &str) -> Result<PathBuf, String> {
    let threads = find_all_threads(ws)?;

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
    pub fn encode(bytes: [u8; 3]) -> String {
        bytes.iter().map(|b| format!("{:02x}", b)).collect()
    }
}

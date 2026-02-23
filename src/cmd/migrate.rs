use std::path::{Path, PathBuf};

use clap::Args;
use colored::Colorize;

use crate::args::DirectionArgs;
use crate::thread::{
    self, Thread, get_log_entries_from_section, get_notes_from_section,
    get_todo_items_from_section, strip_old_sections,
};
use crate::workspace;

#[derive(Args)]
pub struct MigrateArgs {
    /// Thread ID to migrate (omit to migrate threads in scope)
    #[arg(default_value = "")]
    id: String,

    #[command(flatten)]
    direction: DirectionArgs,

    /// Migrate all threads in workspace
    #[arg(short = 'a', long)]
    all: bool,

    /// Preview changes without writing
    #[arg(long)]
    dry_run: bool,
}

pub fn run(args: MigrateArgs, ws: &Path) -> Result<(), String> {
    if !args.id.is_empty() {
        // Single thread
        let file = workspace::find_by_ref(ws, &args.id)?;
        migrate_file(&file, ws, args.dry_run)?;
    } else {
        // Multi-thread mode
        let files = collect_files(&args, ws)?;
        if files.is_empty() {
            println!("No threads found.");
            return Ok(());
        }

        let mut migrated = 0;
        let mut already = 0;
        let mut errors = 0;

        for file in &files {
            match migrate_file(file, ws, args.dry_run) {
                Ok(true) => migrated += 1,
                Ok(false) => already += 1,
                Err(e) => {
                    let rel = file.strip_prefix(ws).unwrap_or(file);
                    eprintln!("{}: {}", rel.display(), e);
                    errors += 1;
                }
            }
        }

        if args.dry_run {
            println!(
                "\nDry run: {} would be migrated, {} already migrated",
                migrated, already
            );
        } else {
            let mut parts = Vec::new();
            if migrated > 0 {
                parts.push(format!("{} migrated", migrated));
            }
            if already > 0 {
                parts.push(format!("{} already migrated", already));
            }
            if errors > 0 {
                parts.push(format!("{} errors", errors).red().to_string());
            }
            println!("{}", parts.join(", "));
        }

        if errors > 0 {
            return Err(format!("{} files had errors", errors));
        }
    }

    Ok(())
}

fn collect_files(args: &MigrateArgs, ws: &Path) -> Result<Vec<PathBuf>, String> {
    if args.all {
        return workspace::find_all_threads(ws);
    }

    let scope = workspace::infer_scope(ws, None)?;
    let start_path = scope.threads_dir.parent().unwrap_or(ws);
    let options = args.direction.to_find_options();
    workspace::find_threads_with_options(start_path, ws, &options)
}

/// Migrate a single thread file from section-based to frontmatter-based storage.
/// Returns Ok(true) if migration was performed, Ok(false) if already migrated.
fn migrate_file(file: &Path, ws: &Path, dry_run: bool) -> Result<bool, String> {
    let mut t = Thread::parse(file)?;

    let rel = file
        .strip_prefix(ws)
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| file.to_string_lossy().to_string());

    // Check if any old sections exist
    let has_notes_section = thread::extract_section(&t.content, "Notes")
        .lines()
        .any(|l| l.starts_with("- "));
    let has_todo_section = thread::extract_section(&t.content, "Todo")
        .lines()
        .any(|l| l.starts_with("- ["));
    let has_log_section = !thread::extract_section(&t.content, "Log").is_empty();

    if !has_notes_section && !has_todo_section && !has_log_section {
        // Already migrated or empty sections
        if dry_run {
            println!("already migrated: {}", rel.dimmed());
        }
        return Ok(false);
    }

    // Parse items from old sections (only if frontmatter is empty for that type)
    let notes = if t.frontmatter.notes.is_empty() {
        get_notes_from_section(&t.content)
    } else {
        t.frontmatter.notes.clone()
    };
    let todos = if t.frontmatter.todo.is_empty() {
        get_todo_items_from_section(&t.content)
    } else {
        t.frontmatter.todo.clone()
    };
    let log_entries = if t.frontmatter.log.is_empty() {
        get_log_entries_from_section(&t.content)
    } else {
        t.frontmatter.log.clone()
    };

    let n_notes = notes.len();
    let n_todos = todos.len();
    let n_log = log_entries.len();

    if dry_run {
        let mut parts = Vec::new();
        if n_notes > 0 {
            parts.push(format!("{} notes", n_notes));
        }
        if n_todos > 0 {
            parts.push(format!("{} todos", n_todos));
        }
        if n_log > 0 {
            parts.push(format!("{} log entries", n_log));
        }
        println!(
            "would migrate: {} ({})",
            rel,
            if parts.is_empty() {
                "empty sections".to_string()
            } else {
                parts.join(", ")
            }
        );
        return Ok(true);
    }

    // Strip old sections from the markdown body
    let old_body = t.content[t.body_start..].to_string();
    let new_body = strip_old_sections(&old_body);

    // Update content with stripped body (body_start stays valid — same offset in old frontmatter)
    t.content = t.content[..t.body_start].to_string() + &new_body;

    // Set frontmatter items
    t.frontmatter.notes = notes;
    t.frontmatter.todo = todos;
    t.frontmatter.log = log_entries;

    // Rebuild content (serializes updated frontmatter + stripped body, updates body_start)
    t.rebuild_content()?;
    t.write()?;

    let mut parts = Vec::new();
    if n_notes > 0 {
        parts.push(format!("{} notes", n_notes));
    }
    if n_todos > 0 {
        parts.push(format!("{} todos", n_todos));
    }
    if n_log > 0 {
        parts.push(format!("{} log entries", n_log));
    }

    println!(
        "migrated: {} ({})",
        rel,
        if parts.is_empty() {
            "empty sections stripped".to_string()
        } else {
            parts.join(", ")
        }
    );

    Ok(true)
}

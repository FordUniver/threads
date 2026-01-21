use std::fs;
use std::io::{self, Read};
use std::path::Path;

use chrono::Local;
use clap::Args;
use serde::Serialize;

use crate::git;
use crate::output::OutputFormat;
use crate::thread;
use crate::workspace;

#[derive(Args)]
pub struct NewArgs {
    /// [path] title - Path is optional, title is required
    /// Path resolution:
    ///   (none)  → PWD (current directory)
    ///   .       → PWD (explicit)
    ///   ./X/Y   → PWD-relative
    ///   /X/Y    → Absolute
    ///   X/Y     → Git-root-relative
    #[arg(required = true, num_args = 1..=2)]
    args: Vec<String>,

    /// Initial status
    #[arg(long, default_value = "idea")]
    status: String,

    /// One-line description
    #[arg(long, default_value = "")]
    desc: String,

    /// Initial body content
    #[arg(long, default_value = "")]
    body: String,

    /// Commit after creating
    #[arg(long)]
    commit: bool,

    /// Commit message
    #[arg(short = 'm', long)]
    m: Option<String>,

    /// Output format (auto-detects TTY for fancy vs plain)
    #[arg(short = 'f', long, value_enum, default_value = "fancy")]
    format: OutputFormat,

    /// Output as JSON (shorthand for --format=json)
    #[arg(long, conflicts_with = "format")]
    json: bool,
}

#[derive(Serialize)]
struct NewOutput {
    id: String,
    path: String,
    path_absolute: String,
}

pub fn run(args: NewArgs, git_root: &Path) -> Result<(), String> {
    let format = if args.json {
        OutputFormat::Json
    } else {
        args.format.resolve()
    };

    // Validate status early
    if !thread::is_valid_status(&args.status) {
        return Err(format!(
            "Invalid status '{}'. Must be one of: idea, planning, active, blocked, paused, resolved, superseded, deferred, rejected",
            args.status
        ));
    }

    // Parse positional args: either [title] or [path, title]
    let (path_arg, title) = if args.args.len() == 2 {
        (Some(args.args[0].as_str()), args.args[1].clone())
    } else if args.args.len() == 1 {
        // Single arg is title, no path specified (will use PWD)
        (None, args.args[0].clone())
    } else {
        return Err("title is required".to_string());
    };

    if title.is_empty() {
        return Err("title is required".to_string());
    }

    // Warn if no description provided
    if args.desc.is_empty() {
        eprintln!("Warning: No --desc provided. Add one with: threads update <id> --desc \"...\"");
    }

    // Slugify title
    let slug = workspace::slugify(&title);
    if slug.is_empty() {
        return Err("title produces empty slug".to_string());
    }

    // Read body from stdin if available and not provided via flag
    let body = if args.body.is_empty() {
        read_stdin_if_available()
    } else {
        args.body.clone()
    };

    // Determine scope using new path resolution
    let scope = workspace::infer_scope(git_root, path_arg)?;

    // Generate ID
    let id = workspace::generate_id(git_root)?;

    // Ensure threads directory exists
    fs::create_dir_all(&scope.threads_dir)
        .map_err(|e| format!("creating threads directory: {}", e))?;

    // Build file path
    let filename = format!("{}-{}.md", id, slug);
    let thread_path = scope.threads_dir.join(&filename);

    // Check if file already exists
    if thread_path.exists() {
        return Err(format!("thread already exists: {}", thread_path.display()));
    }

    // Generate content
    let today = Local::now().format("%Y-%m-%d").to_string();
    let timestamp = Local::now().format("%H:%M").to_string();

    let mut content = String::new();
    content.push_str("---\n");
    content.push_str(&format!("id: {}\n", id));
    content.push_str(&format!("name: {}\n", title));
    content.push_str(&format!("desc: {}\n", args.desc));
    content.push_str(&format!("status: {}\n", args.status));
    content.push_str("---\n\n");

    if !body.is_empty() {
        content.push_str(&body);
        if !body.ends_with('\n') {
            content.push('\n');
        }
        content.push('\n');
    }

    content.push_str("## Todo\n\n");
    content.push_str("## Log\n\n");
    content.push_str(&format!("### {}\n\n", today));
    content.push_str(&format!("- **{}** Created thread.\n", timestamp));

    // Write file
    fs::write(&thread_path, &content).map_err(|e| format!("writing thread file: {}", e))?;

    // Display path relative to git root
    let rel_path = workspace::path_relative_to_git_root(git_root, &thread_path);

    match format {
        OutputFormat::Fancy | OutputFormat::Plain => {
            println!("Created thread in {}: {}", scope.level_desc, id);
            println!("  → {}", rel_path);

            if body.is_empty() {
                eprintln!(
                    "Hint: Add body with: echo \"content\" | threads body {} --set",
                    id
                );
            }
        }
        OutputFormat::Json => {
            let output = NewOutput {
                id: id.clone(),
                path: rel_path.clone(),
                path_absolute: thread_path.to_string_lossy().to_string(),
            };
            let json = serde_json::to_string_pretty(&output)
                .map_err(|e| format!("JSON serialization failed: {}", e))?;
            println!("{}", json);
        }
        OutputFormat::Yaml => {
            let output = NewOutput {
                id: id.clone(),
                path: rel_path.clone(),
                path_absolute: thread_path.to_string_lossy().to_string(),
            };
            let yaml = serde_yaml::to_string(&output)
                .map_err(|e| format!("YAML serialization failed: {}", e))?;
            print!("{}", yaml);
        }
    }

    // Commit if requested
    if args.commit {
        let msg = args.m.unwrap_or_else(|| {
            git::generate_commit_message(git_root, &[thread_path.to_string_lossy().to_string()])
        });
        git::auto_commit(git_root, &thread_path, &msg)?;
    } else if matches!(format, OutputFormat::Fancy | OutputFormat::Plain) {
        println!(
            "Note: Thread {} has uncommitted changes. Use 'threads commit {}' when ready.",
            id, id
        );
    }

    Ok(())
}

fn read_stdin_if_available() -> String {
    // Check if stdin has data available (non-blocking check)
    // On Unix, we can check if stdin is a tty
    if is_stdin_piped() {
        let mut buffer = String::new();
        if io::stdin().read_to_string(&mut buffer).is_ok() {
            return buffer;
        }
    }
    String::new()
}

fn is_stdin_piped() -> bool {
    use std::io::IsTerminal;
    !io::stdin().is_terminal()
}

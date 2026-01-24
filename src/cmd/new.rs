use std::fs;
use std::path::Path;

use chrono::Local;
use clap::Args;
use serde::Serialize;

use crate::args::FormatArgs;
use crate::config::{Config, env_bool, env_string, is_quiet};
use crate::git;
use crate::input;
use crate::output::{self, OutputFormat};
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

    #[command(flatten)]
    format: FormatArgs,
}

#[derive(Serialize)]
struct NewOutput {
    id: String,
    path: String,
    path_absolute: String,
}

pub fn run(args: NewArgs, git_root: &Path, config: &Config) -> Result<(), String> {
    let format = args.format.resolve();

    // Resolve status: CLI flag > THREADS_DEFAULT_STATUS env > config default > hardcoded default
    let default_status = &config.defaults.new;
    let status = if args.status != "idea" {
        // User explicitly set --status
        args.status.clone()
    } else if let Some(env_status) = env_string("THREADS_DEFAULT_STATUS") {
        env_status
    } else {
        default_status.clone()
    };

    // Validate status early using config status lists
    if !thread::is_valid_status_with_config(&status, &config.status.open, &config.status.closed) {
        let all_statuses: Vec<&str> = config
            .status
            .open
            .iter()
            .chain(config.status.closed.iter())
            .map(|s| s.as_str())
            .collect();
        return Err(format!(
            "Invalid status '{}'. Must be one of: {}",
            status,
            all_statuses.join(", ")
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

    // Warn if no description provided (unless quiet mode)
    if args.desc.is_empty() && !is_quiet(config) {
        eprintln!("Warning: No --desc provided. Add one with: threads update <id> --desc \"...\"");
    }

    // Slugify title
    let slug = workspace::slugify(&title);
    if slug.is_empty() {
        return Err("title produces empty slug".to_string());
    }

    // Read body from stdin if available and not provided via flag
    let body = if args.body.is_empty() {
        input::read_stdin(false)
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
    let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

    let mut content = String::new();
    content.push_str("---\n");
    content.push_str(&format!("id: {}\n", id));
    content.push_str(&format!("name: {}\n", quote_yaml_value(&title)));
    content.push_str(&format!("desc: {}\n", quote_yaml_value(&args.desc)));
    content.push_str(&format!("status: {}\n", status));
    content.push_str("---\n\n");

    // Add Body section
    content.push_str("## Body\n\n");
    if !body.is_empty() {
        content.push_str(&body);
        if !body.ends_with('\n') {
            content.push('\n');
        }
        content.push('\n');
    }

    // Add Notes section (empty by default)
    content.push_str("## Notes\n\n");

    // Add Todo section
    content.push_str("## Todo\n\n");

    // Add Log section
    content.push_str("## Log\n\n");
    content.push_str(&format!("- [{}] Created thread.\n", timestamp));

    // Write file
    fs::write(&thread_path, &content).map_err(|e| format!("writing thread file: {}", e))?;

    // Display path relative to git root
    let rel_path = workspace::path_relative_to_git_root(git_root, &thread_path);

    match format {
        OutputFormat::Pretty | OutputFormat::Plain => {
            println!("Created thread in {}: {}", scope.level_desc, id);
            println!("  → {}", rel_path);

            if body.is_empty() && !is_quiet(config) {
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

    // Commit if requested or THREADS_AUTO_COMMIT is set
    let should_commit = args.commit || env_bool("THREADS_AUTO_COMMIT").unwrap_or(false);
    if should_commit {
        let repo = workspace::open()?;
        let rel_path = thread_path.strip_prefix(git_root).unwrap_or(&thread_path);
        let msg = args
            .m
            .unwrap_or_else(|| git::generate_commit_message(&repo, &[rel_path]));
        git::auto_commit(&repo, &thread_path, &msg)?;
    } else if matches!(format, OutputFormat::Pretty | OutputFormat::Plain) && !is_quiet(config) {
        output::print_uncommitted_hint(&id);
    }

    Ok(())
}

/// Quote a YAML value if it contains special characters
fn quote_yaml_value(value: &str) -> String {
    // Check if already quoted
    if (value.starts_with('"') && value.ends_with('"'))
        || (value.starts_with('\'') && value.ends_with('\''))
    {
        return value.to_string();
    }

    // Check if quoting is needed
    let special_chars = [
        ':', '#', '[', ']', '{', '}', ',', '&', '*', '!', '|', '>', '%', '@', '`',
    ];
    let special_starts = [
        '-', '?', ':', '&', '*', '!', '|', '>', '\'', '"', '%', '@', '`',
    ];

    let needs_quoting = value.chars().any(|c| special_chars.contains(&c))
        || value
            .chars()
            .next()
            .map(|c| special_starts.contains(&c))
            .unwrap_or(false)
        || value != value.trim()
        || matches!(
            value.to_lowercase().as_str(),
            "true" | "false" | "null" | "yes" | "no" | "on" | "off"
        )
        || value.parse::<f64>().is_ok();

    if needs_quoting {
        // Prefer single quotes unless value contains them
        if value.contains('\'') {
            let escaped = value.replace('\\', "\\\\").replace('"', "\\\"");
            format!("\"{}\"", escaped)
        } else {
            format!("'{}'", value)
        }
    } else {
        value.to_string()
    }
}

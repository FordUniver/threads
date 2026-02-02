use std::path::Path;

use clap::Args;
use clap_complete::engine::ArgValueCompleter;

use crate::config::{Config, env_bool, is_quiet};
use crate::git;
use crate::input;
use crate::output;
use crate::thread::{self, Thread};
use crate::workspace;

/// Read or edit the Body section of a thread.
///
/// Without flags and from an interactive terminal, displays the current body.
/// With piped input, writes to the body (--set by default, --append to add).
#[derive(Args)]
pub struct BodyArgs {
    /// Thread ID or name reference
    #[arg(add = ArgValueCompleter::new(crate::workspace::complete_thread_ids))]
    id: String,

    /// Replace body content (default when content is piped)
    #[arg(long)]
    set: bool,

    /// Append to existing body content
    #[arg(long)]
    append: bool,

    /// Commit after editing
    #[arg(long)]
    commit: bool,

    /// Commit message
    #[arg(short = 'm', long)]
    m: Option<String>,
}

pub fn run(args: BodyArgs, ws: &Path, config: &Config) -> Result<(), String> {
    // Check TTY state before reading - this distinguishes interactive use from empty pipe
    let stdin_is_tty = input::stdin_is_tty();
    let content = input::read_stdin(false);

    // Read mode: no flags AND stdin is a terminal (interactive use)
    // This prevents `printf '' | threads body <id>` from silently succeeding
    if !args.set && !args.append && stdin_is_tty {
        let file = workspace::find_by_ref(ws, &args.id)?;
        let t = Thread::parse(&file)?;
        let body = thread::extract_section(&t.content, "Body");
        if !body.is_empty() {
            println!("{}", body);
        }
        return Ok(());
    }

    // Write mode: require content
    if content.is_empty() {
        return Err("no content provided (use stdin)".to_string());
    }

    // Default to set mode for writes
    let set_mode = args.set || !args.append;

    let file = workspace::find_by_ref(ws, &args.id)?;

    let mut t = Thread::parse(&file)?;

    if set_mode {
        t.content = thread::replace_section(&t.content, "Body", &content);
    } else {
        t.content = thread::append_to_section(&t.content, "Body", &content);
    }

    t.write()?;

    let mode = if set_mode { "set" } else { "append" };
    println!("Body {}: {}", mode, file.display());

    let should_commit = args.commit || env_bool("THREADS_AUTO_COMMIT").unwrap_or(false);
    if should_commit {
        let repo = workspace::open()?;
        let rel_path = file.strip_prefix(ws).unwrap_or(&file);
        let msg = args
            .m
            .unwrap_or_else(|| git::generate_commit_message(&repo, &[rel_path]));
        git::auto_commit(&repo, &file, &msg)?;
    } else if !is_quiet(config) {
        output::print_uncommitted_hint(&args.id);
    }

    Ok(())
}

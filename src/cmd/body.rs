use std::path::Path;

use clap::Args;
use clap_complete::engine::ArgValueCompleter;

use crate::config::{env_bool, is_quiet, resolve_section_name, Config};
use crate::git;
use crate::input;
use crate::thread::{self, Thread};
use crate::workspace;

#[derive(Args)]
pub struct BodyArgs {
    /// Thread ID or name reference
    #[arg(add = ArgValueCompleter::new(crate::workspace::complete_thread_ids))]
    id: String,

    /// Replace body content
    #[arg(long)]
    set: bool,

    /// Append to body content
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
    // Default to set mode
    let set_mode = args.set || !args.append;

    // Read content from stdin
    let content = input::read_stdin(false);

    if content.is_empty() {
        return Err("no content provided (use stdin)".to_string());
    }

    // Get configured section name (or error if disabled)
    let section_name = resolve_section_name(&config.sections, "Body")
        .ok_or("Body section is disabled in config")?;

    let file = workspace::find_by_ref(ws, &args.id)?;

    let mut t = Thread::parse(&file)?;

    // Ensure section exists (may be missing in old threads or with renamed sections)
    // Insert Body before Notes or Todo, whichever comes first
    let insert_before = resolve_section_name(&config.sections, "Notes")
        .or_else(|| resolve_section_name(&config.sections, "Todo"))
        .unwrap_or("Todo");
    t.content = thread::ensure_section(&t.content, section_name, insert_before);

    if set_mode {
        t.content = thread::replace_section(&t.content, section_name, &content);
    } else {
        t.content = thread::append_to_section(&t.content, section_name, &content);
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
        println!(
            "Note: Thread {} has uncommitted changes. Use 'threads commit {}' when ready.",
            args.id, args.id
        );
    }

    Ok(())
}

use std::path::Path;

use clap::Args;
use clap_complete::engine::ArgValueCompleter;

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

pub fn run(args: BodyArgs, ws: &Path) -> Result<(), String> {
    // Default to set mode
    let set_mode = args.set || !args.append;

    // Read content from stdin
    let content = input::read_stdin(false);

    if content.is_empty() {
        return Err("no content provided (use stdin)".to_string());
    }

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

    if args.commit {
        let repo = workspace::open()?;
        let rel_path = file.strip_prefix(ws).unwrap_or(&file);
        let msg = args
            .m
            .unwrap_or_else(|| git::generate_commit_message(&repo, &[rel_path]));
        git::auto_commit(&repo, &file, &msg)?;
    } else {
        println!(
            "Note: Thread {} has uncommitted changes. Use 'threads commit {}' when ready.",
            args.id, args.id
        );
    }

    Ok(())
}

use std::io::{self, Read};
use std::path::Path;

use clap::Args;
use clap_complete::engine::ArgValueCompleter;

use crate::git;
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
    let content = read_stdin_if_available();

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
        let msg = args.m.unwrap_or_else(|| {
            git::generate_commit_message(ws, &[file.to_string_lossy().to_string()])
        });
        git::auto_commit(ws, &file, &msg)?;
    } else {
        println!(
            "Note: Thread {} has uncommitted changes. Use 'threads commit {}' when ready.",
            args.id, args.id
        );
    }

    Ok(())
}

fn read_stdin_if_available() -> String {
    // Check if stdin has data available
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

use std::io::{self, Read};
use std::path::Path;

use clap::Args;

use crate::git;
use crate::thread::{self, Thread};
use crate::workspace;

#[derive(Args)]
pub struct LogArgs {
    /// Thread ID or name reference
    id: String,

    /// Log entry text
    #[arg(default_value = "")]
    entry: String,

    /// Commit after adding
    #[arg(long)]
    commit: bool,

    /// Commit message
    #[arg(short = 'm', long)]
    m: Option<String>,
}

pub fn run(args: LogArgs, ws: &Path) -> Result<(), String> {
    let mut entry = args.entry.clone();

    // Read entry from stdin if not provided
    if entry.is_empty() {
        entry = read_stdin_if_available();
    }

    if entry.is_empty() {
        return Err("no log entry provided".to_string());
    }

    let file = workspace::find_by_ref(ws, &args.id)?;

    let mut t = Thread::parse(&file)?;

    t.content = thread::insert_log_entry(&t.content, &entry);

    t.write()?;

    println!("Logged to: {}", file.display());

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
    if is_stdin_piped() {
        let mut buffer = String::new();
        if io::stdin().read_to_string(&mut buffer).is_ok() {
            return buffer.trim().to_string();
        }
    }
    String::new()
}

fn is_stdin_piped() -> bool {
    use std::os::unix::io::AsRawFd;
    unsafe { libc::isatty(io::stdin().as_raw_fd()) == 0 }
}

use std::path::Path;

use clap::Args;
use clap_complete::engine::ArgValueCompleter;

use crate::git;
use crate::thread::Thread;
use crate::workspace;

#[derive(Args)]
pub struct UpdateArgs {
    /// Thread ID or name reference
    #[arg(add = ArgValueCompleter::new(crate::workspace::complete_thread_ids))]
    id: String,

    /// New title
    #[arg(long)]
    title: Option<String>,

    /// New description
    #[arg(long)]
    desc: Option<String>,

    /// Commit after updating
    #[arg(long)]
    commit: bool,

    /// Commit message
    #[arg(short = 'm', long)]
    m: Option<String>,
}

pub fn run(args: UpdateArgs, ws: &Path) -> Result<(), String> {
    if args.title.is_none() && args.desc.is_none() {
        return Err("specify --title and/or --desc".to_string());
    }

    let file = workspace::find_by_ref(ws, &args.id)?;

    let mut t = Thread::parse(&file)?;

    if let Some(ref title) = args.title {
        t.set_frontmatter_field("name", title)?;
        println!("Title updated: {}", title);
    }

    if let Some(ref desc) = args.desc {
        t.set_frontmatter_field("desc", desc)?;
        println!("Description updated: {}", desc);
    }

    t.write()?;

    println!("Updated: {}", file.display());

    if args.commit {
        let repo = workspace::open()?;
        let rel_path = file.strip_prefix(ws).unwrap_or(&file);
        let msg = args.m.unwrap_or_else(|| {
            git::generate_commit_message(&repo, &[rel_path])
        });
        git::auto_commit(&repo, &file, &msg)?;
    } else {
        println!(
            "Note: Thread {} has uncommitted changes. Use 'threads commit {}' when ready.",
            args.id, args.id
        );
    }

    Ok(())
}

use std::path::Path;

use clap::Args;
use clap_complete::engine::ArgValueCompleter;
use serde::Serialize;

use crate::args::FormatArgs;
use crate::git;
use crate::output::OutputFormat;
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

    #[command(flatten)]
    format: FormatArgs,
}

#[derive(Serialize)]
struct UpdateOutput {
    id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    desc: Option<String>,
    path: String,
    committed: bool,
}

pub fn run(args: UpdateArgs, ws: &Path) -> Result<(), String> {
    let format = args.format.resolve();

    if args.title.is_none() && args.desc.is_none() {
        return Err("specify --title and/or --desc".to_string());
    }

    let file = workspace::find_by_ref(ws, &args.id)?;

    let mut t = Thread::parse(&file)?;
    let id = t.id().to_string();

    if let Some(ref title) = args.title {
        t.set_frontmatter_field("name", title)?;
    }

    if let Some(ref desc) = args.desc {
        t.set_frontmatter_field("desc", desc)?;
    }

    t.write()?;

    let committed = if args.commit {
        let repo = workspace::open()?;
        let rel_path = file.strip_prefix(ws).unwrap_or(&file);
        let msg = args
            .m
            .unwrap_or_else(|| git::generate_commit_message(&repo, &[rel_path]));
        git::auto_commit(&repo, &file, &msg)?;
        true
    } else {
        false
    };

    let rel_path = workspace::path_relative_to_git_root(ws, &file);

    match format {
        OutputFormat::Pretty | OutputFormat::Plain => {
            if let Some(ref title) = args.title {
                println!("Updated title: {}", title);
            }
            if let Some(ref desc) = args.desc {
                println!("Updated desc: {}", desc);
            }
            println!("  → {}", rel_path);
            if !committed {
                println!(
                    "Note: Thread {} has uncommitted changes. Use 'threads commit {}' when ready.",
                    id, id
                );
            }
        }
        OutputFormat::Json => {
            let output = UpdateOutput {
                id,
                title: args.title,
                desc: args.desc,
                path: rel_path,
                committed,
            };
            let json = serde_json::to_string_pretty(&output)
                .map_err(|e| format!("JSON serialization failed: {}", e))?;
            println!("{}", json);
        }
        OutputFormat::Yaml => {
            let output = UpdateOutput {
                id,
                title: args.title,
                desc: args.desc,
                path: rel_path,
                committed,
            };
            let yaml = serde_yaml::to_string(&output)
                .map_err(|e| format!("YAML serialization failed: {}", e))?;
            print!("{}", yaml);
        }
    }

    Ok(())
}

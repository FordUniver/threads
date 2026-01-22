use std::fs;
use std::io::IsTerminal;
use std::path::Path;

use clap::Args;
use clap_complete::engine::ArgValueCompleter;
use termimad::MadSkin;

use crate::workspace;

#[derive(Args)]
pub struct ReadArgs {
    /// Thread ID or name reference
    #[arg(add = ArgValueCompleter::new(crate::workspace::complete_thread_ids))]
    id: String,

    /// Output raw markdown (skip rendering)
    #[arg(long)]
    raw: bool,
}

pub fn run(args: ReadArgs, ws: &Path) -> Result<(), String> {
    let file = workspace::find_by_ref(ws, &args.id)?;

    let content = fs::read_to_string(&file).map_err(|e| format!("reading file: {}", e))?;

    // Render markdown if TTY and not --raw
    if std::io::stdout().is_terminal() && !args.raw {
        let skin = MadSkin::default();
        skin.print_text(&content);
    } else {
        print!("{}", content);
    }
    Ok(())
}

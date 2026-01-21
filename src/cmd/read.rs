use std::fs;
use std::path::Path;

use clap::Args;
use clap_complete::engine::ArgValueCompleter;

use crate::workspace;

#[derive(Args)]
pub struct ReadArgs {
    /// Thread ID or name reference
    #[arg(add = ArgValueCompleter::new(crate::workspace::complete_thread_ids))]
    id: String,
}

pub fn run(args: ReadArgs, ws: &Path) -> Result<(), String> {
    let file = workspace::find_by_ref(ws, &args.id)?;

    let content = fs::read_to_string(&file)
        .map_err(|e| format!("reading file: {}", e))?;

    print!("{}", content);
    Ok(())
}

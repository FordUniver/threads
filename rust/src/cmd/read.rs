use std::fs;
use std::path::Path;

use clap::Args;

use crate::workspace;

#[derive(Args)]
pub struct ReadArgs {
    /// Thread ID or name reference
    id: String,
}

pub fn run(args: ReadArgs, ws: &Path) -> Result<(), String> {
    let file = workspace::find_by_ref(ws, &args.id)?;

    let content = fs::read_to_string(&file)
        .map_err(|e| format!("reading file: {}", e))?;

    print!("{}", content);
    Ok(())
}

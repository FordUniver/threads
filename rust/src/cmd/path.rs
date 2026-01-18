use std::path::Path;

use clap::Args;
use clap_complete::engine::ArgValueCompleter;

use crate::workspace;

#[derive(Args)]
pub struct PathArgs {
    /// Thread ID or name reference
    #[arg(add = ArgValueCompleter::new(crate::workspace::complete_thread_ids))]
    id: String,
}

pub fn run(args: PathArgs, ws: &Path) -> Result<(), String> {
    let file = workspace::find_by_ref(ws, &args.id)?;

    let abs_path = file.canonicalize()
        .unwrap_or_else(|_| file.to_path_buf());

    println!("{}", abs_path.display());
    Ok(())
}

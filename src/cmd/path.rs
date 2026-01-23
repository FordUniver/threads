use std::path::Path;

use clap::Args;
use clap_complete::engine::ArgValueCompleter;
use serde::Serialize;

use crate::args::FormatArgs;
use crate::output::OutputFormat;
use crate::workspace;

#[derive(Args)]
pub struct PathArgs {
    /// Thread ID or name reference
    #[arg(add = ArgValueCompleter::new(crate::workspace::complete_thread_ids))]
    id: String,

    #[command(flatten)]
    format: FormatArgs,
}

#[derive(Serialize)]
struct PathOutput {
    path: String,
    path_absolute: String,
}

pub fn run(args: PathArgs, ws: &Path) -> Result<(), String> {
    let format = args.format.resolve();

    let file = workspace::find_by_ref(ws, &args.id)?;

    let abs_path = file.canonicalize().unwrap_or_else(|_| file.to_path_buf());
    let rel_path = workspace::path_relative_to_git_root(ws, &file);

    match format {
        OutputFormat::Pretty | OutputFormat::Plain => {
            println!("{}", abs_path.display());
        }
        OutputFormat::Json => {
            let output = PathOutput {
                path: rel_path,
                path_absolute: abs_path.to_string_lossy().to_string(),
            };
            let json = serde_json::to_string_pretty(&output)
                .map_err(|e| format!("JSON serialization failed: {}", e))?;
            println!("{}", json);
        }
        OutputFormat::Yaml => {
            let output = PathOutput {
                path: rel_path,
                path_absolute: abs_path.to_string_lossy().to_string(),
            };
            let yaml = serde_yaml::to_string(&output)
                .map_err(|e| format!("YAML serialization failed: {}", e))?;
            print!("{}", yaml);
        }
    }

    Ok(())
}

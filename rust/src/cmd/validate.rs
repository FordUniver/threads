use std::path::Path;

use clap::Args;
use serde::Serialize;

use crate::output::OutputFormat;
use crate::thread::{self, Thread};
use crate::workspace;

#[derive(Args)]
pub struct ValidateArgs {
    /// Path to validate (specific file or all)
    #[arg(default_value = "")]
    path: String,

    /// Include nested categories/projects
    #[arg(short = 'r', long)]
    recursive: bool,

    /// Output format (auto-detects TTY for fancy vs plain)
    #[arg(short = 'f', long, value_enum, default_value = "fancy")]
    format: OutputFormat,

    /// Output as JSON (shorthand for --format=json)
    #[arg(long, conflicts_with = "format")]
    json: bool,
}

#[derive(Serialize)]
struct ValidationResult {
    path: String,
    valid: bool,
    issues: Vec<String>,
}

pub fn run(args: ValidateArgs, ws: &Path) -> Result<(), String> {
    let format = if args.json {
        OutputFormat::Json
    } else {
        args.format.resolve()
    };

    let files = if !args.path.is_empty() {
        let target = &args.path;
        let abs_path = if Path::new(target).is_absolute() {
            target.clone()
        } else {
            ws.join(target).to_string_lossy().to_string()
        };
        vec![abs_path.into()]
    } else {
        workspace::find_all_threads(ws)?
    };

    let mut results = Vec::new();
    let mut errors = 0;

    for file in files {
        let rel_path = file
            .strip_prefix(ws)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| file.to_string_lossy().to_string());

        let mut issues = Vec::new();

        match Thread::parse(&file) {
            Ok(t) => {
                if t.name().is_empty() {
                    issues.push("missing name/title field".to_string());
                }
                if t.status().is_empty() {
                    issues.push("missing status field".to_string());
                } else if !thread::is_valid_status(t.status()) {
                    issues.push(format!("invalid status '{}'", thread::base_status(t.status())));
                }
            }
            Err(e) => {
                issues.push(format!("parse error: {}", e));
            }
        }

        let valid = issues.is_empty();
        if !valid {
            errors += 1;
        }

        results.push(ValidationResult {
            path: rel_path,
            valid,
            issues,
        });
    }

    match format {
        OutputFormat::Fancy | OutputFormat::Plain => {
            for r in &results {
                if r.valid {
                    println!("OK: {}", r.path);
                } else {
                    println!("WARN: {}: {}", r.path, r.issues.join(", "));
                }
            }
        }
        OutputFormat::Json => {
            #[derive(Serialize)]
            struct JsonOutput {
                total: usize,
                errors: usize,
                results: Vec<ValidationResult>,
            }
            let output = JsonOutput {
                total: results.len(),
                errors,
                results,
            };
            let json = serde_json::to_string_pretty(&output)
                .map_err(|e| format!("JSON serialization failed: {}", e))?;
            println!("{}", json);
        }
        OutputFormat::Yaml => {
            #[derive(Serialize)]
            struct YamlOutput {
                total: usize,
                errors: usize,
                results: Vec<ValidationResult>,
            }
            let output = YamlOutput {
                total: results.len(),
                errors,
                results,
            };
            let yaml = serde_yaml::to_string(&output)
                .map_err(|e| format!("YAML serialization failed: {}", e))?;
            print!("{}", yaml);
        }
    }

    if errors > 0 {
        return Err(format!("{} validation error(s)", errors));
    }

    Ok(())
}

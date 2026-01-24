//! Configuration introspection command.
//!
//! Provides `threads config` subcommands:
//! - show: Display resolved configuration
//! - env: List environment variables
//! - schema: Output JSON schema
//! - init: Create template manifest

use std::fs;
use std::path::Path;

use clap::{Args, Subcommand};

use crate::config::{
    self, CONFIG_DIR, Config, ConfigSource, ENV_VARS, MANIFEST_FILE, load_config,
    template_manifest, user_config_path,
};

#[derive(Args)]
pub struct ConfigArgs {
    #[command(subcommand)]
    command: ConfigCommand,
}

#[derive(Subcommand)]
enum ConfigCommand {
    /// Show resolved configuration
    Show(ShowArgs),

    /// List environment variables
    Env,

    /// Output JSON schema for manifest validation
    Schema,

    /// Create template manifest file
    Init(InitArgs),
}

#[derive(Args)]
struct ShowArgs {
    /// Show where each value came from
    #[arg(long)]
    effective: bool,
}

#[derive(Args)]
struct InitArgs {
    /// Directory to create manifest in (default: current directory)
    #[arg(default_value = ".")]
    path: String,

    /// Overwrite existing manifest
    #[arg(long)]
    force: bool,
}

pub fn run(args: ConfigArgs, ws: &Path) -> Result<(), String> {
    let cwd = std::env::current_dir().map_err(|e| format!("cannot get cwd: {}", e))?;

    match args.command {
        ConfigCommand::Show(show_args) => run_show(ws, &cwd, show_args.effective),
        ConfigCommand::Env => run_env(),
        ConfigCommand::Schema => run_schema(),
        ConfigCommand::Init(init_args) => run_init(&cwd, init_args),
    }
}

fn run_show(ws: &Path, cwd: &Path, effective: bool) -> Result<(), String> {
    let loaded = load_config(ws, cwd);

    if effective {
        print_effective(&loaded.config, &loaded.sources);
    } else {
        let yaml = serde_yaml::to_string(&loaded.config)
            .map_err(|e| format!("failed to serialize config: {}", e))?;
        println!("{}", yaml.trim());
    }

    Ok(())
}

fn print_effective(config: &Config, sources: &[ConfigSource]) {
    println!("# Resolved configuration");
    println!("# Sources (in order of precedence):");
    for source in sources {
        println!("#   - {}", source);
    }
    println!();

    // Print YAML with source annotations
    let yaml = serde_yaml::to_string(config).unwrap_or_default();
    print!("{}", yaml);
}

fn run_env() -> Result<(), String> {
    println!("Environment Variables:");
    println!();

    for var in ENV_VARS {
        println!("  {}", var.name);
        println!("    {}", var.description);
        if let Some(values) = var.values {
            println!("    Values: {}", values);
        }
        println!("    Default: {}", var.default);
        println!("    Config path: {}", var.config_path);
        println!();
    }

    Ok(())
}

fn run_schema() -> Result<(), String> {
    println!("{}", config::json_schema());
    Ok(())
}

fn run_init(cwd: &Path, args: InitArgs) -> Result<(), String> {
    let target_dir = if args.path == "." {
        cwd.to_path_buf()
    } else {
        cwd.join(&args.path)
    };

    let config_dir = target_dir.join(CONFIG_DIR);
    let manifest_path = config_dir.join(MANIFEST_FILE);

    // Check if already exists
    if manifest_path.exists() && !args.force {
        return Err(format!(
            "manifest already exists: {}\nUse --force to overwrite",
            manifest_path.display()
        ));
    }

    // Create config directory
    fs::create_dir_all(&config_dir)
        .map_err(|e| format!("failed to create {}: {}", config_dir.display(), e))?;

    // Write template
    fs::write(&manifest_path, template_manifest())
        .map_err(|e| format!("failed to write {}: {}", manifest_path.display(), e))?;

    println!("Created: {}", manifest_path.display());

    // Show user global config path as hint
    if let Some(user_path) = user_config_path()
        && !user_path.exists()
    {
        println!(
            "Hint: User global config can be placed at: {}",
            user_path.display()
        );
    }

    Ok(())
}

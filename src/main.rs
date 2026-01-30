use std::io;
use std::process;

use clap::{CommandFactory, Parser, Subcommand, ValueEnum};
use clap_complete::env::CompleteEnv;
use clap_complete::{Shell, generate};

mod args;
mod cache;
mod cmd;
mod config;
mod fuzzy;
mod git;
mod input;
mod output;
mod thread;
mod workspace;

#[derive(Parser)]
#[command(name = "threads")]
#[command(version = env!("THREADS_VERSION"))]
#[command(about = "Thread management for LLM workflows")]
#[command(
    long_about = "threads - Persistent context management for LLM-assisted development.\n\nThreads are markdown files in .threads/ directories at workspace, category,\nor project level. Each thread tracks a single topic: a feature, bug,\nexploration, or decision."
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// List threads
    #[command(alias = "ls")]
    List(cmd::list::ListArgs),

    /// Search thread content (fuzzy)
    Search(cmd::search::SearchArgs),

    /// Create a new thread
    New(cmd::new::NewArgs),

    /// Move thread to new location
    #[command(alias = "mv")]
    Move(cmd::move_cmd::MoveArgs),

    /// Validate thread files
    Validate(cmd::validate::ValidateArgs),

    /// Manage timestamp cache
    Cache(cmd::cache::CacheArgs),

    /// Git operations (status, commit)
    Git(cmd::git_cmd::GitArgs),

    /// Show thread count by status
    Stats(cmd::stats::StatsArgs),

    /// Read thread content
    #[command(alias = "cat", alias = "show")]
    Read(cmd::read::ReadArgs),

    /// Show thread info summary
    Info(cmd::info::InfoArgs),

    /// Print thread file path
    Path(cmd::path::PathArgs),

    /// Change thread status
    Status(cmd::status::StatusArgs),

    /// Update thread title/desc
    Update(cmd::update::UpdateArgs),

    /// Read or edit Body section
    Body(cmd::body::BodyArgs),

    /// Manage notes
    Note(cmd::note::NoteArgs),

    /// Manage todo items
    Todo(cmd::todo::TodoArgs),

    /// Add log entry
    Log(cmd::log::LogArgs),

    /// Mark thread closed
    #[command(alias = "resolve")]
    Close(cmd::resolve::ResolveArgs),

    /// Reopen resolved thread
    Reopen(cmd::reopen::ReopenArgs),

    /// Remove thread entirely
    #[command(alias = "rm")]
    Remove(cmd::remove::RemoveArgs),

    /// Generate shell completion script
    Completion(CompletionArgs),

    /// Configuration introspection
    Config(cmd::config_cmd::ConfigArgs),
}

#[derive(clap::Args)]
struct CompletionArgs {
    /// Shell to generate completions for
    #[arg(value_enum)]
    shell: CompletionShell,
}

#[derive(Clone, ValueEnum)]
enum CompletionShell {
    Bash,
    Zsh,
    Fish,
    Powershell,
}

fn main() {
    // Handle dynamic shell completions
    CompleteEnv::with_factory(Cli::command).complete();

    // Use try_parse to catch errors and normalize exit code
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(e) => {
            // Print the error (includes usage for missing args)
            let _ = e.print();
            // Exit with 0 for help/version, 1 for actual errors
            let exit_code = if e.kind() == clap::error::ErrorKind::DisplayHelp
                || e.kind() == clap::error::ErrorKind::DisplayVersion
            {
                0
            } else {
                1
            };
            process::exit(exit_code);
        }
    };

    // Handle completion before workspace lookup (doesn't need workspace)
    if let Commands::Completion(args) = &cli.command {
        let shell = match args.shell {
            CompletionShell::Bash => Shell::Bash,
            CompletionShell::Zsh => Shell::Zsh,
            CompletionShell::Fish => Shell::Fish,
            CompletionShell::Powershell => Shell::PowerShell,
        };
        generate(shell, &mut Cli::command(), "threads", &mut io::stdout());
        return;
    }

    // Find workspace
    let ws = match workspace::find() {
        Ok(ws) => ws,
        Err(e) => {
            eprintln!("workspace not found: {}", e);
            process::exit(1);
        }
    };

    // Load config
    let cwd = std::env::current_dir().unwrap_or_else(|_| ws.clone());
    let loaded_config = config::load_config(&ws, &cwd);

    let result = match cli.command {
        Commands::List(args) => cmd::list::run(args, &ws, &loaded_config.config),
        Commands::Search(args) => cmd::search::run(args, &ws, &loaded_config.config),
        Commands::New(args) => cmd::new::run(args, &ws, &loaded_config.config),
        Commands::Move(args) => cmd::move_cmd::run(args, &ws, &loaded_config.config),
        Commands::Validate(args) => cmd::validate::run(args, &ws, &loaded_config.config),
        Commands::Cache(args) => cmd::cache::run(args, &ws),
        Commands::Git(args) => cmd::git_cmd::run(args, &ws),
        Commands::Stats(args) => cmd::stats::run(args, &ws, &loaded_config.config),
        Commands::Read(args) => cmd::read::run(args, &ws),
        Commands::Info(args) => cmd::info::run(args, &ws),
        Commands::Path(args) => cmd::path::run(args, &ws),
        Commands::Status(args) => cmd::status::run(args, &ws, &loaded_config.config),
        Commands::Update(args) => cmd::update::run(args, &ws, &loaded_config.config),
        Commands::Body(args) => cmd::body::run(args, &ws, &loaded_config.config),
        Commands::Note(args) => cmd::note::run(args, &ws, &loaded_config.config),
        Commands::Todo(args) => cmd::todo::run(args, &ws, &loaded_config.config),
        Commands::Log(args) => cmd::log::run(args, &ws, &loaded_config.config),
        Commands::Close(args) => cmd::resolve::run(args, &ws, &loaded_config.config),
        Commands::Reopen(args) => cmd::reopen::run(args, &ws, &loaded_config.config),
        Commands::Remove(args) => cmd::remove::run(args, &ws, &loaded_config.config),
        Commands::Config(args) => cmd::config_cmd::run(args, &ws),
        Commands::Completion(_) => unreachable!(), // Handled above
    };

    if let Err(e) = result {
        eprintln!("{}", e);
        process::exit(1);
    }
}

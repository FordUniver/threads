use std::process;

use clap::{Parser, Subcommand};

mod cmd;
mod git;
mod thread;
mod workspace;

#[derive(Parser)]
#[command(name = "threads")]
#[command(about = "Thread management for LLM workflows")]
#[command(long_about = "threads - Persistent context management for LLM-assisted development.\n\nThreads are markdown files in .threads/ directories at workspace, category,\nor project level. Each thread tracks a single topic: a feature, bug,\nexploration, or decision.")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// List threads
    List(cmd::list::ListArgs),

    /// Create a new thread
    New(cmd::new::NewArgs),

    /// Move thread to new location
    Move(cmd::move_cmd::MoveArgs),

    /// Commit thread changes
    Commit(cmd::commit::CommitArgs),

    /// Validate thread files
    Validate(cmd::validate::ValidateArgs),

    /// Show pending thread changes
    Git(cmd::git_cmd::GitArgs),

    /// Show thread count by status
    Stats(cmd::stats::StatsArgs),

    /// Read thread content
    Read(cmd::read::ReadArgs),

    /// Change thread status
    Status(cmd::status::StatusArgs),

    /// Update thread title/desc
    Update(cmd::update::UpdateArgs),

    /// Edit Body section (stdin for content)
    Body(cmd::body::BodyArgs),

    /// Manage notes
    Note(cmd::note::NoteArgs),

    /// Manage todo items
    Todo(cmd::todo::TodoArgs),

    /// Add log entry
    Log(cmd::log::LogArgs),

    /// Mark thread resolved
    Resolve(cmd::resolve::ResolveArgs),

    /// Reopen resolved thread
    Reopen(cmd::reopen::ReopenArgs),

    /// Remove thread entirely
    #[command(alias = "rm")]
    Remove(cmd::remove::RemoveArgs),
}

fn main() {
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

    // Find workspace
    let ws = match workspace::find() {
        Ok(ws) => ws,
        Err(e) => {
            eprintln!("workspace not found: {}", e);
            process::exit(1);
        }
    };

    let result = match cli.command {
        Commands::List(args) => cmd::list::run(args, &ws),
        Commands::New(args) => cmd::new::run(args, &ws),
        Commands::Move(args) => cmd::move_cmd::run(args, &ws),
        Commands::Commit(args) => cmd::commit::run(args, &ws),
        Commands::Validate(args) => cmd::validate::run(args, &ws),
        Commands::Git(args) => cmd::git_cmd::run(args, &ws),
        Commands::Stats(args) => cmd::stats::run(args, &ws),
        Commands::Read(args) => cmd::read::run(args, &ws),
        Commands::Status(args) => cmd::status::run(args, &ws),
        Commands::Update(args) => cmd::update::run(args, &ws),
        Commands::Body(args) => cmd::body::run(args, &ws),
        Commands::Note(args) => cmd::note::run(args, &ws),
        Commands::Todo(args) => cmd::todo::run(args, &ws),
        Commands::Log(args) => cmd::log::run(args, &ws),
        Commands::Resolve(args) => cmd::resolve::run(args, &ws),
        Commands::Reopen(args) => cmd::reopen::run(args, &ws),
        Commands::Remove(args) => cmd::remove::run(args, &ws),
    };

    if let Err(e) = result {
        eprintln!("{}", e);
        process::exit(1);
    }
}

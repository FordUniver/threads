use std::io;
use std::process;

use clap::{CommandFactory, Parser, Subcommand, ValueEnum};
use clap_complete::env::CompleteEnv;
use clap_complete::{generate, Shell};

mod cmd;
mod git;
mod output;
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
    #[command(alias = "ls")]
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

    /// Print thread file path
    Path(cmd::path::PathArgs),

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

    /// Generate shell completion script
    Completion(CompletionArgs),
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

    let result = match cli.command {
        Commands::List(args) => cmd::list::run(args, &ws),
        Commands::New(args) => cmd::new::run(args, &ws),
        Commands::Move(args) => cmd::move_cmd::run(args, &ws),
        Commands::Commit(args) => cmd::commit::run(args, &ws),
        Commands::Validate(args) => cmd::validate::run(args, &ws),
        Commands::Git(args) => cmd::git_cmd::run(args, &ws),
        Commands::Stats(args) => cmd::stats::run(args, &ws),
        Commands::Read(args) => cmd::read::run(args, &ws),
        Commands::Path(args) => cmd::path::run(args, &ws),
        Commands::Status(args) => cmd::status::run(args, &ws),
        Commands::Update(args) => cmd::update::run(args, &ws),
        Commands::Body(args) => cmd::body::run(args, &ws),
        Commands::Note(args) => cmd::note::run(args, &ws),
        Commands::Todo(args) => cmd::todo::run(args, &ws),
        Commands::Log(args) => cmd::log::run(args, &ws),
        Commands::Resolve(args) => cmd::resolve::run(args, &ws),
        Commands::Reopen(args) => cmd::reopen::run(args, &ws),
        Commands::Remove(args) => cmd::remove::run(args, &ws),
        Commands::Completion(_) => unreachable!(), // Handled above
    };

    if let Err(e) = result {
        eprintln!("{}", e);
        process::exit(1);
    }
}

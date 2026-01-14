import ArgumentParser
import Foundation

struct Threads: ParsableCommand {
    static var configuration = CommandConfiguration(
        commandName: "threads",
        abstract: "Thread management for LLM workflows",
        discussion: """
            threads - Persistent context management for LLM-assisted development.

            Threads are markdown files in .threads/ directories at workspace, category,
            or project level. Each thread tracks a single topic: a feature, bug,
            exploration, or decision.
            """,
        subcommands: [
            ListCmd.self,
            NewCmd.self,
            MoveCmd.self,
            CommitCmd.self,
            ValidateCmd.self,
            GitCmd.self,
            StatsCmd.self,
            ReadCmd.self,
            PathCmd.self,
            StatusCmd.self,
            UpdateCmd.self,
            BodyCmd.self,
            NoteCmd.self,
            TodoCmd.self,
            LogCmd.self,
            ResolveCmd.self,
            ReopenCmd.self,
            RemoveCmd.self,
        ]
    )
}

// Custom main to control exit codes
@main
enum ThreadsMain {
    static func main() {
        do {
            var command = try Threads.parseAsRoot()
            try command.run()
        } catch let error as WorkspaceError {
            // Workspace errors (not found, ambiguous) -> exit 1
            if case .ambiguousReference = error {
                fputs("Error: \(error.localizedDescription)\n", stderr)
                exit(2)
            }
            fputs("Error: \(error.localizedDescription)\n", stderr)
            exit(1)
        } catch let error as ValidationError {
            // Validation errors from ArgumentParser -> exit 1
            fputs("Error: \(error.message)\n", stderr)
            exit(1)
        } catch let exitCode as ExitCode {
            // Explicit exit code request
            exit(exitCode.rawValue)
        } catch {
            // Check if this is a help request or version request
            let errorStr = "\(error)"
            if errorStr.contains("helpRequested") || errorStr.contains("versionRequested") {
                // Let ArgumentParser handle help/version output
                Threads.main()
                return
            }
            // All other errors (including argument parsing) -> exit 1
            if errorStr.contains("Usage:") || errorStr.contains("Error:") {
                // Already formatted
                fputs("\(error)\n", stderr)
            } else {
                fputs("Error: \(error.localizedDescription)\n", stderr)
            }
            exit(1)
        }
    }
}

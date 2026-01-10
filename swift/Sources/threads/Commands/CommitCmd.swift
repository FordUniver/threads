import ArgumentParser
import Foundation

struct CommitCmd: ParsableCommand {
    static var configuration = CommandConfiguration(
        commandName: "commit",
        abstract: "Commit thread changes",
        discussion: """
            Commit specific threads or all pending thread changes.

            Use --pending to commit all modified threads at once.
            """
    )

    @Flag(name: .long, help: "Commit all modified threads")
    var pending = false

    @Option(name: .shortAndLong, help: "Commit message")
    var m: String?

    @Flag(name: .long, help: "Auto-accept generated message")
    var auto = false

    @Argument(help: "Thread IDs to commit")
    var ids: [String] = []

    func run() throws {
        let ws = try getWorkspace()
        var files: [String] = []

        if pending {
            // Collect all thread files with uncommitted changes
            let threads = try findAllThreads(ws)

            for t in threads {
                let relPath = t.relativePath(from: ws) ?? t
                if gitHasChanges(ws, relPath) {
                    files.append(t)
                }
            }
        } else {
            // Resolve provided IDs to files
            if ids.isEmpty {
                throw ValidationError("provide thread IDs or use --pending")
            }

            for id in ids {
                let file = try findByRef(ws, id)
                let relPath = file.relativePath(from: ws) ?? file
                if !gitHasChanges(ws, relPath) {
                    fputs("No changes in thread: \(id)\n", stderr)
                    continue
                }
                files.append(file)
            }
        }

        if files.isEmpty {
            print("No threads to commit.")
            return
        }

        // Generate commit message if not provided
        var msg = m
        if msg == nil {
            let generatedMsg = generateCommitMessage(ws, files)
            print("Generated message: \(generatedMsg)")

            if !auto && isTerminal() {
                print("Proceed? [Y/n] ", terminator: "")
                if let response = readLine()?.trimmingCharacters(in: .whitespaces).lowercased() {
                    if response == "n" || response == "no" {
                        print("Aborted.")
                        return
                    }
                }
            }

            msg = generatedMsg
        }

        // Stage and commit
        let relPaths = files.map { $0.relativePath(from: ws) ?? $0 }

        try gitCommit(ws, relPaths, msg!)

        print("Committed \(files.count) thread(s).")
    }
}

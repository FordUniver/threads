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
                var relPath = t
                if t.hasPrefix(ws) {
                    relPath = String(t.dropFirst(ws.count))
                    if relPath.hasPrefix("/") {
                        relPath = String(relPath.dropFirst())
                    }
                }
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
                var relPath = file
                if file.hasPrefix(ws) {
                    relPath = String(file.dropFirst(ws.count))
                    if relPath.hasPrefix("/") {
                        relPath = String(relPath.dropFirst())
                    }
                }
                if !gitHasChanges(ws, relPath) {
                    print("No changes in thread: \(id)")
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
        var relPaths: [String] = []
        for f in files {
            var relPath = f
            if f.hasPrefix(ws) {
                relPath = String(f.dropFirst(ws.count))
                if relPath.hasPrefix("/") {
                    relPath = String(relPath.dropFirst())
                }
            }
            relPaths.append(relPath)
        }

        try gitCommit(ws, relPaths, msg!)

        do {
            try gitPush(ws)
        } catch {
            print("WARNING: git push failed (commit succeeded): \(error.localizedDescription)")
        }

        print("Committed \(files.count) thread(s).")
    }
}

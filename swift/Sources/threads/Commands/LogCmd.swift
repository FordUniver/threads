import ArgumentParser
import Foundation

struct LogCmd: ParsableCommand {
    static var configuration = CommandConfiguration(
        commandName: "log",
        abstract: "Add log entry",
        discussion: "Add a timestamped entry to the Log section."
    )

    @Flag(name: .long, help: "Commit after adding")
    var commit = false

    @Option(name: .shortAndLong, help: "Commit message")
    var m: String?

    @Argument(help: "Thread ID or name")
    var id: String

    @Argument(help: "Log entry text")
    var entry: String?

    func run() throws {
        let ws = try getWorkspace()

        var logEntry = entry

        // Read entry from stdin if not provided
        if logEntry == nil || logEntry!.isEmpty {
            if let stdinContent = readStdin() {
                logEntry = stdinContent
            }
        }

        guard let finalEntry = logEntry, !finalEntry.isEmpty else {
            throw ValidationError("no log entry provided")
        }

        let file = try findByRef(ws, id)
        let t = try Thread.parse(path: file)

        t.content = insertLogEntry(t.content, finalEntry)

        try t.write()

        print("Logged to: \(file)")

        if commit {
            let msg = m ?? generateCommitMessage(ws, [file])
            try gitAutoCommit(ws, file, msg)
        } else {
            print("Note: Thread \(id) has uncommitted changes. Use 'threads commit \(id)' when ready.")
        }
    }
}

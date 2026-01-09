import ArgumentParser
import Foundation

struct ResolveCmd: ParsableCommand {
    static var configuration = CommandConfiguration(
        commandName: "resolve",
        abstract: "Mark thread resolved"
    )

    @Flag(name: .long, help: "Commit after resolving")
    var commit = false

    @Option(name: .shortAndLong, help: "Commit message")
    var m: String?

    @Argument(help: "Thread ID or name")
    var id: String

    func run() throws {
        let ws = try getWorkspace()

        let file = try findByRef(ws, id)
        let t = try Thread.parse(path: file)

        let oldStatus = t.status

        // Update status
        try t.setFrontmatterField("status", "resolved")

        // Add log entry
        t.content = insertLogEntry(t.content, "Resolved.")

        try t.write()

        print("Resolved: \(oldStatus) → resolved (\(file))")

        if commit {
            let msg = m ?? generateCommitMessage(ws, [file])
            try gitAutoCommit(ws, file, msg)
        } else {
            print("Note: Thread \(id) has uncommitted changes. Use 'threads commit \(id)' when ready.")
        }
    }
}

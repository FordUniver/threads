import ArgumentParser
import Foundation

struct ReopenCmd: ParsableCommand {
    static var configuration = CommandConfiguration(
        commandName: "reopen",
        abstract: "Reopen resolved thread"
    )

    @Option(name: .long, help: "Status to reopen to")
    var status: String = "active"

    @Flag(name: .long, help: "Commit after reopening")
    var commit = false

    @Option(name: .shortAndLong, help: "Commit message")
    var m: String?

    @Argument(help: "Thread ID or name")
    var id: String

    func run() throws {
        let ws = try getWorkspace()

        guard Thread.isValidStatus(status) else {
            throw ValidationError("Invalid status '\(status)'. Must be one of: idea, planning, active, blocked, paused, resolved, superseded, deferred, rejected")
        }

        let file = try findByRef(ws, id)
        let t = try Thread.parse(path: file)

        let oldStatus = t.status

        // Update status
        try t.setFrontmatterField("status", status)

        // Add log entry
        t.content = insertLogEntry(t.content, "Reopened.")

        try t.write()

        print("Reopened: \(oldStatus) → \(status) (\(file))")

        if commit {
            let msg = m ?? generateCommitMessage(ws, [file])
            try gitAutoCommit(ws, file, msg)
        } else {
            print("Note: Thread \(id) has uncommitted changes. Use 'threads commit \(id)' when ready.")
        }
    }
}

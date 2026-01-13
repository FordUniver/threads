import ArgumentParser
import Foundation

struct StatusCmd: ParsableCommand {
    static var configuration = CommandConfiguration(
        commandName: "status",
        abstract: "Change thread status"
    )

    @Flag(name: .long, help: "Commit after changing")
    var commit = false

    @Option(name: .shortAndLong, help: "Commit message")
    var m: String?

    @Argument(help: "Thread ID or name")
    var id: String

    @Argument(help: "New status")
    var newStatus: String

    func run() throws {
        let ws = try getWorkspace()

        guard Thread.isValidStatus(newStatus) else {
            throw ValidationError("Invalid status '\(newStatus)'. Must be one of: idea, planning, active, blocked, paused, resolved, superseded, deferred, rejected")
        }

        let file = try findByRef(ws, id)
        let t = try Thread.parse(path: file)

        let oldStatus = t.status

        try t.setFrontmatterField("status", newStatus)
        try t.write()

        print("Status changed: \(oldStatus) → \(newStatus) (\(file))")

        if commit {
            let msg = m ?? generateCommitMessage(ws, [file])
            try gitAutoCommit(ws, file, msg)
        } else {
            print("Note: Thread \(id) has uncommitted changes. Use 'threads commit \(id)' when ready.")
        }
    }
}

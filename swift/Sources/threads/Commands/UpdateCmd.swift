import ArgumentParser
import Foundation

struct UpdateCmd: ParsableCommand {
    static var configuration = CommandConfiguration(
        commandName: "update",
        abstract: "Update thread title/desc"
    )

    @Option(name: .long, help: "New title")
    var title: String?

    @Option(name: .long, help: "New description")
    var desc: String?

    @Flag(name: .long, help: "Commit after updating")
    var commit = false

    @Option(name: .shortAndLong, help: "Commit message")
    var m: String?

    @Argument(help: "Thread ID or name")
    var id: String

    func run() throws {
        let ws = try getWorkspace()

        if title == nil && desc == nil {
            throw ValidationError("specify --title and/or --desc")
        }

        let file = try findByRef(ws, id)
        let t = try Thread.parse(path: file)

        if let newTitle = title {
            try t.setFrontmatterField("name", newTitle)
            print("Title updated: \(newTitle)")
        }

        if let newDesc = desc {
            try t.setFrontmatterField("desc", newDesc)
            print("Description updated: \(newDesc)")
        }

        try t.write()
        print("Updated: \(file)")

        if commit {
            let msg = m ?? generateCommitMessage(ws, [file])
            try gitAutoCommit(ws, file, msg)
        } else {
            print("Note: Thread \(id) has uncommitted changes. Use 'threads commit \(id)' when ready.")
        }
    }
}

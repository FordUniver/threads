import ArgumentParser
import Foundation

struct BodyCmd: ParsableCommand {
    static var configuration = CommandConfiguration(
        commandName: "body",
        abstract: "Edit Body section (stdin for content)",
        discussion: """
            Edit the Body section of a thread.

            Content is read from stdin. Use --set to replace or --append to add.
            """
    )

    @Flag(name: .long, help: "Replace body content")
    var set = false

    @Flag(name: .long, help: "Append to body content")
    var append = false

    @Flag(name: .long, help: "Commit after editing")
    var commit = false

    @Option(name: .shortAndLong, help: "Commit message")
    var m: String?

    @Argument(help: "Thread ID or name")
    var id: String

    func run() throws {
        let ws = try getWorkspace()

        // Default to set mode
        var useSet = set
        if !set && !append {
            useSet = true
        }

        // Read content from stdin
        guard let content = readStdin(), !content.isEmpty else {
            throw ValidationError("no content provided (use stdin)")
        }

        let file = try findByRef(ws, id)
        let t = try Thread.parse(path: file)

        if useSet {
            t.content = replaceSection(t.content, "Body", content)
        } else {
            t.content = appendToSection(t.content, "Body", content)
        }

        try t.write()

        let mode = append ? "append" : "set"
        print("Body \(mode): \(file)")

        if commit {
            let msg = m ?? generateCommitMessage(ws, [file])
            try gitAutoCommit(ws, file, msg)
        } else {
            print("Note: Thread \(id) has uncommitted changes. Use 'threads commit \(id)' when ready.")
        }
    }
}

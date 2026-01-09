import ArgumentParser
import Foundation

struct NoteCmd: ParsableCommand {
    static var configuration = CommandConfiguration(
        commandName: "note",
        abstract: "Manage notes",
        discussion: """
            Manage notes in the Notes section.

            Actions:
              add <text>           Add a new note
              edit <hash> <text>   Edit a note by hash
              remove <hash>        Remove a note by hash
            """
    )

    @Flag(name: .long, help: "Commit after editing")
    var commit = false

    @Option(name: .shortAndLong, help: "Commit message")
    var m: String?

    @Argument(help: "Thread ID, action, and arguments")
    var args: [String] = []

    func run() throws {
        guard args.count >= 2 else {
            throw ValidationError("usage: threads note <id> <action> [args...]")
        }

        let ws = try getWorkspace()
        let ref = args[0]
        let action = args[1]

        let file = try findByRef(ws, ref)
        let t = try Thread.parse(path: file)

        var logEntry: String = ""

        switch action {
        case "add":
            guard args.count >= 3 else {
                throw ValidationError("usage: threads note <id> add \"text\"")
            }
            let text = args[2]

            let (newContent, hash) = addNote(t.content, text)
            t.content = newContent

            // Add log entry
            logEntry = "Added note: \(text)"
            t.content = insertLogEntry(t.content, logEntry)

            print("Added note: \(text) (id: \(hash))")

        case "edit":
            guard args.count >= 4 else {
                throw ValidationError("usage: threads note <id> edit <hash> \"new text\"")
            }
            let hash = args[2]
            let newText = args[3]

            // Check for ambiguous hash
            let count = countMatchingItems(t.content, "Notes", hash)
            if count == 0 {
                throw ValidationError("no note with hash '\(hash)' found")
            }
            if count > 1 {
                throw ValidationError("ambiguous hash '\(hash)' matches \(count) notes")
            }

            t.content = try editByHash(t.content, "Notes", hash, newText)

            logEntry = "Edited note \(hash)"
            t.content = insertLogEntry(t.content, logEntry)

            print("Edited note \(hash)")

        case "remove":
            guard args.count >= 3 else {
                throw ValidationError("usage: threads note <id> remove <hash>")
            }
            let hash = args[2]

            // Check for ambiguous hash
            let count = countMatchingItems(t.content, "Notes", hash)
            if count == 0 {
                throw ValidationError("no note with hash '\(hash)' found")
            }
            if count > 1 {
                throw ValidationError("ambiguous hash '\(hash)' matches \(count) notes")
            }

            t.content = try removeByHash(t.content, "Notes", hash)

            logEntry = "Removed note \(hash)"
            t.content = insertLogEntry(t.content, logEntry)

            print("Removed note \(hash)")

        default:
            throw ValidationError("unknown action '\(action)'. Use: add, edit, remove")
        }

        try t.write()

        if commit {
            let msg = m ?? generateCommitMessage(ws, [file])
            try gitAutoCommit(ws, file, msg)
        } else {
            print("Note: Thread \(ref) has uncommitted changes. Use 'threads commit \(ref)' when ready.")
        }
    }
}

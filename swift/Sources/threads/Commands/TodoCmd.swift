import ArgumentParser
import Foundation

struct TodoCmd: ParsableCommand {
    static var configuration = CommandConfiguration(
        commandName: "todo",
        abstract: "Manage todo items",
        discussion: """
            Manage todo items in the Todo section.

            Actions:
              add <text>     Add a new todo item
              check <hash>   Mark item as checked
              uncheck <hash> Mark item as unchecked
              remove <hash>  Remove item
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
            throw ValidationError("usage: threads todo <id> <action> [args...]")
        }

        let ws = try getWorkspace()
        let ref = args[0]
        let action = args[1]

        let file = try findByRef(ws, ref)
        let t = try Thread.parse(path: file)

        switch action {
        case "add":
            guard args.count >= 3 else {
                throw ValidationError("usage: threads todo <id> add \"item text\"")
            }
            let text = args[2]

            let (newContent, hash) = addTodoItem(t.content, text)
            t.content = newContent

            print("Added to Todo: \(text) (id: \(hash))")

        case "check", "complete", "done":
            guard args.count >= 3 else {
                throw ValidationError("usage: threads todo <id> check <hash>")
            }
            let hash = args[2]

            // Check for ambiguous hash
            let count = countMatchingItems(t.content, "Todo", hash)
            if count == 0 {
                throw ValidationError("no unchecked item with hash '\(hash)' found")
            }
            if count > 1 {
                throw ValidationError("ambiguous hash '\(hash)' matches \(count) items")
            }

            t.content = try setTodoChecked(t.content, hash, true)

            print("Checked item \(hash)")

        case "uncheck":
            guard args.count >= 3 else {
                throw ValidationError("usage: threads todo <id> uncheck <hash>")
            }
            let hash = args[2]

            // Check for ambiguous hash
            let count = countMatchingItems(t.content, "Todo", hash)
            if count == 0 {
                throw ValidationError("no checked item with hash '\(hash)' found")
            }
            if count > 1 {
                throw ValidationError("ambiguous hash '\(hash)' matches \(count) items")
            }

            t.content = try setTodoChecked(t.content, hash, false)

            print("Unchecked item \(hash)")

        case "remove":
            guard args.count >= 3 else {
                throw ValidationError("usage: threads todo <id> remove <hash>")
            }
            let hash = args[2]

            // Check for ambiguous hash
            let count = countMatchingItems(t.content, "Todo", hash)
            if count == 0 {
                throw ValidationError("no item with hash '\(hash)' found")
            }
            if count > 1 {
                throw ValidationError("ambiguous hash '\(hash)' matches \(count) items")
            }

            t.content = try removeByHash(t.content, "Todo", hash)

            print("Removed item \(hash)")

        default:
            throw ValidationError("unknown action '\(action)'. Use: add, check, uncheck, remove")
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

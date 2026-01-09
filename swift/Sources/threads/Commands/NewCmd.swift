import ArgumentParser
import Foundation

struct NewCmd: ParsableCommand {
    static var configuration = CommandConfiguration(
        commandName: "new",
        abstract: "Create a new thread",
        discussion: """
            Create a new thread at the specified level.

            If path is omitted, the level is inferred from the current directory.
            Use "." for workspace level.
            """
    )

    @Option(name: .long, help: "Initial status")
    var status: String = "idea"

    @Option(name: .long, help: "One-line description")
    var desc: String = ""

    @Option(name: .long, help: "Initial body content")
    var body: String = ""

    @Flag(name: .long, help: "Commit after creating")
    var commit = false

    @Option(name: .shortAndLong, help: "Commit message")
    var m: String?

    @Argument(help: "Path (optional) and title")
    var args: [String] = []

    mutating func run() throws {
        let ws = try getWorkspace()

        var path: String
        var title: String

        if args.count == 2 {
            path = args[0]
            title = args[1]
        } else if args.count == 1 {
            title = args[0]
            // Infer path from cwd
            path = FileManager.default.currentDirectoryPath
        } else {
            throw ValidationError("Usage: threads new [path] <title>")
        }

        if title.isEmpty {
            throw ValidationError("title is required")
        }

        // Warn if no description provided
        if desc.isEmpty {
            printError("Warning: No --desc provided. Add one with: threads update <id> --desc \"...\"")
        }

        // Slugify title
        let slug = slugify(title)
        if slug.isEmpty {
            throw ValidationError("title produces empty slug")
        }

        // Read body from stdin if available and not provided via flag
        var bodyContent = body
        if bodyContent.isEmpty {
            if let stdinContent = readStdin() {
                bodyContent = stdinContent
            }
        }

        // Determine scope
        let scope = try inferScope(ws, path)

        // Generate ID
        let id = try generateID(ws)

        // Ensure threads directory exists
        try FileManager.default.createDirectory(atPath: scope.threadsDir, withIntermediateDirectories: true)

        // Build file path
        let filename = "\(id)-\(slug).md"
        let threadPath = (scope.threadsDir as NSString).appendingPathComponent(filename)

        // Check if file already exists
        if FileManager.default.fileExists(atPath: threadPath) {
            throw ValidationError("thread already exists: \(threadPath)")
        }

        // Generate content
        let dateFormatter = DateFormatter()
        dateFormatter.dateFormat = "yyyy-MM-dd"
        let today = dateFormatter.string(from: Date())

        dateFormatter.dateFormat = "HH:mm"
        let timestamp = dateFormatter.string(from: Date())

        var content = """
            ---
            id: \(id)
            name: \(title)
            desc: \(desc)
            status: \(status)
            ---

            """

        if !bodyContent.isEmpty {
            content += bodyContent
            if !bodyContent.hasSuffix("\n") {
                content += "\n"
            }
            content += "\n"
        }

        content += """
            ## Todo

            ## Log

            ### \(today)

            - **\(timestamp)** Created thread.
            """

        // Write file
        try content.write(toFile: threadPath, atomically: true, encoding: .utf8)

        var relPath = threadPath
        if threadPath.hasPrefix(ws) {
            relPath = String(threadPath.dropFirst(ws.count))
            if relPath.hasPrefix("/") {
                relPath = String(relPath.dropFirst())
            }
        }

        print("Created \(scope.levelDesc): \(id)")
        print("  → \(relPath)")

        if bodyContent.isEmpty {
            printError("Hint: Add body with: echo \"content\" | threads body \(id) --set")
        }

        // Commit if requested
        if commit {
            let msg = m ?? generateCommitMessage(ws, [threadPath])
            try gitAutoCommit(ws, threadPath, msg)
        } else {
            print("Note: Thread \(id) has uncommitted changes. Use 'threads commit \(id)' when ready.")
        }
    }
}

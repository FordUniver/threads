import ArgumentParser
import Foundation

struct RemoveCmd: ParsableCommand {
    static var configuration = CommandConfiguration(
        commandName: "remove",
        abstract: "Remove thread entirely",
        aliases: ["rm"]
    )

    @Flag(name: .long, help: "Commit after removing")
    var commit = false

    @Option(name: .shortAndLong, help: "Commit message")
    var m: String?

    @Argument(help: "Thread ID or name")
    var id: String

    func run() throws {
        let ws = try getWorkspace()

        let file = try findByRef(ws, id)
        let t = try Thread.parse(path: file)

        let name = t.name
        let relPath = file.relativePath(from: ws) ?? file

        // Check if file is tracked
        let wasTracked = gitIsTracked(ws, relPath)

        // Remove file
        try FileManager.default.removeItem(atPath: file)

        print("Removed: \(file)")

        if !wasTracked {
            print("Note: Thread was never committed to git, no commit needed.")
            return
        }

        if commit {
            let msg = m ?? "threads: remove '\(name)'"
            try gitAdd(ws, [relPath])
            try gitCommit(ws, [relPath], msg)
            do {
                try gitPush(ws)
            } catch {
                print("WARNING: git push failed (commit succeeded): \(error.localizedDescription)")
            }
        } else {
            print("Note: To commit this removal, run:")
            print("  git -C \"$WORKSPACE\" add \"\(relPath)\" && git -C \"$WORKSPACE\" commit -m \"threads: remove '\(name)'\"")
        }
    }
}

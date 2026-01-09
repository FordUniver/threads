import ArgumentParser
import Foundation

struct GitCmd: ParsableCommand {
    static var configuration = CommandConfiguration(
        commandName: "git",
        abstract: "Show pending thread changes"
    )

    func run() throws {
        let ws = try getWorkspace()

        let threads = try findAllThreads(ws)

        var modified: [String] = []
        for t in threads {
            var relPath = t
            if t.hasPrefix(ws) {
                relPath = String(t.dropFirst(ws.count))
                if relPath.hasPrefix("/") {
                    relPath = String(relPath.dropFirst())
                }
            }
            if gitHasChanges(ws, relPath) {
                modified.append(relPath)
            }
        }

        if modified.isEmpty {
            print("No pending thread changes.")
            return
        }

        print("Pending thread changes:")
        for f in modified {
            print("  \(f)")
        }
        print()
        print("Suggested:")
        print("  git add \(modified.joined(separator: " ")) && git commit -m \"threads: update\" && git push")
    }
}

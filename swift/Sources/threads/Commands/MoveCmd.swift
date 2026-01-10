import ArgumentParser
import Foundation

struct MoveCmd: ParsableCommand {
    static var configuration = CommandConfiguration(
        commandName: "move",
        abstract: "Move thread to new location"
    )

    @Flag(name: .long, help: "Commit after moving")
    var commit = false

    @Option(name: .shortAndLong, help: "Commit message")
    var m: String?

    @Argument(help: "Thread ID or name")
    var id: String

    @Argument(help: "New path")
    var newPath: String

    func run() throws {
        let ws = try getWorkspace()

        // Find source thread
        let srcFile = try findByRef(ws, id)

        // Resolve destination scope
        let scope = try inferScope(ws, newPath)

        // Ensure dest .threads/ exists
        try FileManager.default.createDirectory(atPath: scope.threadsDir, withIntermediateDirectories: true)

        // Move file
        let filename = (srcFile as NSString).lastPathComponent
        let destFile = (scope.threadsDir as NSString).appendingPathComponent(filename)

        if FileManager.default.fileExists(atPath: destFile) {
            throw ValidationError("thread already exists at destination: \(destFile)")
        }

        try FileManager.default.moveItem(atPath: srcFile, toPath: destFile)

        let relDest = destFile.relativePath(from: ws) ?? destFile

        print("Moved to \(scope.levelDesc)")
        print("  → \(relDest)")

        // Commit if requested
        if commit {
            let relSrc = srcFile.relativePath(from: ws) ?? srcFile

            try gitAdd(ws, [relSrc, relDest])
            let msg = m ?? "threads: move \((srcFile as NSString).lastPathComponent) to \(scope.levelDesc)"
            try gitCommit(ws, [relSrc, relDest], msg)
            do {
                try gitPush(ws)
            } catch {
                print("WARNING: git push failed (commit succeeded): \(error.localizedDescription)")
            }
        } else {
            print("Note: Use --commit to commit this move")
        }
    }
}

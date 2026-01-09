import ArgumentParser
import Foundation

struct ValidateCmd: ParsableCommand {
    static var configuration = CommandConfiguration(
        commandName: "validate",
        abstract: "Validate thread files"
    )

    @Flag(name: .shortAndLong, help: "Validate recursively")
    var recursive = false

    @Argument(help: "Path to validate")
    var path: String?

    func run() throws {
        let ws = try getWorkspace()
        var files: [String]

        if let target = path {
            // Check if it's a file path
            var absPath = target
            if !(target as NSString).isAbsolutePath {
                absPath = (ws as NSString).appendingPathComponent(target)
            }

            var isDir: ObjCBool = false
            if FileManager.default.fileExists(atPath: absPath, isDirectory: &isDir), isDir.boolValue {
                // It's a directory - find threads under it
                files = try findAllThreads(ws).filter { $0.hasPrefix(absPath) }
            } else {
                files = [absPath]
            }
        } else if recursive {
            // All threads in workspace
            files = try findAllThreads(ws)
        } else {
            // Only workspace-level threads
            let threadsDir = (ws as NSString).appendingPathComponent(".threads")
            files = []
            if FileManager.default.fileExists(atPath: threadsDir),
               let fileList = try? FileManager.default.contentsOfDirectory(atPath: threadsDir) {
                for file in fileList where file.hasSuffix(".md") {
                    files.append((threadsDir as NSString).appendingPathComponent(file))
                }
            }
        }

        var errorCount = 0

        for file in files {
            var relPath = file
            if file.hasPrefix(ws) {
                relPath = String(file.dropFirst(ws.count))
                if relPath.hasPrefix("/") {
                    relPath = String(relPath.dropFirst())
                }
            }

            var issues: [String] = []

            do {
                let t = try Thread.parse(path: file)

                // Note: ID can be derived from filename, so we don't check for empty ID
                // The Thread.parse method already extracts ID from filename

                if t.name.isEmpty {
                    issues.append("missing name/title field")
                }
                if t.status.isEmpty {
                    issues.append("missing status field")
                } else if !Thread.isValidStatus(t.status) {
                    issues.append("invalid status '\(Thread.baseStatus(t.status))'")
                }
            } catch {
                issues.append("parse error: \(error.localizedDescription)")
            }

            if !issues.isEmpty {
                print("WARN: \(relPath): \(issues.joined(separator: ", "))")
                errorCount += 1
            } else {
                print("OK: \(relPath)")
            }
        }

        if errorCount > 0 {
            throw ExitCode(1)
        }
    }
}

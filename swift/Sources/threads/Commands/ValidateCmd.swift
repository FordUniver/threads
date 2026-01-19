import ArgumentParser
import Foundation
import Yams

struct ValidateCmd: ParsableCommand {
    static var configuration = CommandConfiguration(
        commandName: "validate",
        abstract: "Validate thread files"
    )

    @Flag(name: .shortAndLong, help: "Validate recursively")
    var recursive = false

    @Option(name: .shortAndLong, help: "Output format (json, yaml, plain)")
    var format: String?

    @Flag(name: .long, help: "Output as JSON (shorthand for --format=json)")
    var json = false

    @Argument(help: "Path to validate")
    var path: String?

    func run() throws {
        let ws = try getWorkspace()
        let fmt = json ? "json" : (format?.lowercased() ?? "fancy")
        var files: [String]

        if let target = path {
            // Check if it's a file path
            var absPath = target
            if !(target as NSString).isAbsolutePath {
                absPath = (ws as NSString).appendingPathComponent(target)
            }

            var isDir: ObjCBool = false
            if FileManager.default.fileExists(atPath: absPath, isDirectory: &isDir), isDir.boolValue {
                // It's a directory - find threads under it (use secure containment check)
                files = try findAllThreads(ws).filter { isContained($0, in: absPath) }
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

        struct ValidationResult {
            let path: String
            let valid: Bool
            let issues: [String]
        }

        var results: [ValidationResult] = []
        var errorCount = 0

        for file in files {
            let relPath = file.relativePath(from: ws) ?? file
            var issues: [String] = []

            do {
                let t = try Thread.parse(path: file)

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

            let valid = issues.isEmpty
            if !valid {
                errorCount += 1
            }

            results.append(ValidationResult(path: relPath, valid: valid, issues: issues))
        }

        // Output based on format
        switch fmt {
        case "json":
            let resultsArray = results.map { r -> [String: Any] in
                ["path": r.path, "valid": r.valid, "issues": r.issues]
            }
            let data: [String: Any] = ["total": results.count, "errors": errorCount, "results": resultsArray]
            if let jsonData = try? JSONSerialization.data(withJSONObject: data, options: [.prettyPrinted, .sortedKeys]),
               let output = String(data: jsonData, encoding: .utf8) {
                print(output)
            }
        case "yaml":
            let resultsArray = results.map { r -> [String: Any] in
                ["path": r.path, "valid": r.valid, "issues": r.issues]
            }
            let data: [String: Any] = ["total": results.count, "errors": errorCount, "results": resultsArray]
            if let output = try? Yams.dump(object: data) {
                print(output, terminator: "")
            }
        default:
            for r in results {
                if r.valid {
                    print("OK: \(r.path)")
                } else {
                    print("WARN: \(r.path): \(r.issues.joined(separator: ", "))")
                }
            }
        }

        if errorCount > 0 {
            throw ExitCode(1)
        }
    }
}

import Foundation

// MARK: - Batched Git Status

/// Cached result of git status --porcelain for a workspace
private var cachedGitStatus: (ws: String, files: Set<String>)?

/// gitStatusBatch runs `git status --porcelain` once and returns set of files with changes.
/// Results are cached per workspace for the duration of the process.
func gitStatusBatch(_ ws: String) -> Set<String> {
    // Return cached result if same workspace
    if let cached = cachedGitStatus, cached.ws == ws {
        return cached.files
    }

    let process = Process()
    process.executableURL = URL(fileURLWithPath: "/usr/bin/git")
    process.arguments = ["-C", ws, "status", "--porcelain", "-uall"]

    let pipe = Pipe()
    process.standardOutput = pipe
    process.standardError = FileHandle.nullDevice

    do {
        try process.run()
        process.waitUntilExit()
    } catch {
        return []
    }

    let data = pipe.fileHandleForReading.readDataToEndOfFile()
    guard let output = String(data: data, encoding: .utf8) else {
        return []
    }

    // Parse porcelain output: "XY filename" or "XY filename -> newname"
    var files = Set<String>()
    for line in output.split(separator: "\n", omittingEmptySubsequences: true) {
        // Skip the status indicator (first 3 chars: "XY ")
        guard line.count > 3 else { continue }
        let idx = line.index(line.startIndex, offsetBy: 3)
        var path = String(line[idx...])

        // Handle renames: "R  old -> new"
        if let arrowRange = path.range(of: " -> ") {
            // Both old and new paths have changes
            files.insert(String(path[..<arrowRange.lowerBound]))
            path = String(path[arrowRange.upperBound...])
        }

        files.insert(path)
    }

    cachedGitStatus = (ws, files)
    return files
}

/// Clears the cached git status (call when files are modified)
func gitStatusClearCache() {
    cachedGitStatus = nil
}

// hasChanges checks if a file has uncommitted changes (staged, unstaged, or untracked)
// Uses batched git status for efficiency when checking multiple files.
func gitHasChanges(_ ws: String, _ relPath: String) -> Bool {
    let changedFiles = gitStatusBatch(ws)
    return changedFiles.contains(relPath)
}

// isTracked checks if a file is tracked by git
func gitIsTracked(_ ws: String, _ relPath: String) -> Bool {
    let process = Process()
    process.executableURL = URL(fileURLWithPath: "/usr/bin/git")
    process.arguments = ["-C", ws, "ls-files", "--error-unmatch", relPath]
    process.standardOutput = FileHandle.nullDevice
    process.standardError = FileHandle.nullDevice
    do {
        try process.run()
        process.waitUntilExit()
        return process.terminationStatus == 0
    } catch {
        return false
    }
}

// existsInHEAD checks if a file exists in HEAD
func gitExistsInHEAD(_ ws: String, _ relPath: String) -> Bool {
    let ref = "HEAD:\(relPath)"
    let process = Process()
    process.executableURL = URL(fileURLWithPath: "/usr/bin/git")
    process.arguments = ["-C", ws, "cat-file", "-e", ref]
    process.standardOutput = FileHandle.nullDevice
    process.standardError = FileHandle.nullDevice
    do {
        try process.run()
        process.waitUntilExit()
        return process.terminationStatus == 0
    } catch {
        return false
    }
}

// gitAdd stages a file
func gitAdd(_ ws: String, _ files: [String]) throws {
    let process = Process()
    process.executableURL = URL(fileURLWithPath: "/usr/bin/git")
    process.arguments = ["-C", ws, "add"] + files

    let pipe = Pipe()
    process.standardOutput = pipe
    process.standardError = pipe

    try process.run()
    process.waitUntilExit()

    if process.terminationStatus != 0 {
        let data = pipe.fileHandleForReading.readDataToEndOfFile()
        let output = String(data: data, encoding: .utf8) ?? ""
        throw GitError.addFailed(output)
    }
}

// gitCommit creates a commit with the given message
func gitCommit(_ ws: String, _ files: [String], _ message: String) throws {
    // Stage files
    try gitAdd(ws, files)

    // Commit
    let process = Process()
    process.executableURL = URL(fileURLWithPath: "/usr/bin/git")
    process.arguments = ["-C", ws, "commit", "-m", message] + files

    let pipe = Pipe()
    process.standardOutput = pipe
    process.standardError = pipe

    try process.run()
    process.waitUntilExit()

    // Clear status cache since files were committed
    gitStatusClearCache()

    if process.terminationStatus != 0 {
        let data = pipe.fileHandleForReading.readDataToEndOfFile()
        let output = String(data: data, encoding: .utf8) ?? ""
        throw GitError.commitFailed(output)
    }
}

// gitPush does git pull --rebase && git push
func gitPush(_ ws: String) throws {
    // Pull with rebase
    let pull = Process()
    pull.executableURL = URL(fileURLWithPath: "/usr/bin/git")
    pull.arguments = ["-C", ws, "pull", "--rebase"]

    let pullPipe = Pipe()
    pull.standardOutput = pullPipe
    pull.standardError = pullPipe

    try pull.run()
    pull.waitUntilExit()

    if pull.terminationStatus != 0 {
        let data = pullPipe.fileHandleForReading.readDataToEndOfFile()
        let output = String(data: data, encoding: .utf8) ?? ""
        throw GitError.pullFailed(output)
    }

    // Push
    let push = Process()
    push.executableURL = URL(fileURLWithPath: "/usr/bin/git")
    push.arguments = ["-C", ws, "push"]

    let pushPipe = Pipe()
    push.standardOutput = pushPipe
    push.standardError = pushPipe

    try push.run()
    push.waitUntilExit()

    if push.terminationStatus != 0 {
        let data = pushPipe.fileHandleForReading.readDataToEndOfFile()
        let output = String(data: data, encoding: .utf8) ?? ""
        throw GitError.pushFailed(output)
    }
}

// gitAutoCommit stages, commits, and pushes a file
func gitAutoCommit(_ ws: String, _ file: String, _ message: String) throws {
    let relPath = file.relativePath(from: ws) ?? file

    try gitCommit(ws, [relPath], message)

    do {
        try gitPush(ws)
    } catch {
        // Warning only - commit succeeded
        fputs("WARNING: git push failed (commit succeeded): \(error.localizedDescription)\n", stderr)
    }
}

// generateCommitMessage creates a conventional commit message for thread changes
func generateCommitMessage(_ ws: String, _ files: [String]) -> String {
    var added: [String] = []
    var modified: [String] = []
    var deleted: [String] = []

    for file in files {
        let relPath = file.relativePath(from: ws) ?? file

        let filename = (file as NSString).lastPathComponent
        let name = (filename as NSString).deletingPathExtension

        if gitExistsInHEAD(ws, relPath) {
            // File exists in HEAD
            if FileManager.default.fileExists(atPath: file) {
                modified.append(name)
            } else {
                deleted.append(name)
            }
        } else {
            // File not in HEAD - it's new
            added.append(name)
        }
    }

    let total = added.count + modified.count + deleted.count

    if total == 1 {
        if added.count == 1 {
            return "threads: add \(extractID(added[0]))"
        }
        if modified.count == 1 {
            return "threads: update \(extractID(modified[0]))"
        }
        return "threads: remove \(extractID(deleted[0]))"
    }

    if total <= 3 {
        let all = added + modified + deleted
        let ids = all.map { extractID($0) }
        var action = "update"
        if added.count == total {
            action = "add"
        } else if deleted.count == total {
            action = "remove"
        }
        return "threads: \(action) \(ids.joined(separator: " "))"
    }

    var action = "update"
    if added.count == total {
        action = "add"
    } else if deleted.count == total {
        action = "remove"
    }
    return "threads: \(action) \(total) threads"
}

// extractID extracts the ID prefix from a filename like "abc123-slug-name"
func extractID(_ name: String) -> String {
    if name.count >= 6 {
        let prefix = String(name.prefix(6))
        if isHex(prefix) {
            return prefix
        }
    }
    return name
}

func isHex(_ s: String) -> Bool {
    let hexChars = CharacterSet(charactersIn: "0123456789abcdef")
    return s.unicodeScalars.allSatisfy { hexChars.contains($0) }
}

enum GitError: Error, LocalizedError {
    case addFailed(String)
    case commitFailed(String)
    case pullFailed(String)
    case pushFailed(String)

    var errorDescription: String? {
        switch self {
        case .addFailed(let msg):
            return "git add failed: \(msg)"
        case .commitFailed(let msg):
            return "git commit failed: \(msg)"
        case .pullFailed(let msg):
            return "git pull --rebase failed: \(msg)"
        case .pushFailed(let msg):
            return "git push failed: \(msg)"
        }
    }
}

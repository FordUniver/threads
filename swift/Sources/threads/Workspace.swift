import ArgumentParser
import Foundation

// Scope represents thread placement information
struct Scope {
    var threadsDir: String  // path to .threads directory
    var category: String    // category name or "-" for workspace level
    var project: String     // project name or "-" for category/workspace level
    var levelDesc: String   // human-readable description
}

// Find returns the workspace root from $WORKSPACE (required)
func findWorkspace() throws -> String {
    guard let ws = ProcessInfo.processInfo.environment["WORKSPACE"], !ws.isEmpty else {
        throw ValidationError("WORKSPACE environment variable not set")
    }
    // Normalize path to handle double slashes, trailing slashes, etc.
    let normalized = (ws as NSString).standardizingPath
    var isDir: ObjCBool = false
    guard FileManager.default.fileExists(atPath: normalized, isDirectory: &isDir), isDir.boolValue else {
        throw ValidationError("WORKSPACE directory does not exist: \(ws)")
    }
    return normalized
}

// findAllThreads returns all thread file paths in the workspace
func findAllThreads(_ ws: String) throws -> [String] {
    var threads: [String] = []
    let fm = FileManager.default

    // Helper to collect .md files from a .threads directory
    func collectThreads(from threadsDir: String) {
        guard fm.fileExists(atPath: threadsDir),
              let files = try? fm.contentsOfDirectory(atPath: threadsDir) else {
            return
        }
        for file in files where file.hasSuffix(".md") {
            let fullPath = (threadsDir as NSString).appendingPathComponent(file)
            if !fullPath.contains("/archive/") {
                threads.append(fullPath)
            }
        }
    }

    // Level 1: workspace/.threads/
    collectThreads(from: (ws as NSString).appendingPathComponent(".threads"))

    // Level 2: workspace/*/.threads/ (categories)
    if let categories = try? fm.contentsOfDirectory(atPath: ws) {
        for cat in categories where !cat.hasPrefix(".") {
            let catPath = (ws as NSString).appendingPathComponent(cat)
            var isDir: ObjCBool = false
            if fm.fileExists(atPath: catPath, isDirectory: &isDir), isDir.boolValue {
                collectThreads(from: (catPath as NSString).appendingPathComponent(".threads"))

                // Level 3: workspace/*/*/.threads/ (projects)
                if let projects = try? fm.contentsOfDirectory(atPath: catPath) {
                    for proj in projects where !proj.hasPrefix(".") {
                        let projPath = (catPath as NSString).appendingPathComponent(proj)
                        if fm.fileExists(atPath: projPath, isDirectory: &isDir), isDir.boolValue {
                            collectThreads(from: (projPath as NSString).appendingPathComponent(".threads"))
                        }
                    }
                }
            }
        }
    }

    return threads.sorted()
}

// expandGlobPattern expands a path with * wildcards
func expandGlobPattern(_ pattern: String) -> [String] {
    var results: [String] = []
    let fm = FileManager.default

    // Split pattern into components
    let components = pattern.split(separator: "/", omittingEmptySubsequences: false).map(String.init)

    func expand(currentPath: String, remaining: ArraySlice<String>) {
        guard let component = remaining.first else {
            results.append(currentPath)
            return
        }

        let nextRemaining = remaining.dropFirst()

        if component == "*" {
            // Expand wildcard
            if let entries = try? fm.contentsOfDirectory(atPath: currentPath) {
                for entry in entries where !entry.hasPrefix(".") {
                    let entryPath = (currentPath as NSString).appendingPathComponent(entry)
                    var isDir: ObjCBool = false
                    if fm.fileExists(atPath: entryPath, isDirectory: &isDir), isDir.boolValue {
                        expand(currentPath: entryPath, remaining: nextRemaining)
                    }
                }
            }
        } else {
            let nextPath = (currentPath as NSString).appendingPathComponent(component)
            if fm.fileExists(atPath: nextPath) {
                expand(currentPath: nextPath, remaining: nextRemaining)
            }
        }
    }

    // Start expansion
    if pattern.hasPrefix("/") {
        expand(currentPath: "/", remaining: components.dropFirst()[...])
    } else {
        expand(currentPath: ".", remaining: components[...])
    }

    return results
}

// inferScope determines the threads directory and level from a path
func inferScope(_ ws: String, _ path: String) throws -> Scope {
    // Handle explicit "." for workspace level
    if path == "." {
        return Scope(
            threadsDir: (ws as NSString).appendingPathComponent(".threads"),
            category: "-",
            project: "-",
            levelDesc: "workspace-level thread"
        )
    }

    var absPath: String

    // Resolve to absolute path
    if (path as NSString).isAbsolutePath {
        absPath = path
    } else {
        // Try as relative to workspace first
        let wsRelPath = (ws as NSString).appendingPathComponent(path)
        var isDir: ObjCBool = false
        if FileManager.default.fileExists(atPath: wsRelPath, isDirectory: &isDir), isDir.boolValue {
            absPath = wsRelPath
        } else if FileManager.default.fileExists(atPath: path, isDirectory: &isDir), isDir.boolValue {
            absPath = (FileManager.default.currentDirectoryPath as NSString).appendingPathComponent(path)
            absPath = (absPath as NSString).standardizingPath
        } else {
            throw WorkspaceError.pathNotFound(path)
        }
    }

    // Must be within workspace (use secure path containment)
    if !isContained(absPath, in: ws) {
        return Scope(
            threadsDir: (ws as NSString).appendingPathComponent(".threads"),
            category: "-",
            project: "-",
            levelDesc: "workspace-level thread"
        )
    }

    let rel = absPath.relativePath(from: ws) ?? ""

    if rel.isEmpty {
        return Scope(
            threadsDir: (ws as NSString).appendingPathComponent(".threads"),
            category: "-",
            project: "-",
            levelDesc: "workspace-level thread"
        )
    }

    let parts = rel.split(separator: "/", maxSplits: 2).map(String.init)
    let category = parts[0]
    var project = "-"

    if parts.count >= 2, !parts[1].isEmpty {
        project = parts[1]
    }

    if project == "-" {
        return Scope(
            threadsDir: "\(ws)/\(category)/.threads",
            category: category,
            project: "-",
            levelDesc: "category-level thread (\(category))"
        )
    }

    return Scope(
        threadsDir: "\(ws)/\(category)/\(project)/.threads",
        category: category,
        project: project,
        levelDesc: "project-level thread (\(category)/\(project))"
    )
}

// parseThreadPath extracts category, project, and name from a thread file path
func parseThreadPath(_ ws: String, _ path: String) -> (category: String, project: String, name: String) {
    let rel = path.relativePath(from: ws) ?? path

    let filename = (path as NSString).lastPathComponent
    let base = (filename as NSString).deletingPathExtension

    // Extract name, stripping ID prefix if present
    var name = extractNameFromPath(path)
    if name.isEmpty {
        name = base
    }

    // Check if workspace-level
    if rel.hasPrefix(".threads/") {
        return ("-", "-", name)
    }

    // Extract category and project from path like: category/project/.threads/name.md
    let parts = rel.split(separator: "/").map(String.init)
    var category = "-"
    var project = "-"

    if parts.count >= 2 {
        category = parts[0]
        if parts[1] == ".threads" {
            project = "-"
        } else if parts.count >= 3 {
            project = parts[1]
        }
    }

    return (category, project, name)
}

// generateID creates a unique 6-character hex ID
func generateID(_ ws: String) throws -> String {
    var existing = Set<String>()

    let threads = try findAllThreads(ws)
    for t in threads {
        let id = extractIDFromPath(t)
        if !id.isEmpty {
            existing.insert(id)
        }
    }

    // Try to generate unique ID
    for _ in 0..<10 {
        var bytes = [UInt8](repeating: 0, count: 3)
        let status = SecRandomCopyBytes(kSecRandomDefault, bytes.count, &bytes)
        guard status == errSecSuccess else {
            throw WorkspaceError.idGenerationFailed
        }
        let id = bytes.map { String(format: "%02x", $0) }.joined()
        if !existing.contains(id) {
            return id
        }
    }

    throw WorkspaceError.idGenerationFailed
}

// slugify converts a title to kebab-case filename
func slugify(_ title: String) -> String {
    var s = title.lowercased()

    // Replace non-alphanumeric with hyphens
    let allowedChars = CharacterSet.alphanumerics
    s = s.unicodeScalars.map { allowedChars.contains($0) ? String($0) : "-" }.joined()

    // Clean up multiple hyphens
    while s.contains("--") {
        s = s.replacingOccurrences(of: "--", with: "-")
    }

    // Trim leading/trailing hyphens
    s = s.trimmingCharacters(in: CharacterSet(charactersIn: "-"))

    return s
}

// findByRef locates a thread by ID or name (with fuzzy matching)
func findByRef(_ ws: String, _ ref: String) throws -> String {
    let threads = try findAllThreads(ws)

    // Fast path: exact ID match (6 hex chars)
    if CachedRegex.exactId.firstMatch(in: ref, range: NSRange(ref.startIndex..., in: ref)) != nil {
        for t in threads {
            if extractIDFromPath(t) == ref {
                return t
            }
        }
    }

    // Slow path: name matching
    var substringMatches: [String] = []
    let refLower = ref.lowercased()

    for t in threads {
        let name = extractNameFromPath(t)

        // Exact name match
        if name == ref {
            return t
        }

        // Substring match (case-insensitive)
        if name.lowercased().contains(refLower) {
            substringMatches.append(t)
        }
    }

    if substringMatches.count == 1 {
        return substringMatches[0]
    }

    if substringMatches.count > 1 {
        let ids = substringMatches.map { path in
            let id = extractIDFromPath(path)
            let name = extractNameFromPath(path)
            return "\(id) (\(name))"
        }
        throw WorkspaceError.ambiguousReference(ref, substringMatches.count, ids.joined(separator: ", "))
    }

    throw WorkspaceError.threadNotFound(ref)
}

enum WorkspaceError: Error, LocalizedError {
    case notInWorkspace
    case pathNotFound(String)
    case idGenerationFailed
    case threadNotFound(String)
    case ambiguousReference(String, Int, String)

    var errorDescription: String? {
        switch self {
        case .notInWorkspace:
            return "not in a workspace (no .threads/ found)"
        case .pathNotFound(let path):
            return "path not found: \(path)"
        case .idGenerationFailed:
            return "could not generate unique ID after 10 attempts"
        case .threadNotFound(let ref):
            return "thread not found: \(ref)"
        case .ambiguousReference(let ref, let count, let ids):
            return "ambiguous reference '\(ref)' matches \(count) threads: \(ids)"
        }
    }
}

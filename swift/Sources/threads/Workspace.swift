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
// Uses optimized 3-level traversal (workspace/category/project) for performance
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
    var absPath: String

    // Handle "." as PWD (current directory), not workspace root
    if path == "." {
        absPath = FileManager.default.currentDirectoryPath
    } else if path.hasPrefix("./") {
        // PWD-relative path
        let relPart = String(path.dropFirst(2))
        absPath = (FileManager.default.currentDirectoryPath as NSString).appendingPathComponent(relPart)
        absPath = (absPath as NSString).standardizingPath
    } else if (path as NSString).isAbsolutePath {
        absPath = path
    } else {
        // Git-root-relative path: try workspace first
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

    // Verify path exists
    var isDir: ObjCBool = false
    guard FileManager.default.fileExists(atPath: absPath, isDirectory: &isDir), isDir.boolValue else {
        throw WorkspaceError.pathNotFound(path)
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
        let bytes = (0..<3).map { _ in UInt8.random(in: 0...255) }
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

// MARK: - Direction-based thread finding

/// Options for finding threads with direction and boundary controls
struct FindOptions {
    /// Down depth: nil = no recursion, 0 = unlimited, N = N levels
    var down: Int?
    /// Up depth: nil = no up search, 0 = unlimited, N = N levels
    var up: Int?

    /// Returns true if down searching is enabled
    var hasDown: Bool { down != nil }

    /// Returns true if up searching is enabled
    var hasUp: Bool { up != nil }

    /// Returns the effective down depth: -1 for unlimited, 0 for no search, N for N levels
    var downDepth: Int {
        guard let d = down else { return 0 }
        return d == 0 ? -1 : d  // 0 means unlimited
    }

    /// Returns the effective up depth: -1 for unlimited, 0 for no search, N for N levels
    var upDepth: Int {
        guard let u = up else { return 0 }
        return u == 0 ? -1 : u  // 0 means unlimited
    }
}

/// Check if a directory is a git root (contains .git)
func isGitRoot(_ path: String) -> Bool {
    let gitPath = (path as NSString).appendingPathComponent(".git")
    var isDir: ObjCBool = false
    return FileManager.default.fileExists(atPath: gitPath, isDirectory: &isDir)
}

/// Collect thread files from a .threads directory at the given path
func collectThreadsAtPath(_ dir: String, _ threads: inout [String]) {
    let fm = FileManager.default
    let threadsDir = (dir as NSString).appendingPathComponent(".threads")

    var isDir: ObjCBool = false
    guard fm.fileExists(atPath: threadsDir, isDirectory: &isDir), isDir.boolValue else {
        return
    }

    guard let files = try? fm.contentsOfDirectory(atPath: threadsDir) else {
        return
    }

    for file in files where file.hasSuffix(".md") {
        let fullPath = (threadsDir as NSString).appendingPathComponent(file)
        if !fullPath.contains("/archive/") {
            threads.append(fullPath)
        }
    }
}

/// Recursively find threads going down into subdirectories
func findThreadsDown(_ dir: String, _ ws: String, _ threads: inout [String],
                     currentDepth: Int, maxDepth: Int, crossGitBoundaries: Bool) {
    // Check depth limit (-1 means unlimited)
    if maxDepth >= 0 && currentDepth >= maxDepth {
        return
    }

    let fm = FileManager.default
    guard let entries = try? fm.contentsOfDirectory(atPath: dir) else {
        return
    }

    for entry in entries {
        // Skip hidden directories
        if entry.hasPrefix(".") {
            continue
        }

        let subdir = (dir as NSString).appendingPathComponent(entry)
        var isDir: ObjCBool = false
        guard fm.fileExists(atPath: subdir, isDirectory: &isDir), isDir.boolValue else {
            continue
        }

        // Check git boundary
        if !crossGitBoundaries && subdir != ws && isGitRoot(subdir) {
            continue
        }

        // Collect threads at this level
        collectThreadsAtPath(subdir, &threads)

        // Continue recursing
        findThreadsDown(subdir, ws, &threads,
                       currentDepth: currentDepth + 1,
                       maxDepth: maxDepth,
                       crossGitBoundaries: crossGitBoundaries)
    }
}

/// Find threads going up into parent directories
func findThreadsUp(_ dir: String, _ ws: String, _ threads: inout [String],
                   currentDepth: Int, maxDepth: Int, crossGitBoundaries: Bool) {
    // Check depth limit (-1 means unlimited)
    if maxDepth >= 0 && currentDepth >= maxDepth {
        return
    }

    let parent = (dir as NSString).deletingLastPathComponent
    if parent == dir || parent.isEmpty {
        return  // reached filesystem root
    }

    let absParent = (parent as NSString).standardizingPath
    let absWs = (ws as NSString).standardizingPath

    // Check git boundary: stop at workspace root unless crossing is allowed
    if !crossGitBoundaries && !absParent.hasPrefix(absWs) {
        return
    }

    // Collect threads at parent
    collectThreadsAtPath(absParent, &threads)

    // Continue up
    findThreadsUp(absParent, ws, &threads,
                 currentDepth: currentDepth + 1,
                 maxDepth: maxDepth,
                 crossGitBoundaries: crossGitBoundaries)
}

/// Find threads with direction and boundary controls
func findThreadsWithOptions(_ startPath: String, _ ws: String, _ options: FindOptions) -> [String] {
    var threads: [String] = []

    let absStart = (startPath as NSString).standardizingPath

    // Always collect threads at start_path
    collectThreadsAtPath(absStart, &threads)

    // Search down (subdirectories)
    if options.hasDown {
        let maxDepth = options.downDepth
        findThreadsDown(absStart, ws, &threads,
                       currentDepth: 0,
                       maxDepth: maxDepth,
                       crossGitBoundaries: false)
    }

    // Search up (parent directories)
    if options.hasUp {
        let maxDepth = options.upDepth
        findThreadsUp(absStart, ws, &threads,
                     currentDepth: 0,
                     maxDepth: maxDepth,
                     crossGitBoundaries: false)
    }

    // Sort and deduplicate
    threads.sort()
    return Array(Set(threads)).sorted()
}

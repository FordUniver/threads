import Foundation

// MARK: - Cached Regex Patterns

/// Container for pre-compiled regex patterns used across the codebase
enum CachedRegex {
    /// Matches 6-char hex ID prefix in filenames (e.g., "abc123-slug")
    static let idPrefixCapture: NSRegularExpression = {
        try! NSRegularExpression(pattern: #"^([0-9a-f]{6})-"#)
    }()

    /// Matches 6-char hex ID prefix (non-capturing)
    static let idPrefix: NSRegularExpression = {
        try! NSRegularExpression(pattern: #"^[0-9a-f]{6}-"#)
    }()

    /// Matches exact 6-char hex ID
    static let exactId: NSRegularExpression = {
        try! NSRegularExpression(pattern: #"^[0-9a-f]{6}$"#)
    }()

    /// Matches hash comments like <!-- abc1 -->
    static let hashComment: NSRegularExpression = {
        try! NSRegularExpression(pattern: #"<!--\s*([a-f0-9]{4})\s*-->"#)
    }()

    /// Matches ## Log section header
    static let logSection: NSRegularExpression = {
        try! NSRegularExpression(pattern: "(?m)^## Log")
    }()
}

// Helper to get workspace with error handling
func getWorkspace() throws -> String {
    try findWorkspace()
}

// Helper to print to stderr
func printError(_ message: String) {
    fputs("\(message)\n", stderr)
}

// Check if stdin has data available (non-blocking)
func stdinHasData() -> Bool {
    var statInfo = stat()
    if fstat(STDIN_FILENO, &statInfo) == 0 {
        return (statInfo.st_mode & S_IFMT) != S_IFCHR
    }
    return false
}

// Read all data from stdin
func readStdin() -> String? {
    guard stdinHasData() else { return nil }
    var data = Data()
    while let byte = try? FileHandle.standardInput.read(upToCount: 4096), !byte.isEmpty {
        data.append(byte)
    }
    if data.isEmpty { return nil }
    return String(data: data, encoding: .utf8)
}

// Check if stdin is a terminal (interactive)
func isTerminal() -> Bool {
    return isatty(STDIN_FILENO) != 0
}

// Truncate string for display
func truncate(_ s: String, _ max: Int) -> String {
    if s.count <= max {
        return s
    }
    return String(s.prefix(max - 1)) + "…"
}

extension String {
    // Left-pad a string to a given length
    func leftPad(toLength length: Int, withPad pad: Character = " ") -> String {
        if self.count >= length {
            return self
        }
        return String(repeating: pad, count: length - self.count) + self
    }

    // Right-pad a string to a given length (no truncation)
    func rightPad(toLength length: Int, withPad pad: Character = " ") -> String {
        if self.count >= length {
            return self
        }
        return self + String(repeating: pad, count: length - self.count)
    }

    // Compute relative path from a base path
    func relativePath(from base: String) -> String? {
        let basePath = (base as NSString).standardizingPath
        let selfPath = (self as NSString).standardizingPath
        guard selfPath.hasPrefix(basePath) else { return nil }
        var rel = String(selfPath.dropFirst(basePath.count))
        if rel.hasPrefix("/") {
            rel = String(rel.dropFirst())
        }
        return rel
    }
}

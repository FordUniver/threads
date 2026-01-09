import Foundation

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
}

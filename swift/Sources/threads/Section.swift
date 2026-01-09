import Foundation

// extractSection returns the content of a section (between ## Name and next ## or EOF)
func extractSection(_ content: String, _ name: String) -> String {
    let pattern = "(?ms)^## \(NSRegularExpression.escapedPattern(for: name))\n(.+?)(?:^## |\\z)"
    guard let regex = try? NSRegularExpression(pattern: pattern, options: [.anchorsMatchLines, .dotMatchesLineSeparators]),
          let match = regex.firstMatch(in: content, range: NSRange(content.startIndex..., in: content)),
          let range = Range(match.range(at: 1), in: content) else {
        return ""
    }
    return String(content[range]).trimmingCharacters(in: .whitespacesAndNewlines)
}

// replaceSection replaces the content of a section
func replaceSection(_ content: String, _ name: String, _ newContent: String) -> String {
    let pattern = "(?ms)(^## \(NSRegularExpression.escapedPattern(for: name))\n)(.+?)(^## |\\z)"
    guard let regex = try? NSRegularExpression(pattern: pattern, options: [.anchorsMatchLines, .dotMatchesLineSeparators]),
          regex.firstMatch(in: content, range: NSRange(content.startIndex..., in: content)) != nil else {
        // Section doesn't exist
        return content
    }

    return regex.stringByReplacingMatches(
        in: content,
        range: NSRange(content.startIndex..., in: content),
        withTemplate: "$1\n\(newContent)\n\n$3"
    )
}

// appendToSection appends content to a section
func appendToSection(_ content: String, _ name: String, _ addition: String) -> String {
    let sectionContent = extractSection(content, name)
    var newContent = sectionContent.trimmingCharacters(in: .whitespacesAndNewlines)
    if !newContent.isEmpty {
        newContent += "\n"
    }
    newContent += addition
    return replaceSection(content, name, newContent)
}

// ensureSection creates a section if it doesn't exist, placing it before another section
func ensureSection(_ content: String, _ name: String, _ before: String) -> String {
    let pattern = "(?m)^## \(NSRegularExpression.escapedPattern(for: name))"
    if let regex = try? NSRegularExpression(pattern: pattern),
       regex.firstMatch(in: content, range: NSRange(content.startIndex..., in: content)) != nil {
        return content // Section already exists
    }

    let beforePattern = "(?m)(^## \(NSRegularExpression.escapedPattern(for: before)))"
    if let beforeRegex = try? NSRegularExpression(pattern: beforePattern),
       beforeRegex.firstMatch(in: content, range: NSRange(content.startIndex..., in: content)) != nil {
        return beforeRegex.stringByReplacingMatches(
            in: content,
            range: NSRange(content.startIndex..., in: content),
            withTemplate: "## \(name)\n\n$1"
        )
    }

    // If before section doesn't exist, append at end
    return content + "\n## \(name)\n\n"
}

// generateHash creates a 4-character hash for an item using FNV-1a
func generateHash(_ text: String) -> String {
    let data = "\(text)\(Date().timeIntervalSince1970)"
    var hash: UInt64 = 14695981039346656037  // FNV offset basis
    for byte in data.utf8 {
        hash ^= UInt64(byte)
        hash &*= 1099511628211  // FNV prime
    }
    // Extract 2 bytes (4 hex chars)
    return String(format: "%04x", hash & 0xFFFF)
}

// insertLogEntry adds a timestamped entry to the Log section
func insertLogEntry(_ content: String, _ entry: String) -> String {
    let dateFormatter = DateFormatter()
    dateFormatter.dateFormat = "yyyy-MM-dd"
    let today = dateFormatter.string(from: Date())

    dateFormatter.dateFormat = "HH:mm"
    let timestamp = dateFormatter.string(from: Date())

    let bulletEntry = "- **\(timestamp)** \(entry)"
    let heading = "### \(today)"

    // Check if today's heading exists
    let todayPattern = "(?m)^### \(NSRegularExpression.escapedPattern(for: today))"
    if let todayRegex = try? NSRegularExpression(pattern: todayPattern),
       todayRegex.firstMatch(in: content, range: NSRange(content.startIndex..., in: content)) != nil {
        // Insert after today's heading
        let insertPattern = "(?m)(^### \(NSRegularExpression.escapedPattern(for: today))\n)"
        if let insertRegex = try? NSRegularExpression(pattern: insertPattern) {
            return insertRegex.stringByReplacingMatches(
                in: content,
                range: NSRange(content.startIndex..., in: content),
                withTemplate: "$1\n\(bulletEntry)\n"
            )
        }
    }

    // Check if Log section exists
    if CachedRegex.logSection.firstMatch(in: content, range: NSRange(content.startIndex..., in: content)) != nil {
        // Insert new heading after ## Log
        return CachedRegex.logSection.stringByReplacingMatches(
            in: content,
            range: NSRange(content.startIndex..., in: content),
            withTemplate: "## Log\n\n\(heading)\n\n\(bulletEntry)"
        )
    }

    // No Log section - append one
    return content + "\n## Log\n\n\(heading)\n\n\(bulletEntry)\n"
}

// addNote adds a note to the Notes section with a hash comment
func addNote(_ content: String, _ text: String) -> (String, String) {
    // Ensure Notes section exists
    var newContent = ensureSection(content, "Notes", "Todo")

    let hash = generateHash(text)
    let noteEntry = "- \(text)  <!-- \(hash) -->"

    // Insert at top of Notes section
    let pattern = "(?m)(^## Notes\n)"
    if let regex = try? NSRegularExpression(pattern: pattern) {
        newContent = regex.stringByReplacingMatches(
            in: newContent,
            range: NSRange(newContent.startIndex..., in: newContent),
            withTemplate: "$1\n\(noteEntry)\n"
        )
    }

    return (newContent, hash)
}

// removeByHash removes a line containing the specified hash comment from a section
func removeByHash(_ content: String, _ section: String, _ hash: String) throws -> String {
    let lines = content.components(separatedBy: "\n")
    var inSection = false
    let hashPattern = "<!-- \(hash)"
    var found = false

    var result: [String] = []
    for line in lines {
        if line.hasPrefix("## \(section)") {
            inSection = true
        } else if line.hasPrefix("## ") {
            inSection = false
        }

        if inSection && line.contains(hashPattern) && !found {
            found = true
            continue // skip this line
        }
        result.append(line)
    }

    if !found {
        throw SectionError.hashNotFound(hash)
    }

    return result.joined(separator: "\n")
}

// editByHash replaces the text of an item by hash
func editByHash(_ content: String, _ section: String, _ hash: String, _ newText: String) throws -> String {
    let lines = content.components(separatedBy: "\n")
    var inSection = false
    let hashPattern = "<!-- \(hash)"
    var found = false

    var result: [String] = []

    for line in lines {
        if line.hasPrefix("## \(section)") {
            inSection = true
        } else if line.hasPrefix("## ") {
            inSection = false
        }

        if inSection && line.contains(hashPattern) && !found {
            found = true
            // Extract hash from line and rebuild
            if let match = CachedRegex.hashComment.firstMatch(in: line, range: NSRange(line.startIndex..., in: line)),
               let range = Range(match.range(at: 1), in: line) {
                let extractedHash = String(line[range])
                result.append("- \(newText)  <!-- \(extractedHash) -->")
                continue
            }
        }
        result.append(line)
    }

    if !found {
        throw SectionError.hashNotFound(hash)
    }

    return result.joined(separator: "\n")
}

// addTodoItem adds a checkbox item to the Todo section
func addTodoItem(_ content: String, _ text: String) -> (String, String) {
    let hash = generateHash(text)
    let todoEntry = "- [ ] \(text)  <!-- \(hash) -->"

    // Insert at top of Todo section
    let pattern = "(?m)(^## Todo\n)"
    if let regex = try? NSRegularExpression(pattern: pattern) {
        let newContent = regex.stringByReplacingMatches(
            in: content,
            range: NSRange(content.startIndex..., in: content),
            withTemplate: "$1\n\(todoEntry)\n"
        )
        return (newContent, hash)
    }

    return (content, hash)
}

// setTodoChecked sets a todo item's checked state by hash
func setTodoChecked(_ content: String, _ hash: String, _ checked: Bool) throws -> String {
    let lines = content.components(separatedBy: "\n")
    var inTodo = false
    let hashPattern = "<!-- \(hash)"
    var found = false

    var result: [String] = []
    for var line in lines {
        if line.hasPrefix("## Todo") {
            inTodo = true
        } else if line.hasPrefix("## ") {
            inTodo = false
        }

        if inTodo && line.contains(hashPattern) && !found {
            found = true
            if checked {
                line = line.replacingOccurrences(of: "- [ ]", with: "- [x]")
            } else {
                line = line.replacingOccurrences(of: "- [x]", with: "- [ ]")
            }
        }
        result.append(line)
    }

    if !found {
        throw SectionError.hashNotFound(hash)
    }

    return result.joined(separator: "\n")
}

// countMatchingItems counts items matching a hash prefix in a section
func countMatchingItems(_ content: String, _ section: String, _ hash: String) -> Int {
    let lines = content.components(separatedBy: "\n")
    var inSection = false
    let hashPattern = "<!-- \(hash)"
    var count = 0

    for line in lines {
        if line.hasPrefix("## \(section)") {
            inSection = true
        } else if line.hasPrefix("## ") {
            inSection = false
        }

        if inSection && line.contains(hashPattern) {
            count += 1
        }
    }

    return count
}

enum SectionError: Error, LocalizedError {
    case hashNotFound(String)

    var errorDescription: String? {
        switch self {
        case .hashNotFound(let hash):
            return "no item with hash '\(hash)' found"
        }
    }
}

import Foundation
import Yams

// Status constants
let terminalStatuses = ["resolved", "superseded", "deferred"]
let activeStatuses = ["idea", "planning", "active", "blocked", "paused"]
let allStatuses = activeStatuses + terminalStatuses

// FrontmatterRaw is the raw parsed YAML with optional fields
struct FrontmatterRaw: Codable {
    var id: String?
    var name: String?
    var desc: String?
    var status: String?
}

// Frontmatter represents the YAML frontmatter of a thread
struct Frontmatter: Codable {
    var id: String
    var name: String
    var desc: String
    var status: String

    init(id: String = "", name: String = "", desc: String = "", status: String = "idea") {
        self.id = id
        self.name = name
        self.desc = desc
        self.status = status
    }

    init(from raw: FrontmatterRaw) {
        self.id = raw.id ?? ""
        self.name = raw.name ?? ""
        self.desc = raw.desc ?? ""
        self.status = raw.status ?? ""
    }
}

// Thread represents a parsed thread file
class Thread {
    var path: String         // absolute file path
    var frontmatter: Frontmatter
    var content: String      // full file content
    var bodyStart: Int       // character offset where body starts (after frontmatter)

    init(path: String, frontmatter: Frontmatter, content: String, bodyStart: Int) {
        self.path = path
        self.frontmatter = frontmatter
        self.content = content
        self.bodyStart = bodyStart
    }

    // Parse reads and parses a thread file
    static func parse(path: String) throws -> Thread {
        let content = try String(contentsOfFile: path, encoding: .utf8)

        let thread = Thread(
            path: path,
            frontmatter: Frontmatter(),
            content: content,
            bodyStart: 0
        )

        try thread.parseFrontmatter()

        // Extract ID from filename if not in frontmatter
        if thread.frontmatter.id.isEmpty {
            thread.frontmatter.id = extractIDFromPath(path)
        }

        return thread
    }

    // parseFrontmatter extracts and parses YAML frontmatter
    func parseFrontmatter() throws {
        guard content.hasPrefix("---\n") else {
            throw ThreadError.missingFrontmatterDelimiter
        }

        // Find closing delimiter
        let searchStart = content.index(content.startIndex, offsetBy: 4)
        guard let endRange = content.range(of: "\n---", range: searchStart..<content.endIndex) else {
            throw ThreadError.unclosedFrontmatter
        }

        let yamlContent = String(content[searchStart..<endRange.lowerBound])
        bodyStart = content.distance(from: content.startIndex, to: endRange.upperBound) + 1 // skip past \n---\n

        // Parse YAML - use FrontmatterRaw to handle optional fields
        let decoder = YAMLDecoder()
        do {
            let raw = try decoder.decode(FrontmatterRaw.self, from: yamlContent)
            frontmatter = Frontmatter(from: raw)
        } catch {
            throw ThreadError.yamlParseError(error.localizedDescription)
        }
    }

    // ID returns the thread ID
    var id: String { frontmatter.id }

    // Name returns the thread name/title
    var name: String { frontmatter.name }

    // Status returns the thread status
    var status: String { frontmatter.status }

    // baseStatus returns status without reason suffix
    var baseStatus: String {
        Thread.baseStatus(frontmatter.status)
    }

    // baseStatus strips reason suffix from status
    static func baseStatus(_ status: String) -> String {
        if let idx = status.range(of: " (") {
            return String(status[..<idx.lowerBound])
        }
        return status
    }

    // isTerminal returns true if the status is terminal
    static func isTerminal(_ status: String) -> Bool {
        let base = baseStatus(status)
        return terminalStatuses.contains(base)
    }

    // isValidStatus returns true if the status is valid
    static func isValidStatus(_ status: String) -> Bool {
        let base = baseStatus(status)
        return allStatuses.contains(base)
    }

    // body returns the content after frontmatter
    var body: String {
        guard bodyStart < content.count else { return "" }
        let idx = content.index(content.startIndex, offsetBy: bodyStart)
        return String(content[idx...])
    }

    // setFrontmatterField updates a frontmatter field and rewrites content
    func setFrontmatterField(_ field: String, _ value: String) throws {
        switch field {
        case "id":
            frontmatter.id = value
        case "name":
            frontmatter.name = value
        case "desc":
            frontmatter.desc = value
        case "status":
            frontmatter.status = value
        default:
            throw ThreadError.unknownField(field)
        }

        try rebuildContent()
    }

    // rebuildContent reconstructs file content from frontmatter and body
    func rebuildContent() throws {
        var sb = "---\n"

        // Marshal frontmatter
        let encoder = YAMLEncoder()
        let yaml = try encoder.encode(frontmatter)
        sb += yaml
        sb += "---\n"

        // Preserve body
        if bodyStart < content.count {
            let idx = content.index(content.startIndex, offsetBy: bodyStart)
            sb += String(content[idx...])
        }

        content = sb
    }

    // write saves the thread to disk
    func write() throws {
        try content.write(toFile: path, atomically: true, encoding: .utf8)
    }

    // relPath returns the path relative to workspace
    func relPath(_ ws: String) -> String {
        if path.hasPrefix(ws) {
            var rel = String(path.dropFirst(ws.count))
            if rel.hasPrefix("/") {
                rel = String(rel.dropFirst())
            }
            return rel
        }
        return path
    }
}

// extractIDFromPath extracts the 6-char hex ID from a filename
func extractIDFromPath(_ path: String) -> String {
    let filename = (path as NSString).lastPathComponent
    let base = (filename as NSString).deletingPathExtension

    // Check for ID prefix pattern: abc123-slug
    let pattern = #"^([0-9a-f]{6})-"#
    if let regex = try? NSRegularExpression(pattern: pattern),
       let match = regex.firstMatch(in: base, range: NSRange(base.startIndex..., in: base)),
       let range = Range(match.range(at: 1), in: base) {
        return String(base[range])
    }
    return ""
}

// extractNameFromPath extracts the human-readable name from a filename
func extractNameFromPath(_ path: String) -> String {
    let filename = (path as NSString).lastPathComponent
    let base = (filename as NSString).deletingPathExtension

    // Check for ID prefix pattern: abc123-slug
    let pattern = #"^[0-9a-f]{6}-"#
    if let regex = try? NSRegularExpression(pattern: pattern),
       regex.firstMatch(in: base, range: NSRange(base.startIndex..., in: base)) != nil {
        // Skip the ID prefix (7 chars: 6 hex + dash)
        return String(base.dropFirst(7))
    }
    return base
}

enum ThreadError: Error, LocalizedError {
    case missingFrontmatterDelimiter
    case unclosedFrontmatter
    case yamlParseError(String)
    case unknownField(String)

    var errorDescription: String? {
        switch self {
        case .missingFrontmatterDelimiter:
            return "missing frontmatter delimiter"
        case .unclosedFrontmatter:
            return "unclosed frontmatter"
        case .yamlParseError(let msg):
            return "YAML parse error: \(msg)"
        case .unknownField(let field):
            return "unknown field: \(field)"
        }
    }
}

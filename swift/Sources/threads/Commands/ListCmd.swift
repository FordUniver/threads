import ArgumentParser
import Foundation

struct ThreadInfo: Codable {
    let id: String
    let status: String
    let category: String
    let project: String
    let name: String
    let title: String
    let desc: String
}

struct ListCmd: ParsableCommand {
    static var configuration = CommandConfiguration(
        commandName: "list",
        abstract: "List threads",
        discussion: """
            List threads at the specified level.

            By default shows active threads at the current level only.
            Use -d/--down to include subdirectories, -u/--up to include parent directories.
            Use -r as an alias for --down (unlimited depth).
            Use --include-closed to include resolved/terminal threads.

            Depth values: N levels, or 0 for unlimited.
            """,
        aliases: ["ls"]
    )

    @Option(name: .shortAndLong, help: "Search subdirectories (N levels, 0=unlimited)")
    var down: Int?

    @Flag(name: .shortAndLong, help: "Alias for --down (unlimited depth)")
    var recursive = false

    @Option(name: .shortAndLong, help: "Search parent directories (N levels, 0=unlimited)")
    var up: Int?

    @Flag(name: .long, help: "Include resolved/terminal threads")
    var includeClosed = false

    @Option(name: .shortAndLong, help: "Search name/title/desc (substring)")
    var search: String?

    @Option(name: .long, help: "Filter by status")
    var status: String?

    @Option(name: .shortAndLong, help: "Filter by category")
    var category: String?

    @Option(name: .shortAndLong, help: "Filter by project")
    var project: String?

    @Flag(name: .long, help: "Output as JSON")
    var json = false

    @Argument(help: "Path to list threads from")
    var path: String?

    /// Describes the search direction for output display
    struct SearchDirection {
        var hasDown: Bool
        var downDepth: Int  // -1 = unlimited, 0+ = specific depth
        var hasUp: Bool
        var upDepth: Int    // -1 = unlimited, 0+ = specific depth

        func description() -> String {
            var parts: [String] = []

            if hasDown {
                if downDepth < 0 {
                    parts.append("recursive")
                } else {
                    parts.append("down \(downDepth)")
                }
            }

            if hasUp {
                if upDepth < 0 {
                    parts.append("up")
                } else {
                    parts.append("up \(upDepth)")
                }
            }

            if parts.isEmpty {
                return ""
            }
            return " (\(parts.joined(separator: ", ")))"
        }

        var isSearching: Bool {
            return hasDown || hasUp
        }
    }

    mutating func run() throws {
        let ws = try getWorkspace()

        // Determine search direction: --down/-d takes priority, then -r as alias
        let hasDown = down != nil || recursive
        var downDepth = -1  // unlimited by default when enabled
        if let d = down, d > 0 {
            downDepth = d
        }

        let hasUp = up != nil
        var upDepth = -1  // unlimited by default when enabled
        if let u = up, u > 0 {
            upDepth = u
        }

        // Track search direction for output
        let searchDir = SearchDirection(
            hasDown: hasDown,
            downDepth: downDepth,
            hasUp: hasUp,
            upDepth: upDepth
        )

        // Determine start path - default to PWD, not workspace root
        var startPath = FileManager.default.currentDirectoryPath
        var categoryFilter = category
        var projectFilter = project
        var searchFilter = search

        // Parse path filter if provided
        if let pathFilter = path {
            // Use inferScope to properly resolve the path
            if let scope = try? inferScope(ws, pathFilter) {
                startPath = scope.threadsDir.replacingOccurrences(of: "/.threads", with: "")
                if scope.category != "-" {
                    categoryFilter = scope.category
                    if scope.project != "-" {
                        projectFilter = scope.project
                    }
                }
            } else {
                // Treat as search filter
                searchFilter = pathFilter
            }
        }

        // Build find options
        var options = FindOptions()

        if hasDown {
            // Convert depth: -1 (unlimited) -> 0 in FindOptions convention
            options.down = downDepth < 0 ? 0 : downDepth
        }

        if hasUp {
            // Convert depth: -1 (unlimited) -> 0 in FindOptions convention
            options.up = upDepth < 0 ? 0 : upDepth
        }

        // Find threads using options
        let threads = findThreadsWithOptions(startPath, ws, options)
        var results: [ThreadInfo] = []

        for threadPath in threads {
            guard let t = try? Thread.parse(path: threadPath) else { continue }

            let (cat, proj, name) = parseThreadPath(ws, threadPath)
            let threadStatus = t.status
            let baseStatus = Thread.baseStatus(threadStatus)

            // Category filter (when not searching directionally)
            if !searchDir.isSearching {
                if let catFilter = categoryFilter, cat != catFilter {
                    continue
                }
                if let projFilter = projectFilter, proj != projFilter {
                    continue
                }
            }

            // Status filter
            if let statusFilter = status {
                let statuses = statusFilter.split(separator: ",").map(String.init)
                if !statuses.contains(baseStatus) {
                    continue
                }
            } else {
                if !includeClosed && Thread.isTerminal(threadStatus) {
                    continue
                }
            }

            // Search filter
            if let searchTerm = searchFilter {
                let searchLower = searchTerm.lowercased()
                let nameLower = name.lowercased()
                let titleLower = t.name.lowercased()
                let descLower = t.frontmatter.desc.lowercased()

                if !nameLower.contains(searchLower) &&
                   !titleLower.contains(searchLower) &&
                   !descLower.contains(searchLower) {
                    continue
                }
            }

            // Use title if available, else humanize name
            var title = t.name
            if title.isEmpty {
                title = name.replacingOccurrences(of: "-", with: " ")
            }

            results.append(ThreadInfo(
                id: t.id,
                status: baseStatus,
                category: cat,
                project: proj,
                name: name,
                title: title,
                desc: t.frontmatter.desc
            ))
        }

        if json {
            outputJSON(results)
        } else {
            outputTable(results, ws, categoryFilter: categoryFilter, projectFilter: projectFilter, statusFilter: status, showAll: includeClosed, searchDir: searchDir)
        }
    }

    func outputJSON(_ results: [ThreadInfo]) {
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
        if let data = try? encoder.encode(results),
           let output = String(data: data, encoding: .utf8) {
            print(output)
        }
    }

    func outputTable(_ results: [ThreadInfo], _ ws: String, categoryFilter: String?, projectFilter: String?, statusFilter: String?, showAll: Bool, searchDir: SearchDirection) {
        // Build header description
        var levelDesc: String
        var pathSuffix = ""

        if let proj = projectFilter, let cat = categoryFilter {
            levelDesc = "project-level"
            pathSuffix = " (\(cat)/\(proj))"
        } else if let cat = categoryFilter {
            levelDesc = "category-level"
            pathSuffix = " (\(cat))"
        } else {
            levelDesc = "workspace-level"
        }

        var statusDesc = "active"
        if let sf = statusFilter {
            statusDesc = sf
        } else if showAll {
            statusDesc = ""
        }

        let searchSuffix = searchDir.description()

        if !statusDesc.isEmpty {
            print("Showing \(results.count) \(statusDesc) \(levelDesc) threads\(pathSuffix)\(searchSuffix)")
        } else {
            print("Showing \(results.count) \(levelDesc) threads\(pathSuffix) (all statuses)\(searchSuffix)")
        }
        print()

        if results.isEmpty {
            if !searchDir.isSearching {
                print("Hint: use -r to include nested directories, -u to search parents")
            }
            return
        }

        // Print table header
        print(formatRow("ID", "STATUS", "CATEGORY", "PROJECT", "NAME"))
        print(formatRow("--", "------", "--------", "-------", "----"))

        for t in results {
            let cat = truncate(t.category, 16)
            let proj = truncate(t.project, 20)
            print(formatRow(t.id, t.status, cat, proj, t.title))
        }
    }

    func formatRow(_ id: String, _ status: String, _ category: String, _ project: String, _ name: String) -> String {
        let idPad = id.rightPad(toLength: 6)
        let statusPad = status.rightPad(toLength: 10)
        let catPad = category.rightPad(toLength: 18)
        let projPad = project.rightPad(toLength: 22)
        return "\(idPad) \(statusPad) \(catPad) \(projPad) \(name)"
    }
}

import ArgumentParser
import Foundation

struct StatsCmd: ParsableCommand {
    static var configuration = CommandConfiguration(
        commandName: "stats",
        abstract: "Show thread count by status",
        discussion: """
            Show thread statistics at the specified level.

            By default shows stats for threads at the current level only.
            Use -d/--down to include subdirectories, -u/--up to include parent directories.
            Use -r as an alias for --down (unlimited depth).

            Depth values: N levels, or 0 for unlimited.
            """
    )

    @Option(name: .shortAndLong, help: "Search subdirectories (N levels, 0=unlimited)")
    var down: Int?

    @Flag(name: .shortAndLong, help: "Alias for --down (unlimited depth)")
    var recursive = false

    @Option(name: .shortAndLong, help: "Search parent directories (N levels, 0=unlimited)")
    var up: Int?

    @Argument(help: "Path to show stats for")
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

    func run() throws {
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
        var categoryFilter: String?
        var projectFilter: String?

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

        var counts: [String: Int] = [:]
        var total = 0

        for threadPath in threads {
            let (cat, proj, _) = parseThreadPath(ws, threadPath)

            // Category/project filter (when not searching directionally)
            if !searchDir.isSearching {
                if let catFilter = categoryFilter, cat != catFilter {
                    continue
                }
                if let projFilter = projectFilter, proj != projFilter {
                    continue
                }
            }

            guard let t = try? Thread.parse(path: threadPath) else { continue }

            var status = t.baseStatus
            if status.isEmpty {
                status = "(none)"
            }
            counts[status, default: 0] += 1
            total += 1
        }

        // Build scope description
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

        let searchSuffix = searchDir.description()

        print("Stats for \(levelDesc) threads\(pathSuffix)\(searchSuffix)")
        print()

        if total == 0 {
            print("No threads found.")
            if !searchDir.isSearching {
                print("Hint: use -r to include nested directories, -u to search parents")
            }
            return
        }

        // Sort by count descending
        let sorted = counts.sorted { $0.value > $1.value }

        print("| Status     | Count |")
        print("|------------|-------|")
        for (status, count) in sorted {
            let statusPad = status.padding(toLength: 10, withPad: " ", startingAt: 0)
            let countStr = String(count).leftPad(toLength: 5)
            print("| \(statusPad) | \(countStr) |")
        }
        print("|------------|-------|")
        let totalPad = "Total".padding(toLength: 10, withPad: " ", startingAt: 0)
        let totalStr = String(total).leftPad(toLength: 5)
        print("| \(totalPad) | \(totalStr) |")
    }
}

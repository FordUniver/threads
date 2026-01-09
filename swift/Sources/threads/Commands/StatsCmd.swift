import ArgumentParser
import Foundation

struct StatsCmd: ParsableCommand {
    static var configuration = CommandConfiguration(
        commandName: "stats",
        abstract: "Show thread count by status"
    )

    @Flag(name: .shortAndLong, help: "Include nested categories/projects")
    var recursive = false

    @Argument(help: "Path to show stats for")
    var path: String?

    func run() throws {
        let ws = try getWorkspace()

        // Parse path filter
        var categoryFilter: String?
        var projectFilter: String?

        if let pathFilter = path {
            let fullPath = "\(ws)/\(pathFilter)"
            var isDir: ObjCBool = false
            if FileManager.default.fileExists(atPath: fullPath, isDirectory: &isDir), isDir.boolValue {
                let parts = pathFilter.split(separator: "/", maxSplits: 1).map(String.init)
                categoryFilter = parts[0]
                if parts.count > 1 {
                    projectFilter = parts[1]
                }
            }
        }

        // Find all threads
        let threads = try findAllThreads(ws)

        var counts: [String: Int] = [:]
        var total = 0

        for threadPath in threads {
            let (cat, proj, _) = parseThreadPath(ws, threadPath)

            // Category filter
            if let catFilter = categoryFilter, cat != catFilter {
                continue
            }

            // Project filter
            if let projFilter = projectFilter, proj != projFilter {
                continue
            }

            // Non-recursive: only threads at current hierarchy level
            if !recursive {
                if projectFilter != nil {
                    // At project level, count all
                } else if categoryFilter != nil {
                    if proj != "-" {
                        continue
                    }
                } else {
                    if cat != "-" {
                        continue
                    }
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

        var recursiveSuffix = ""
        if recursive {
            recursiveSuffix = " (including nested)"
        }

        print("Stats for \(levelDesc) threads\(pathSuffix)\(recursiveSuffix)")
        print()

        if total == 0 {
            print("No threads found.")
            if !recursive {
                print("Hint: use -r to include nested categories/projects")
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

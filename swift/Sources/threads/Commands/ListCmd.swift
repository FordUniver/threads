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
        aliases: ["ls"],
        discussion: """
            List threads at the specified level.

            By default shows active threads at the current level only.
            Use -r to include nested categories/projects.
            Use --include-closed to include resolved/terminal threads.
            """
    )

    @Flag(name: .shortAndLong, help: "Include nested categories/projects")
    var recursive = false

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

    mutating func run() throws {
        let ws = try getWorkspace()

        var categoryFilter = category
        var projectFilter = project
        var searchFilter = search

        // Parse path filter if provided
        if let pathFilter = path {
            let fullPath = "\(ws)/\(pathFilter)"
            var isDir: ObjCBool = false
            if FileManager.default.fileExists(atPath: fullPath, isDirectory: &isDir), isDir.boolValue {
                let parts = pathFilter.split(separator: "/", maxSplits: 1).map(String.init)
                categoryFilter = parts[0]
                if parts.count > 1 {
                    projectFilter = parts[1]
                }
            } else {
                // Treat as search filter
                searchFilter = pathFilter
            }
        }

        // Find all threads
        let threads = try findAllThreads(ws)
        var results: [ThreadInfo] = []

        for threadPath in threads {
            guard let t = try? Thread.parse(path: threadPath) else { continue }

            let (cat, proj, name) = parseThreadPath(ws, threadPath)
            let threadStatus = t.status
            let baseStatus = Thread.baseStatus(threadStatus)

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
                    // At project level, show all threads here
                } else if categoryFilter != nil {
                    // At category level: only show category-level threads
                    if proj != "-" {
                        continue
                    }
                } else {
                    // At workspace level: only show workspace-level threads
                    if cat != "-" {
                        continue
                    }
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
            outputTable(results, ws, categoryFilter: categoryFilter, projectFilter: projectFilter, statusFilter: status, showAll: includeClosed, recursive: recursive)
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

    func outputTable(_ results: [ThreadInfo], _ ws: String, categoryFilter: String?, projectFilter: String?, statusFilter: String?, showAll: Bool, recursive: Bool) {
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

        var recursiveSuffix = ""
        if recursive {
            recursiveSuffix = " (including nested)"
        }

        if !statusDesc.isEmpty {
            print("Showing \(results.count) \(statusDesc) \(levelDesc) threads\(pathSuffix)\(recursiveSuffix)")
        } else {
            print("Showing \(results.count) \(levelDesc) threads\(pathSuffix) (all statuses)\(recursiveSuffix)")
        }
        print()

        if results.isEmpty {
            if !recursive {
                print("Hint: use -r to include nested categories/projects")
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

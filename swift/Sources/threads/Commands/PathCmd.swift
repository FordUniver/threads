import ArgumentParser
import Foundation
import Yams

struct PathCmd: ParsableCommand {
    static var configuration = CommandConfiguration(
        commandName: "path",
        abstract: "Print thread file path"
    )

    @Option(name: .shortAndLong, help: "Output format (json, yaml, plain)")
    var format: String?

    @Flag(name: .long, help: "Output as JSON (shorthand for --format=json)")
    var json = false

    @Argument(help: "Thread ID or name")
    var id: String

    func run() throws {
        let ws = try getWorkspace()
        let fmt = json ? "json" : (format?.lowercased() ?? "fancy")

        let file = try findByRef(ws, id)
        let absPath = (file as NSString).standardizingPath
        let relPath = file.relativePath(from: ws) ?? file

        switch fmt {
        case "json":
            let data: [String: Any] = ["path": relPath, "path_absolute": absPath]
            if let jsonData = try? JSONSerialization.data(withJSONObject: data, options: [.prettyPrinted, .sortedKeys]),
               let output = String(data: jsonData, encoding: .utf8) {
                print(output)
            }
        case "yaml":
            let data = ["path": relPath, "path_absolute": absPath]
            if let output = try? Yams.dump(object: data) {
                print(output, terminator: "")
            }
        default:
            print(absPath)
        }
    }
}

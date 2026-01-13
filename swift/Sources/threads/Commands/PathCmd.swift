import ArgumentParser
import Foundation

struct PathCmd: ParsableCommand {
    static var configuration = CommandConfiguration(
        commandName: "path",
        abstract: "Print thread file path"
    )

    @Argument(help: "Thread ID or name")
    var id: String

    func run() throws {
        let ws = try getWorkspace()

        let file = try findByRef(ws, id)
        let absPath = URL(fileURLWithPath: file).path
        print(absPath)
    }
}

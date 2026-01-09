import ArgumentParser
import Foundation

struct ReadCmd: ParsableCommand {
    static var configuration = CommandConfiguration(
        commandName: "read",
        abstract: "Read thread content"
    )

    @Argument(help: "Thread ID or name")
    var id: String

    func run() throws {
        let ws = try getWorkspace()

        let file = try findByRef(ws, id)
        let content = try String(contentsOfFile: file, encoding: .utf8)
        print(content, terminator: "")
    }
}

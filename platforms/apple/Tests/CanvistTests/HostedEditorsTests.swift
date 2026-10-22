@testable import Canvist
import XCTest

/// What React Native and Lynx call through: editors by id, JSON ranges, and
/// failures carrying their codes.
final class HostedEditorsTests: XCTestCase {
    private func editor(_ markdown: String) throws -> Editor {
        var options = defaultEditorOptions()
        options.extensions = StarterKit().extensions.names
        let editor = try Editor(options: options, measurer: CoreTextMeasurer())
        try editor.load(format: .markdown, content: markdown)

        return editor
    }

    func testCallsReachTheEditorAnIdNamesUntilItIsUnregistered() throws {
        let editor = try editor("Hello")
        let id = "hosted-\(UUID().uuidString)"
        HostedEditors.shared.register(editor, id: id)

        let text = try HostedEditors.shared.call(id) { $0.export(format: .text) }
        XCTAssertEqual(text, "Hello")

        HostedEditors.shared.unregister(id: id)
        XCTAssertThrowsError(try HostedEditors.shared.call(id) { $0.canUndo() }) { error in
            XCTAssertEqual((error as? HostedEditorError)?.code, "editor")
        }
    }

    func testAnEditorIsNotKeptAliveByTheRegistry() throws {
        let id = "hosted-\(UUID().uuidString)"
        weak var released: Editor?

        try autoreleasepool {
            let editor = try editor("Gone")
            released = editor
            HostedEditors.shared.register(editor, id: id)
        }

        XCTAssertNil(released)
        XCTAssertThrowsError(try HostedEditors.shared.call(id) { $0.canUndo() })
    }

    func testRangesRoundTripAsJSON() throws {
        let editor = try editor("# Title\n\nBody text")
        let range = #"{"anchor":{"path":[1],"offset":0},"head":{"path":[1],"offset":4}}"#

        XCTAssertTrue(try editor.select(rangeJSON: range))
        XCTAssertEqual(editor.selectionJSON(), range)
    }

    func testFailuresCarryTheirCodes() throws {
        let editor = try editor("text")

        XCTAssertThrowsError(try editor.select(rangeJSON: #"{"anchor":{}}"#)) { error in
            XCTAssertEqual(HostedEditorError(error).code, "invalidPosition")
        }
        XCTAssertThrowsError(try DocFormat(name: "rtf")) { error in
            XCTAssertEqual(HostedEditorError(error).code, "invalidArgs")
        }
        XCTAssertThrowsError(try editor.run(command: Command(name: "toggleHeading", args: #"{"level":9}"#))) { error in
            let failure = HostedEditorError(error)
            XCTAssertEqual(failure.code, "invalidArgs")
            XCTAssertTrue(failure.message.hasPrefix("[canvist:invalidArgs] "))
        }
    }
}
